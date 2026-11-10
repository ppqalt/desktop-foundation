//! A socket-activated owner. Sleeps on kernel events, and exits after saving.
use super::*;
mod replay;
use std::{
    env,
    ffi::CString,
    fs::OpenOptions,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{MetadataExt, OpenOptionsExt},
            net::UnixDatagram,
            process::CommandExt,
        },
    },
    process::{Child, Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

fn directory() -> Result<PathBuf> {
    let path = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| invalid("Recording requires XDG_RUNTIME_DIR"))?;
    let meta = fs::symlink_metadata(&path)?;
    // SAFETY: geteuid takes no arguments.
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(invalid(
            "Recording requires a private owned runtime directory",
        ));
    }
    let path = path.join("desktop-foundation-recording");
    if let Ok(meta) = fs::symlink_metadata(&path)
        && (!meta.is_dir() || meta.uid() != unsafe { libc::geteuid() })
    {
        return Err(invalid("Recording state directory is not owned"));
    }
    state::private_directory(&path)?;
    Ok(path)
}
fn publish(value: &Value) -> Result<()> {
    let mut value = value.clone();
    value["control_protocol"] = 2.into();
    let dir = directory()?;
    let _publication = state::lock(&dir.join("status.lock"), Duration::from_secs(2))?;
    state::atomic(
        &dir.join("status.json"),
        &serde_json::to_vec(&value)?,
        0o600,
    )
}
pub fn status() -> Result<Value> {
    let dir = directory()?;
    let _publication = state::lock(&dir.join("status.lock"), Duration::from_secs(2))?;
    match bounded(&dir.join("status.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(crate::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
            // The same publication lock protects initialization and subsequent
            // writers. Readers never observe an empty or partially written file.
            let idle = json!({"phase":"idle"});
            state::atomic(&dir.join("status.json"), &serde_json::to_vec(&idle)?, 0o600)?;
            Ok(idle)
        }
        Err(e) => Err(e),
    }
}
pub fn cleanup() -> Result<()> {
    let dir = directory()?;
    let _owner = state::lock(&dir.join("owner.lock"), Duration::from_millis(100))?;
    let mut s = status()?;
    if matches!(
        s["phase"].as_str(),
        Some("starting" | "recording" | "stopping" | "replay" | "replay-recording")
    ) {
        s["phase"] = "error".into();
        s["error"] =
            "Recorder owner exited unexpectedly; any partial recording was retained".into();
        publish(&s)?;
    }
    Ok(())
}
pub fn request(silent: bool, stop: bool) -> Result<()> {
    request_mode(silent.into(), stop)
}
pub(super) fn request_mode(mode: AudioMode, stop: bool) -> Result<()> {
    let result = request_inner(mode, stop, false);
    if let Err(error) = &result {
        notify(&error.to_string());
    }
    result
}
pub(super) fn request_start(mode: AudioMode) -> Result<()> {
    request_inner(mode, false, true)
}
pub(super) fn stop_recording() -> Result<()> {
    require_extended_owner()?;
    let dir = directory()?;
    let socket = UnixDatagram::unbound()?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    // Never socket-activate a recorder merely to stop it.
    if dir.join("requests.sock").exists() {
        socket.send_to(b"stop-recording", dir.join("requests.sock"))?;
    }
    Ok(())
}
fn require_extended_owner() -> Result<()> {
    let current = status()?;
    if !matches!(current["phase"].as_str(), Some("idle" | "error"))
        && current["control_protocol"] != 2
    {
        return Err(invalid(
            "Recorder restart needed for the new controls; finish recording and turn replay off with the existing shortcuts first",
        ));
    }
    Ok(())
}
fn request_inner(mode: AudioMode, stop: bool, start_only: bool) -> Result<()> {
    if start_only || mode == AudioMode::None {
        require_extended_owner()?;
    }
    if !stop && !apps::executable("gpu-screen-recorder") {
        return Err(invalid("Install gpu-screen-recorder to enable recording"));
    }
    let dir = directory()?;
    let _lock = state::lock(&dir.join("request.lock"), Duration::from_secs(3))?;
    if stop
        && !matches!(
            status()?["phase"].as_str(),
            Some("starting" | "recording" | "stopping" | "replay" | "replay-recording")
        )
    {
        return Ok(());
    }
    process::checked(
        &[
            "systemctl".into(),
            "--user".into(),
            "start".into(),
            SOCKET.into(),
        ],
        None,
        Duration::from_secs(3),
        true,
    )?;
    let socket = UnixDatagram::unbound()?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    socket.send_to(
        if stop {
            b"stop"
        } else if start_only {
            mode.start_request()
        } else {
            mode.request()
        },
        dir.join("requests.sock"),
    )?;
    Ok(())
}
pub fn replay_request(save: bool) -> Result<()> {
    let result = replay_request_inner(if save {
        b"replay-save"
    } else {
        b"replay-toggle"
    });
    if let Err(error) = &result {
        notify(&error.to_string());
    }
    result
}
pub fn replay_set(enabled: bool) -> Result<()> {
    let result = replay_request_inner(if enabled { b"replay-on" } else { b"replay-off" });
    if let Err(error) = &result {
        notify(&error.to_string());
    }
    result
}
pub(super) fn editor_request(id: &str) -> Result<()> {
    if !editor::valid_id(id) {
        return Err(invalid("Invalid editor request"));
    }
    replay_request_inner(format!("edit:{id}").as_bytes())
}
fn editor_id(request: &[u8]) -> Option<&str> {
    std::str::from_utf8(request)
        .ok()?
        .strip_prefix("edit:")
        .filter(|id| editor::valid_id(id))
}
fn replay_request_inner(action: &[u8]) -> Result<()> {
    if !apps::executable("gpu-screen-recorder") {
        return Err(invalid("Install gpu-screen-recorder to enable replay"));
    }
    let dir = directory()?;
    let _lock = state::lock(&dir.join("request.lock"), Duration::from_secs(3))?;
    let current = status()?;
    if ["starting", "recording", "stopping"].contains(&current["phase"].as_str().unwrap_or(""))
        && current["replay"] != true
        && current["replay_paused"] != true
    {
        return Err(invalid("Stop the current recording before starting replay"));
    }
    process::checked(
        &[
            "systemctl".into(),
            "--user".into(),
            "start".into(),
            SOCKET.into(),
        ],
        None,
        Duration::from_secs(3),
        true,
    )?;
    let socket = UnixDatagram::unbound()?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    socket.send_to(action, dir.join("requests.sock"))?;
    Ok(())
}
pub(super) fn notify(message: &str) {
    let _ = process::checked(
        &[
            "notify-send".into(),
            "-a".into(),
            "Desktop Foundation".into(),
            "Recording".into(),
            message.into(),
        ],
        None,
        Duration::from_secs(2),
        false,
    );
}
pub fn acknowledge() -> Result<()> {
    let dir = directory()?;
    let _owner = state::lock(&dir.join("owner.lock"), Duration::from_millis(100))?;
    let mut s = status()?;
    if s["phase"] == "error" {
        s["phase"] = "idle".into();
        publish(&s)?;
    }
    Ok(())
}
fn finalized(file: &Path, mode: impl Into<AudioMode>) -> Result<()> {
    if !fs::metadata(file).is_ok_and(|m| m.is_file() && m.len() > 0) {
        return Err(invalid("Recording output is missing or empty"));
    }
    let output = process::checked(
        &[
            "ffprobe".into(),
            "-v".into(),
            "error".into(),
            "-count_packets".into(),
            "-show_entries".into(),
            "format=duration,size:stream=codec_type,codec_name,width,height,nb_read_packets".into(),
            "-of".into(),
            "json".into(),
            file.to_string_lossy().into_owned(),
        ],
        None,
        Duration::from_secs(30),
        true,
    )?;
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    validate_metadata(&metadata, mode)
}
fn validate_metadata(metadata: &Value, mode: impl Into<AudioMode>) -> Result<()> {
    let mode = mode.into();
    let streams = metadata["streams"]
        .as_array()
        .ok_or_else(|| invalid("Recording has no readable streams"))?;
    let videos = streams
        .iter()
        .filter(|v| v["codec_type"] == "video")
        .count();
    let audios = streams
        .iter()
        .filter(|v| v["codec_type"] == "audio")
        .count();
    let packets = |stream: &Value| {
        stream["nb_read_packets"]
            .as_str()
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|count| count > 0)
    };
    let valid_video = streams
        .iter()
        .filter(|v| v["codec_type"] == "video")
        .all(|v| {
            v["codec_name"]
                .as_str()
                .is_some_and(|name| !name.is_empty())
                && v["width"].as_u64().is_some_and(|width| width > 0)
                && v["height"].as_u64().is_some_and(|height| height > 0)
                && packets(v)
        });
    let valid_audio = streams
        .iter()
        .filter(|v| v["codec_type"] == "audio")
        .all(|v| v["codec_name"] == "opus" && packets(v));
    if videos != 1
        || audios != mode.tracks()
        || !valid_video
        || !valid_audio
        || !metadata["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(|v| v.is_finite() && v > 0.0)
    {
        return Err(invalid(
            "Recording finalization validation failed; file retained for inspection",
        ));
    }
    Ok(())
}

