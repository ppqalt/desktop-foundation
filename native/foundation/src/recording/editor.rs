//! Immutable replay snapshots and finite, bounded media jobs. Never captures a screen.
use super::*;
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn valid_id(id: &str) -> bool {
    id.len() == 24 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
fn base() -> Result<PathBuf> {
    let base =
        state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/replay-editor");
    state::private_directory(&base)?;
    Ok(base)
}
fn session(id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        return Err(invalid("Invalid replay editor session"));
    }
    let path = base()?.join(id);
    let m = fs::symlink_metadata(&path)?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
    {
        return Err(invalid("Unsafe replay editor session"));
    }
    Ok(path)
}
fn owned_file(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.len() == 0
    {
        return Err(invalid("Replay editor source is not an owned regular file"));
    }
    Ok(())
}
fn read_json(path: &Path) -> Result<Value> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !f.metadata()?.is_file() || f.metadata()?.len() > 128 * 1024 {
        return Err(invalid("Invalid editor manifest"));
    }
    Ok(serde_json::from_reader(f.take(128 * 1024 + 1))?)
}
fn inspect(path: &Path) -> Result<Value> {
    owned_file(path)?;
    let out = process::checked_capture(&[
        "ffprobe".into(), "-v".into(), "error".into(), "-count_packets".into(),
        "-show_entries".into(), "format=duration,format_name:stream=index,codec_type,codec_name,width,height,channels,sample_rate,avg_frame_rate,start_time,duration,nb_read_packets:stream_tags=title".into(),
        "-of".into(), "json".into(), path.to_string_lossy().into_owned(),
    ], Duration::from_secs(45), 128 * 1024)?;
    let metadata: Value = serde_json::from_slice(&out.stdout)?;
    let streams = metadata["streams"]
        .as_array()
        .ok_or_else(|| invalid("Replay has no readable tracks"))?;
    let videos: Vec<_> = streams
        .iter()
        .filter(|s| s["codec_type"] == "video")
        .collect();
    let audios: Vec<_> = streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .collect();
    let duration = number(&metadata["format"]["duration"])?;
    if duration < 0.05
        || duration > 3660.0
        || videos.len() != 1
        || audios.len() != 2
        || !metadata["format"]["format_name"]
            .as_str()
            .is_some_and(|s| s.contains("matroska"))
        || videos[0]["width"].as_u64().unwrap_or(0) == 0
        || videos[0]["height"].as_u64().unwrap_or(0) == 0
        || audios.iter().any(|s| s["codec_name"] != "opus")
        || streams.iter().any(|s| {
            s["nb_read_packets"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
                == 0
        })
    {
        return Err(invalid(
            "Replay must contain native video and two nonempty Opus tracks in MKV",
        ));
    }
    Ok(metadata)
}
fn number(v: &Value) -> Result<f64> {
    let n = v
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| v.as_f64())
        .filter(|v| v.is_finite());
    n.ok_or_else(|| invalid("Invalid replay time"))
}
fn duration(metadata: &Value) -> Result<f64> {
    number(&metadata["format"]["duration"])
}

/// Called only by the existing recorder owner after a non-clearing save-replay IPC.
pub(super) fn pin(id: &str, source: &Path) -> Result<()> {
    let dir = session(id)?;
    let _lock = state::lock(&dir.join("job.lock"), Duration::from_millis(100))?;
    owned_file(source)?;
    let target = dir.join("source.mkv");
    if fs::metadata(source)?.len() > 2 * 1024 * 1024 * 1024 {
        return Err(invalid("Replay snapshot exceeds the 2 GiB editor limit"));
    }
    match fs::hard_link(source, &target) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(libc::EXDEV) => {
            let mut input = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(source)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&target)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        }
        Err(error) => return Err(error.into()),
    }
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
    File::open(&target)?.sync_all()?;
    // Publish capture completion only after the immutable source exists.
    state::atomic(&dir.join("capture.json"), b"{\"captured\":true}", 0o600)?;
    fs::remove_file(source)?;
    Ok(())
}
pub(super) fn fail(id: &str, error: &str) -> Result<()> {
    state::atomic(
        &session(id)?.join("error.json"),
        &serde_json::to_vec(&json!({"error":error}))?,
        0o600,
    )
}
fn wait_for_capture(dir: &Path, request: impl FnOnce() -> Result<()>) -> Result<()> {
    let cpath = CString::new(dir.as_os_str().as_encoded_bytes())
        .map_err(|_| invalid("Invalid editor path"))?;
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let watch = unsafe { File::from_raw_fd(fd) };
    if unsafe {
        libc::inotify_add_watch(fd, cpath.as_ptr(), libc::IN_MOVED_TO | libc::IN_CLOSE_WRITE)
    } < 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    request()?;
    let deadline = Instant::now() + Duration::from_secs(150);
    loop {
        if dir.join("error.json").try_exists()? {
            return Err(invalid(
                read_json(&dir.join("error.json"))?["error"]
                    .as_str()
                    .unwrap_or("Snapshot failed"),
            ));
        }
        if dir.join("capture.json").try_exists()? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(invalid(
                "Replay snapshot timed out; source retained if it arrives later",
            ));
        }
        let mut poll = libc::pollfd {
            fd: watch.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(1000) as i32;
        let result = unsafe { libc::poll(&mut poll, 1, ms) };
        if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut bytes = [0u8; 4096];
        let _ = (&watch).read(&mut bytes);
    }
}
fn frame(dir: &Path, seconds: f64, large: bool) -> Result<Value> {
    let metadata = read_json(&dir.join("session.json"))?;
    let end = metadata["duration"]
        .as_f64()
        .ok_or_else(|| invalid("Invalid snapshot duration"))?;
    if !seconds.is_finite() || seconds < 0.0 || seconds > end {
        return Err(invalid("Preview lies outside the snapshot"));
    }
    let fps = metadata["fps"]
        .as_f64()
        .filter(|f| f.is_finite() && *f > 0.0)
        .unwrap_or(60.0)
        .clamp(1.0, 240.0);
    let rate = if large { fps } else { 10.0 };
    let tick = (seconds.min((end - 1.0 / fps).max(0.0)) * rate).floor() as u64;
    let frame_time = tick as f64 / rate;
    let target = dir.join(format!(
        "{}-{tick:06}.jpg",
        if large { "preview" } else { "thumb" }
    ));
    if !target.try_exists()? {
        if large
            && fs::read_dir(dir)?
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("preview-"))
                .count()
                >= 128
        {
            // Only discard derived preview frames; the source and selected footage remain immutable.
            for e in fs::read_dir(dir)?.filter_map(std::result::Result::ok) {
                if e.file_name().to_string_lossy().starts_with("preview-")
                    && e.path().extension().is_some_and(|e| e == "jpg")
                {
                    fs::remove_file(e.path())?;
                }
            }
        }
        let temporary = dir.join("frame.part.jpg");
        let args = vec![
            "ffmpeg".into(),
            "-nostdin".into(),
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-threads".into(),
            "1".into(),
            "-ss".into(),
            format!("{frame_time:.6}"),
            "-i".into(),
            dir.join("source.mkv").to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            "-an".into(),
            "-sn".into(),
            "-vf".into(),
            format!("scale={}:-2", if large { 640 } else { 240 }),
            "-filter_threads".into(),
            "1".into(),
            "-threads:v".into(),
            "1".into(),
            temporary.to_string_lossy().into_owned(),
        ];
        process::checked_capture(&args, Duration::from_secs(20), 8192)?;
        owned_file(&temporary)?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        fs::rename(temporary, &target)?;
    }
    Ok(json!({"url":state::file_uri(&target)?, "seconds":frame_time}))
}
fn describe(id: &str, dir: &Path, metadata: &Value) -> Result<Value> {
    let mut value = metadata.clone();
    value["id"] = id.into();
    value["source_url"] = state::file_uri(&dir.join("source.mkv"))?.into();
    if dir.join("export-error.json").try_exists()? {
        value["export_error"] = read_json(&dir.join("export-error.json"))?["error"].clone();
    }
    Ok(value)
}
fn initialize(id: &str) -> Result<Value> {
    let dir = session(id)?;
    let _lock = state::lock(&dir.join("job.lock"), Duration::from_millis(100))?;
    let metadata = inspect(&dir.join("source.mkv"))?;
    let video = metadata["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["codec_type"] == "video")
        .unwrap();
    let fps = video["avg_frame_rate"]
        .as_str()
        .and_then(|s| s.split_once('/'))
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(60.0);
    let mut value = json!({"duration":duration(&metadata)?, "width":video["width"], "height":video["height"], "fps":fps, "audio_tracks":2, "thumbnails":[]});
    state::atomic(
        &dir.join("session.json"),
        &serde_json::to_vec(&value)?,
        0o600,
    )?;
    let mut thumbnails = Vec::new();
    for i in 0..10 {
        thumbnails.push(frame(
            &dir,
            value["duration"].as_f64().unwrap() * i as f64 / 10.0,
            false,
        )?);
    }
    value["thumbnails"] = thumbnails.into();
    state::atomic(
        &dir.join("session.json"),
        &serde_json::to_vec(&value)?,
        0o600,
    )?;
    describe(id, &dir, &value)
}
pub(super) fn open() -> Result<Value> {
    let current = runtime::status()?;
    if current["replay"] != true
        || current["replay_status"] == "paused"
        || current["phase"] == "error"
    {
        return Err(invalid(
            "Enable Instant Replay and wait for captured frames before opening the editor",
        ));
    }
    let base = base()?;
    let _lock = state::lock(&base.join("prepare.lock"), Duration::from_millis(100))?;
    let sessions: Vec<_> = fs::read_dir(&base)?
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            valid_id(&e.file_name().to_string_lossy()) && e.file_type().is_ok_and(|t| t.is_dir())
        })
        .collect();
    if sessions.len() >= 3 {
        return Err(invalid(
            "Three protected replays are retained; resume or discard one before capturing another",
        ));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Invalid clock"))?
        .as_nanos();
    let id = format!("{:016x}{:08x}", nanos as u64, std::process::id());
    let dir = base.join(&id);
    fs::DirBuilder::new().mode(0o700).create(&dir)?;
    wait_for_capture(&dir, || runtime::editor_request(&id))?;
    initialize(&id)
}
fn export(root: &Path, id: &str, start: f64, end: f64) -> Result<Value> {
    let dir = session(id)?;
    let _lock = state::lock(&dir.join("job.lock"), Duration::from_millis(100))?;
    let source = dir.join("source.mkv");
    let metadata = inspect(&source)?;
    let total = duration(&metadata)?;
    if !start.is_finite()
        || !end.is_finite()
        || start < 0.0
        || end > total + 0.001
        || end - start < 0.05
    {
        return Err(invalid("Select at least 50 ms within the protected replay"));
    }
    let (config, _) = settings(root)?;
    let output = if config.directory.is_empty() {
        video_directory()?
    } else {
        PathBuf::from(config.directory)
    };
    fs::create_dir_all(&output)?;
    let temporary = output.join(format!(".{}", runtime::readable_name("Clip-stage")?));
    if temporary.try_exists()? {
        return Err(invalid(
            "A previous incomplete clip is retained; inspect it before saving again",
        ));
    }
    let mut reserved = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let audio_rates: Vec<_> = metadata["streams"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .map(|s| {
            config
                .audio_bitrate_kbps
                .min(256 * s["channels"].as_u64().unwrap_or(1).clamp(1, 2) as u32)
        })
        .collect();
    let whole = start < 0.0005 && (end - total).abs() < 0.0005;
    if whole {
        // A full-buffer save preserves every encoded packet without re-encoding.
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&source)?;
        std::io::copy(&mut input, &mut reserved)?;
        reserved.sync_all()?;
    } else {
        // Exact arbitrary cuts require decoding from the preceding keyframe.
        // Keep video on NVENC and trim/re-encode each independent audio track.
        let args = vec![
            "ffmpeg".into(),
            "-nostdin".into(),
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-threads".into(),
            "2".into(),
            "-ss".into(),
            format!("{start:.6}"),
            "-i".into(),
            source.to_string_lossy().into_owned(),
            "-t".into(),
            format!("{:.6}", end - start),
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "0:a:0".into(),
            "-map".into(),
            "0:a:1".into(),
            "-map_metadata".into(),
            "0".into(),
            "-c:v".into(),
            "h264_nvenc".into(),
            "-preset".into(),
            "p4".into(),
            "-cq".into(),
            "19".into(),
            "-b:v".into(),
            "0".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a:0".into(),
            format!("{}k", audio_rates[0]),
            "-b:a:1".into(),
            format!("{}k", audio_rates[1]),
            "-filter_threads".into(),
            "1".into(),
            "-f".into(),
            "matroska".into(),
            temporary.to_string_lossy().into_owned(),
        ];
        process::checked_capture(&args, Duration::from_secs(600), 64 * 1024).map_err(|e| {
            invalid(format!(
                "Clip encoding failed; partial output retained at {}: {e}",
                temporary.display()
            ))
        })?;
    }
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    let saved = inspect(&temporary)?;
    let expected = end - start;
    if (duration(&saved)? - expected).abs() > 0.25 {
        return Err(invalid(
            "Clip duration validation failed; partial file retained",
        ));
    }
    let videos = |m: &Value| {
        m["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap()
            .clone()
    };
    if videos(&saved)["width"] != videos(&metadata)["width"]
        || videos(&saved)["height"] != videos(&metadata)["height"]
    {
        return Err(invalid("Clip resolution changed; file retained"));
    }
    let starts: Vec<_> = saved["streams"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| number(&s["start_time"]).ok())
        .collect();
    if starts.len() != 3
        || starts.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - starts.iter().copied().fold(f64::INFINITY, f64::min)
            > 0.1
    {
        return Err(invalid(
            "Clip track synchronization validation failed; file retained",
        ));
    }
    File::open(&temporary)?.sync_all()?;
    let destination = output.join(runtime::readable_name("Clip")?);
    fs::hard_link(&temporary, &destination)?;
    fs::remove_file(temporary)?;
    let result = json!({"saved":true,"path":destination,"duration":duration(&saved)?,"audio_tracks":2,"mode":if whole {"copy"} else {"nvenc"}});
    state::atomic(
        &dir.join("saved.json"),
        &serde_json::to_vec(&result)?,
        0o600,
    )?;
    Ok(result)
}
pub(super) fn run(root: &Path, args: &[String]) -> Result<()> {
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["open"] => open()?,
        ["resume", id] => initialize(id)?,
        ["preview", id, seconds] => {
            let dir = session(id)?;
            let _lock = state::lock(&dir.join("job.lock"), Duration::from_millis(100))?;
            frame(
                &dir,
                seconds
                    .parse()
                    .map_err(|_| invalid("Invalid preview time"))?,
                true,
            )?
        }
        ["save", id, start, end] => {
            let result = export(
                root,
                id,
                start.parse().map_err(|_| invalid("Invalid clip start"))?,
                end.parse().map_err(|_| invalid("Invalid clip end"))?,
            );
            match result {
                Ok(value) => {
                    let _ = fs::remove_file(session(id)?.join("export-error.json"));
                    runtime::notify(&format!(
                        "Saved {}",
                        value["path"].as_str().unwrap_or("replay clip")
                    ));
                    value
                }
                Err(error) => {
                    if let Ok(dir) = session(id) {
                        let _ = state::atomic(
                            &dir.join("export-error.json"),
                            &serde_json::to_vec(&json!({"error":error.to_string()}))?,
                            0o600,
                        );
                    }
                    runtime::notify(&format!("Replay clip not saved: {error}"));
                    return Err(error);
                }
            }
        }
        ["list"] => {
            let mut sessions = Vec::new();
            for entry in fs::read_dir(base()?)? {
                let entry = entry?;
                let id = entry.file_name().to_string_lossy().into_owned();
                if valid_id(&id) && entry.file_type()?.is_dir() {
                    sessions
                        .push(json!({"id":id,"ready":entry.path().join("source.mkv").is_file()}));
                }
            }
            sessions.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
            json!({"sessions":sessions})
        }
        ["discard", id] => {
            let dir = session(id)?;
            let _lock = state::lock(&dir.join("job.lock"), Duration::from_millis(100))?;
            // Only this validated private session is removed, never a saved recording.
            fs::remove_dir_all(dir)?;
            json!({"discarded":true})
        }
        _ => {
            return Err(invalid(
                "Expected recording editor open|resume ID|preview ID SECONDS|save ID START END|list|discard ID",
            ));
        }
    };
    println!("{result}");
    Ok(())
}