struct Signals {
    fd: OwnedFd,
    old: libc::sigset_t,
}
impl Signals {
    fn new() -> Result<Self> {
        // SAFETY: initialized signal sets and owned descriptor; this process is
        // single-threaded. Children restore the previous mask before exec.
        unsafe {
            let mut set = std::mem::zeroed();
            let mut old = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            for signal in [libc::SIGCHLD, libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
                libc::sigaddset(&mut set, signal);
            }
            let error = libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut old);
            if error != 0 {
                return Err(io::Error::from_raw_os_error(error).into());
            }
            let fd = libc::signalfd(-1, &set, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK);
            if fd < 0 {
                libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut());
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self {
                fd: OwnedFd::from_raw_fd(fd),
                old,
            })
        }
    }
    fn events(&self) -> Result<(bool, bool)> {
        let mut stopping = false;
        let mut child_changed = false;
        loop {
            // SAFETY: writable buffer with exact signalfd_siginfo size.
            let mut info: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
            let n = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    (&mut info as *mut libc::signalfd_siginfo).cast(),
                    std::mem::size_of_val(&info),
                )
            };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.into());
            }
            if n == 0 {
                break;
            }
            stopping |= info.ssi_signo != libc::SIGCHLD as u32;
            child_changed |= info.ssi_signo == libc::SIGCHLD as u32;
        }
        Ok((stopping, child_changed))
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        // SAFETY: restore the saved signal mask after owned children are reaped.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.old, std::ptr::null_mut());
        }
    }
}
struct Capture {
    child: Child,
    reaped: bool,
}
impl Capture {
    fn stop(&mut self) -> Result<()> {
        // SAFETY: this is our unreaped child, so its PID cannot be recycled.
        if unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGINT) } != 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::ESRCH) {
                return Err(e.into());
            }
        }
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.stop();
            let until = Instant::now() + Duration::from_secs(3);
            while Instant::now() < until {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn queued(socket: &UnixDatagram) -> Result<Option<Vec<u8>>> {
    let mut buf = [0; 32];
    match socket.recv(&mut buf) {
        Ok(n)
            if [
                b"toggle".as_slice(),
                b"silent".as_slice(),
                b"no-audio".as_slice(),
                b"start-mixed".as_slice(),
                b"start-desktop".as_slice(),
                b"start-no-audio".as_slice(),
                b"stop-recording".as_slice(),
                b"stop".as_slice(),
                b"replay-save".as_slice(),
                b"replay-toggle".as_slice(),
                b"replay-on".as_slice(),
                b"replay-off".as_slice(),
            ]
            .contains(&&buf[..n])
                || editor_id(&buf[..n]).is_some() =>
        {
            Ok(Some(buf[..n].to_vec()))
        }
        Ok(_) => Err(invalid("Unknown recording request")),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn drain_stop_requests(socket: &UnixDatagram) -> Result<()> {
    // Once saving has begun, further presses are stop requests rather than a
    // new activation waiting in systemd's socket. The kernel queue is bounded.
    while queued(socket)?.is_some() {}
    Ok(())
}
fn capture_replay_request(
    request: &[u8],
    paused: bool,
    resume: &mut bool,
    state: &mut Value,
) -> Result<bool> {
    if let Some(id) = editor_id(request) {
        let _ = editor::fail(
            id,
            "Replay editing is unavailable during ordinary or desktop-only capture",
        );
        return Ok(true);
    }
    if paused
        && [
            b"replay-toggle".as_slice(),
            b"replay-on".as_slice(),
            b"replay-off".as_slice(),
        ]
        .contains(&request)
    {
        *resume = match request {
            b"replay-on" => true,
            b"replay-off" => false,
            _ => !*resume,
        };
        state["replay"] = (*resume).into();
        state["replay_status"] = if *resume { "paused" } else { "off" }.into();
        publish(state)?;
        return Ok(true);
    }
    if request == b"replay-save" && paused {
        state["save_status"] = "error".into();
        state["save_error"] = "Replay is paused while recording with a different audio mode".into();
        publish(state)?;
        notify("Replay is paused while recording with a different audio mode");
        return Ok(true);
    }
    Ok(false)
}
fn drain_capture_requests(
    socket: &UnixDatagram,
    paused: bool,
    resume: &mut bool,
    state: &mut Value,
) -> Result<()> {
    while let Some(request) = queued(socket)? {
        let _ = capture_replay_request(&request, paused, resume, state)?;
    }
    Ok(())
}
fn first_frame_timestamp(text: &str) -> bool {
    let values: Vec<_> = text.split_whitespace().collect();
    let pair = match values.as_slice() {
        [monotonic, realtime] => [*monotonic, *realtime],
        [
            "monotonic_microsec",
            "realtime_microsec",
            monotonic,
            realtime,
        ] => [*monotonic, *realtime],
        _ => return false,
    };
    pair.iter()
        .all(|value| value.parse::<u64>().is_ok_and(|n| n > 0))
}
fn ready(marker: &Path) -> bool {
    bounded(marker).ok().is_some_and(|bytes| {
        std::str::from_utf8(&bytes)
            .ok()
            .is_some_and(first_frame_timestamp)
    })
}
fn watch(path: &Path) -> Result<OwnedFd> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| invalid("Invalid recording directory"))?;
    // SAFETY: fresh owned fd and NUL-terminated existing directory path.
    unsafe {
        let fd = libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK);
        if fd < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let owned = OwnedFd::from_raw_fd(fd);
        if libc::inotify_add_watch(
            fd,
            path.as_ptr(),
            libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF,
        ) < 0
        {
            return Err(io::Error::last_os_error().into());
        }
        Ok(owned)
    }
}
fn capabilities() -> Result<()> {
    let help = process::run(
        &["gpu-screen-recorder".into(), "--help".into()],
        None,
        Duration::from_secs(2),
        true,
    )?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&help.stdout),
        String::from_utf8_lossy(&help.stderr)
    );
    for option in [
        "-write-first-frame-ts",
        "-fallback-cpu-encoding",
        "-portal-session-token-filepath",
        "-ab",
    ] {
        if !text.contains(option) {
            return Err(invalid(format!(
                "GPU Screen Recorder lacks {option}; update the recorder package"
            )));
        }
    }
    Ok(())
}
pub fn serve(root: &Path) -> Result<()> {
    if env::var("LISTEN_PID").as_deref() != Ok(&std::process::id().to_string())
        || env::var("LISTEN_FDS").as_deref() != Ok("1")
        || env::var_os("INVOCATION_ID").is_none()
    {
        return Err(invalid(
            "Start recording through its socket-activated user service",
        ));
    }
    // SAFETY: systemd passed this process exactly one owned activation socket.
    let socket = unsafe { UnixDatagram::from_raw_fd(3) };
    socket.set_nonblocking(true)?;
    // Drain activation before fallible setup; failed requests cannot reactivate forever.
    let initial = queued(&socket)?;
    let Some(initial) = initial else {
        return Ok(());
    };
    if initial == b"stop" || initial == b"stop-recording" || initial == b"replay-off" {
        publish(&json!({"phase":"idle"}))?;
        return Ok(());
    }
    if let Some(id) = editor_id(&initial) {
        editor::fail(id, "Replay stopped before the snapshot could be captured")?;
        return Ok(());
    }
    let mode = AudioMode::from_request(&initial);
    let dir = directory()?;
    let _owner = state::lock(&dir.join("owner.lock"), Duration::from_millis(100))?;
    let starting_replay =
        initial == b"replay-save" || initial == b"replay-toggle" || initial == b"replay-on";
    publish(
        &json!({"phase":"starting", "silent":mode == AudioMode::Desktop, "audio_mode":mode, "replay":starting_replay, "replay_status":if starting_replay {"starting"} else {"off"}}),
    )?;
    let result = if starting_replay {
        let mut save = initial == b"replay-save";
        loop {
            match replay::run(root, &socket, save) {
                Ok(Some(mode)) => match capture(root, &socket, mode, true) {
                    Ok((true, _)) | Ok((false, false)) => break Ok(()),
                    Ok((false, true)) => save = false,
                    Err(error) => break Err(error),
                },
                Ok(None) => break Ok(()),
                Err(error) => break Err(error),
            }
        }
    } else {
        capture(root, &socket, mode, false).map(|_| ())
    };
    if let Err(e) = &result {
        let _ = drain_stop_requests(&socket);
        let mut s = status().unwrap_or_else(|_| json!({}));
        s["phase"] = "error".into();
        s["error"] = e.to_string().into();
        let _ = publish(&s);
        notify(&e.to_string());
    }
    result
}
pub(super) fn readable_name(kind: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Invalid system clock"))?;
    let seconds =
        libc::time_t::try_from(now.as_secs()).map_err(|_| invalid("Invalid system clock"))?;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: valid pointers, and localtime_r initializes the tm on success.
    if unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) }.is_null() {
        return Err(invalid("Cannot format local recording time"));
    }
    let local = unsafe { local.assume_init() };
    Ok(format!(
        "{kind}-{:04}-{:02}-{:02}_{:02}-{:02}-{:02}-{:06}.mkv",
        local.tm_year + 1900,
        local.tm_mon + 1,
        local.tm_mday,
        local.tm_hour,
        local.tm_min,
        local.tm_sec,
        now.subsec_micros()
    ))
}
fn capture(
    root: &Path,
    socket: &UnixDatagram,
    mode: AudioMode,
    replay_paused: bool,
) -> Result<(bool, bool)> {
    let mut resume_replay = replay_paused;
    capabilities()?;
    let (c, _) = settings(root)?;
    let display = output().unwrap_or(Value::Null);
    let output_dir = if c.directory.is_empty() {
        video_directory()?
    } else {
        PathBuf::from(&c.directory)
    };
    fs::create_dir_all(&output_dir)?;
    let file = output_dir.join(readable_name(mode.filename())?);
    let marker = PathBuf::from(format!("{}.ts", file.display()));
    let state_dir =
        state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/recording");
    state::private_directory(&state_dir)?;
    let args = command(&c, &display, mode, &file, &state_dir.join("portal-token"))?;
    // A second press during setup cancels, instead of unexpectedly starting capture later.
    if let Some(request) = queued(socket)? {
        if [
            b"toggle".as_slice(),
            b"silent".as_slice(),
            b"no-audio".as_slice(),
            b"stop-recording".as_slice(),
            b"stop".as_slice(),
        ]
        .contains(&request.as_slice())
        {
            publish(&json!({"phase":"idle"}))?;
            return Ok((false, resume_replay));
        }
        let mut pending = json!({"phase":"starting", "replay":resume_replay, "replay_paused":replay_paused, "replay_status":if resume_replay {"paused"} else {"off"}, "buffer_seconds":c.replay_seconds});
        if !request.starts_with(b"start-")
            && !capture_replay_request(&request, replay_paused, &mut resume_replay, &mut pending)?
        {
            notify("Stop the current recording before starting replay");
        }
    }
    let reserved = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&file)?;
    drop(reserved);
    let changes = watch(&output_dir)?;
    let signals = Signals::new()?;
    let old = signals.old;
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // SAFETY: only async-signal-safe mask restoration is done between fork/exec.
    unsafe {
        cmd.pre_exec(move || {
            let error = libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut());
            if error == 0 {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(error))
            }
        });
    }
    let mut child = Capture {
        child: cmd.spawn()?,
        reaped: false,
    };
    let mut state = json!({"phase":"starting", "silent":mode == AudioMode::Desktop, "audio_mode":mode, "output":file,
        "replay":resume_replay, "replay_paused":replay_paused, "replay_status":if resume_replay {"paused"} else {"off"}, "buffer_seconds":c.replay_seconds,
        "fps":selected_fps(&c, &display), "resolution":c.resolution, "focused_display":display["name"]});
    publish(&state)?;
    let mut started = false;
    let mut stopping = false;
    let mut startup_cancel = false;
    let mut failure = None;
    let mut shutdown_requested = false;
    let mut deadline = Some(Instant::now() + Duration::from_secs(c.startup_timeout_seconds));
    loop {
        if let Some(exit) = child.child.try_wait()? {
            child.reaped = true;
            let captured = started || ready(&marker);
            let nonempty = fs::metadata(&file).is_ok_and(|m| m.len() > 0);
            drain_capture_requests(socket, replay_paused, &mut resume_replay, &mut state)?;
            let _ = fs::remove_file(&marker);
            if !captured && !nonempty {
                let _ = fs::remove_file(&file);
            }
            if let Some(error) = failure {
                return Err(invalid(error));
            }
            if !exit.success() && !(startup_cancel && !captured && !nonempty) {
                return Err(invalid(format!(
                    "Recorder exited with {exit}; any partial recording is retained at {}",
                    file.display()
                )));
            }
            if !captured && !stopping {
                return Err(invalid("Recorder exited before a video frame was captured"));
            }
            if captured || nonempty {
                state["phase"] = "stopping".into();
                publish(&state)?;
                let finalization = finalized(&file, mode);
                drain_capture_requests(socket, replay_paused, &mut resume_replay, &mut state)?;
                finalization?;
            } else {
                drain_capture_requests(socket, replay_paused, &mut resume_replay, &mut state)?;
            }
            if shutdown_requested {
                resume_replay = false;
            }
            state["replay_paused"] = false.into();
            if resume_replay {
                state["audio_mode"] = serde_json::to_value(AudioMode::DesktopMicrophone)?;
            }
            state["phase"] = if resume_replay { "starting" } else { "idle" }.into();
            state["replay_status"] = if resume_replay { "starting" } else { "off" }.into();
            publish(&state)?;
            if captured || nonempty {
                notify(&format!("Saved {}", file.display()));
            }
            return Ok((shutdown_requested, resume_replay));
        }
        if !started && !stopping && ready(&marker) {
            started = true;
            deadline = None;
            state["phase"] = "recording".into();
            publish(&state)?;
        }
        if startup_cancel && ready(&marker) {
            // Capture became ready during the cancellation grace period. Give
            // the newly initialized encoder its normal graceful save window.
            started = true;
            startup_cancel = false;
            state["phase"] = "stopping".into();
            publish(&state)?;
            child.stop()?;
            deadline = Some(Instant::now() + Duration::from_secs(c.stop_timeout_seconds));
        }
        let expired = deadline.is_some_and(|d| Instant::now() >= d);
        if expired && stopping {
            if startup_cancel {
                // No frame has been produced. Some portal selectors do not
                // service SIGINT while blocked; close only our unstarted child.
                if let Err(error) = child.child.kill()
                    && error.raw_os_error() != Some(libc::ESRCH)
                {
                    return Err(error.into());
                }
                deadline = None;
                continue;
            }
            return Err(invalid(
                "Recorder did not finish saving in time; partial MKV retained",
            ));
        }
        let requested = match queued(socket)? {
            Some(request) if request.starts_with(b"start-") => false,
            Some(request)
                if capture_replay_request(
                    &request,
                    replay_paused,
                    &mut resume_replay,
                    &mut state,
                )? =>
            {
                false
            }
            Some(request) if request.starts_with(b"replay-") => {
                notify("Stop the current recording before starting replay");
                false
            }
            Some(_) => true,
            None => false,
        };
        let (shutdown, child_changed) = signals.events()?;
        shutdown_requested |= shutdown;
        if !stopping && (requested || shutdown || expired) {
            stopping = true;
            startup_cancel = !started && !ready(&marker);
            if expired {
                failure = Some("Screen selection/capture timed out before the first frame");
            }
            state["phase"] = if startup_cancel {
                "starting"
            } else {
                "stopping"
            }
            .into();
            state["cancelling"] = startup_cancel.into();
            publish(&state)?;
            child.stop()?;
            deadline = Some(
                Instant::now()
                    + if startup_cancel {
                        Duration::from_millis(c.cancel_timeout_milliseconds)
                    } else {
                        Duration::from_secs(c.stop_timeout_seconds)
                    },
            );
        }
        if child_changed {
            // SIGCHLD may arrive between try_wait and draining signalfd. Reap
            // before sleeping again; consuming that wakeup then blocking races.
            continue;
        }
        let mut fds = [
            libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: signals.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: changes.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let timeout = deadline.map_or(-1, |d| {
            d.saturating_duration_since(Instant::now())
                .as_millis()
                .clamp(1, i32::MAX as u128) as i32
        });
        // SAFETY: valid descriptor array, and no periodic timeout while recording.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) } < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e.into());
            }
        }
        if fds[2].revents & libc::POLLIN != 0 {
            let mut bytes = [0u8; 8192];
            // SAFETY: writable buffer, nonblocking inotify descriptor.
            unsafe {
                libc::read(changes.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len());
            }
        }
        if fds
            .iter()
            .any(|fd| fd.revents & (libc::POLLNVAL | libc::POLLERR) != 0)
        {
            return Err(invalid("Recording event source failed"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_frame_accepts_current_and_legacy_timestamp_formats_only() {
        assert!(first_frame_timestamp(
            "monotonic_microsec\trealtime_microsec\n501491478\t1794083556868029\n"
        ));
        assert!(first_frame_timestamp("100 200\n"));
        for invalid in [
            "",
            "monotonic_microsec realtime_microsec",
            "wrong header 100 200",
            "100 0",
            "0 200",
            "100 200 extra",
        ] {
            assert!(!first_frame_timestamp(invalid), "accepted {invalid:?}");
        }
    }
    #[test]
    fn finalization_requires_readable_video_and_expected_independent_audio() {
        let complete = json!({"format":{"duration":"1.25"},"streams":[
            {"codec_type":"video","codec_name":"h264","width":2560,"height":1440,"nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus","nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus","nb_read_packets":"10"}]});
        validate_metadata(&complete, false).unwrap();
        assert!(validate_metadata(&complete, true).is_err());
        let silent = json!({"format":{"duration":"1.25"},"streams":[{"codec_type":"video","codec_name":"h264","width":2560,"height":1440,"nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus","nb_read_packets":"10"}]});
        validate_metadata(&silent, true).unwrap();
        assert!(validate_metadata(&silent, false).is_err());
        for duration in ["0", "NaN", "-1", "unavailable"] {
            let mut damaged = complete.clone();
            damaged["format"]["duration"] = duration.into();
            assert!(validate_metadata(&damaged, false).is_err());
        }
        assert!(validate_metadata(&json!({"streams":[]}), true).is_err());
    }
}
