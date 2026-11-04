use super::*;
use crate::{process, state};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
            net::{UnixDatagram, UnixStream},
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(2);
fn capture(args: &[&str]) -> Result<String> {
    let output = process::checked_capture(
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        TIMEOUT,
        4 * 1024 * 1024,
    )?;
    String::from_utf8(output.stdout).map_err(|_| invalid("Invalid audio response"))
}
fn request(command: &str) -> Result<(String, String)> {
    let runtime = runtime()?;
    let path = runtime.join("EasyEffectsServer");
    let metadata = fs::symlink_metadata(&path)?;
    // SAFETY: geteuid takes no pointers.
    if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(invalid("EasyEffects socket is not owned by this user"));
    }
    let mut socket = UnixStream::connect(&path)?;
    socket.set_read_timeout(Some(Duration::from_millis(400)))?;
    socket.set_write_timeout(Some(Duration::from_millis(400)))?;
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: initialized credentials and exact structure length.
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    if cred.uid != metadata.uid() {
        return Err(invalid("EasyEffects peer ownership changed"));
    }
    let stat = fs::read_to_string(format!("/proc/{}/stat", cred.pid))?;
    let start = stat
        .rsplit_once(')')
        .and_then(|(_, s)| s.split_whitespace().nth(19))
        .ok_or_else(|| invalid("Invalid EasyEffects process identity"))?;
    let instance = format!("{}:{start}", cred.pid);
    socket.write_all(command.as_bytes())?;
    let mut data = Vec::new();
    loop {
        let mut chunk = [0; 64];
        let n = socket.read(&mut chunk).map_err(|e| {
            invalid(format!(
                "EasyEffects monitoring-volume API unavailable: {e}; install the reviewed API patch"
            ))
        })?;
        if n == 0 {
            return Err(invalid("Incomplete EasyEffects response"));
        }
        data.extend_from_slice(&chunk[..n]);
        if data.len() > 256 {
            return Err(invalid("Oversized EasyEffects response"));
        }
        if data.contains(&b'\n') || command == "get_microphone_monitoring\n" {
            break;
        }
    }
    Ok((String::from_utf8_lossy(&data).trim().into(), instance))
}
fn runtime() -> Result<PathBuf> {
    let path = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| invalid("XDG_RUNTIME_DIR is unavailable"))?;
    let m = fs::symlink_metadata(&path)?;
    // SAFETY: geteuid takes no pointers.
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err(invalid("Unsafe runtime directory"));
    }
    Ok(path)
}
fn directory() -> Result<PathBuf> {
    Ok(state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/transparency"))
}
fn config(root: &Path) -> Result<Config> {
    let user =
        state::xdg("XDG_CONFIG_HOME", ".config")?.join("desktop-foundation/transparency.json");
    let p = if user.exists() {
        user
    } else {
        root.join("config/transparency.json")
    };
    let c: Config = serde_json::from_slice(&read(&p)?)?;
    c.validate()?;
    Ok(c)
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !f.metadata()?.is_file() || f.metadata()?.len() > 1024 * 1024 {
        return Err(invalid("Invalid Transparency Mode state file"));
    }
    let mut data = Vec::new();
    f.take(1024 * 1024 + 1).read_to_end(&mut data)?;
    if data.len() > 1024 * 1024 {
        return Err(invalid("Transparency state too large"));
    }
    Ok(data)
}
struct Native {
    directory: PathBuf,
}
impl Native {
    fn new() -> Result<Self> {
        Ok(Self {
            directory: directory()?,
        })
    }
}
fn json_query(args: &[&str]) -> Result<Value> {
    Ok(serde_json::from_str(&capture(args)?)?)
}
fn select_microphone(info: &Value, sources: &Value, needle: &str) -> Result<String> {
    let sources = sources
        .as_array()
        .ok_or_else(|| invalid("Invalid source list"))?;
    let physical = |s: &&Value| {
        let name = s["name"].as_str().unwrap_or("");
        !name.is_empty()
            && !name.ends_with(".monitor")
            && s["properties"]["node.virtual"] != "true"
            && s["properties"]["device.class"] != "monitor"
    };
    let mut candidates: Vec<_> = sources
        .iter()
        .filter(physical)
        .filter(|s| {
            let name = s["name"].as_str().unwrap_or("");
            if needle.is_empty() {
                info["default_source_name"] == name
            } else {
                name.contains(needle)
            }
        })
        .collect();
    // A virtual default (for example an effects microphone) is common. Only
    // infer its hardware input when exactly one explicitly physical source exists.
    if needle.is_empty()
        && candidates.is_empty()
        && sources.iter().any(|s| {
            s["name"] == info["default_source_name"] && s["properties"]["node.virtual"] == "true"
        })
    {
        candidates = sources
            .iter()
            .filter(physical)
            .filter(|s| s["properties"]["device.class"] == "sound")
            .collect();
    }
    if candidates.len() != 1 {
        return Err(invalid(if needle.is_empty() {
            "Select a physical microphone as the default input, or configure microphone_match"
        } else {
            "Transparency Mode requires exactly one matching physical microphone"
        }));
    }
    if candidates[0]["mute"] == true {
        return Err(invalid(
            "Microphone is muted; Transparency Mode will not unmute recording input",
        ));
    }
    Ok(candidates[0]["name"].as_str().unwrap().into())
}
impl Audio for Native {
    fn monitor(&mut self) -> Result<Monitor> {
        let (gain, instance) = request("get_microphone_monitoring_volume\n")?;
        let gain: f64 = gain
            .parse()
            .map_err(|_| invalid("Invalid monitoring volume response"))?;
        let (enabled, other) = request("get_microphone_monitoring\n")?;
        if instance != other || !gain.is_finite() || !(0.0..=4.0).contains(&gain) {
            return Err(invalid(
                "EasyEffects monitoring state changed or is invalid",
            ));
        }
        let enabled = match enabled.as_str() {
            "1" => true,
            "2" => false,
            _ => return Err(invalid("Invalid monitoring state response")),
        };
        Ok(Monitor {
            instance,
            enabled,
            gain,
        })
    }
    fn devices(&mut self, needle: &str) -> Result<(String, String)> {
        let info = json_query(&["pactl", "--format=json", "info"])?;
        let sources = json_query(&["pactl", "--format=json", "list", "sources"])?;
        let microphone = select_microphone(&info, &sources, needle)?;
        let output = info["default_sink_name"]
            .as_str()
            .ok_or_else(|| invalid("No default playback device"))?;
        if output == "easyeffects_sink" {
            return Err(invalid(
                "Select the physical playback device before Transparency Mode",
            ));
        }
        Ok((microphone, output.into()))
    }
    fn streams(&mut self) -> Result<Vec<Stream>> {
        let v = json_query(&["pactl", "--format=json", "list", "sink-inputs"])?;
        let mut streams = Vec::new();
        for s in v
            .as_array()
            .ok_or_else(|| invalid("Invalid playback stream list"))?
        {
            let props = &s["properties"];
            let name = props["application.name"].as_str().unwrap_or("");
            let node = props["node.name"].as_str().unwrap_or("");
            if name.to_ascii_lowercase().contains("easy effects")
                || name.to_ascii_lowercase().contains("easyeffects")
                || node.starts_with("ee_")
                || node.starts_with("easyeffects")
            {
                continue;
            }
            let id = s["index"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid("Invalid playback stream ID"))?;
            let serial = props["object.serial"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_digit()))
                .ok_or_else(|| invalid("Playback stream has no stable PipeWire serial"))?;
            let volume = s["volume"]
                .as_object()
                .ok_or_else(|| invalid("Invalid playback volumes"))?;
            // pactl's channel map order is authoritative; JSON object order is not.
            let map = s["channel_map"]
                .as_str()
                .ok_or_else(|| invalid("Missing playback channel map"))?;
            let volumes: Vec<u32> = map
                .split(',')
                .map(|channel| {
                    volume[channel.trim()]["value"]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| invalid("Invalid playback channel volume"))
                })
                .collect::<Result<_>>()?;
            streams.push(Stream {
                id,
                serial: serial.into(),
                volumes,
            });
        }
        Ok(streams)
    }
    fn set_enabled(&mut self, value: bool) -> Result<()> {
        let (answer, _) = request(if value {
            "microphone_monitoring:1\n"
        } else {
            "microphone_monitoring:0\n"
        })?;
        if answer != "ok" {
            return Err(invalid("Monitoring toggle was not acknowledged"));
        }
        Ok(())
    }
    fn set_gain(&mut self, value: f64) -> Result<()> {
        let (answer, _) = request(&format!("microphone_monitoring_volume:{value:.17}\n"))?;
        if answer != "ok" {
            return Err(invalid(format!("Monitoring volume rejected: {answer}")));
        }
        Ok(())
    }
    fn set_stream(&mut self, expected: &Stream, stream: &Stream) -> Result<()> {
        // Share the existing Control Center/keyboard audio lock, without changing master/input gain.
        let _lock = state::lock(&runtime()?.join("desktop-foundation-volume.lock"), TIMEOUT)?;
        let live = self
            .streams()?
            .into_iter()
            .find(|s| s.id == stream.id && s.serial == stream.serial)
            .ok_or_else(|| invalid("Playback stream disappeared"))?;
        if live.volumes.len() != stream.volumes.len() {
            return Err(invalid("Playback channel map changed"));
        }
        if &live != expected {
            return Err(invalid("Playback volume changed externally"));
        }
        let mut args = vec![
            "pactl".into(),
            "set-sink-input-volume".into(),
            stream.id.to_string(),
        ];
        args.extend(stream.volumes.iter().map(u32::to_string));
        process::checked_capture(&args, TIMEOUT, 4096)?;
        Ok(())
    }
    fn verify_path(&mut self, microphone: &str, output: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_millis(600);
        loop {
            let dump = json_query(&["pw-dump"])?;
            if path_ready(&dump, microphone, output) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(invalid(
                    "EasyEffects monitoring path from the selected microphone to the current playback device was not established",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn persist(&mut self, journal: Option<&Journal>) -> Result<()> {
        state::private_directory(&self.directory)?;
        let path = self.directory.join("journal.json");
        if let Some(j) = journal {
            state::atomic(&path, &serde_json::to_vec(j)?, 0o600)
        } else {
            match fs::remove_file(&path) {
                Ok(()) => {
                    fs::File::open(&self.directory)?.sync_all()?;
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            }
        }
    }
    fn wait(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}
/// Prove the monitor branch, not a recording/application route, reaches the physical sink.
pub fn path_ready(dump: &Value, microphone: &str, output: &str) -> bool {
    let Some(rows) = dump.as_array() else {
        return false;
    };
    let node = |name: &str| {
        rows.iter()
            .find(|r| {
                r["type"] == "PipeWire:Interface:Node" && r["info"]["props"]["node.name"] == name
            })
            .and_then(|r| r["id"].as_u64())
    };
    let (Some(source), Some(sink), Some(monitor)) = (
        node(microphone),
        node(output),
        node("ee_mic_monitor_output_level"),
    ) else {
        return false;
    };
    let edges: Vec<(u64, u64)> = rows
        .iter()
        .filter(|r| {
            r["type"] == "PipeWire:Interface:Link"
                && ["active", "paused"].contains(&r["info"]["state"].as_str().unwrap_or(""))
        })
        .filter_map(|r| {
            Some((
                r["info"]["output-node-id"].as_u64()?,
                r["info"]["input-node-id"].as_u64()?,
            ))
        })
        .collect();
    fn reaches(edges: &[(u64, u64)], start: u64, end: u64, blocked: Option<u64>) -> bool {
        let mut todo = vec![start];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = todo.pop() {
            if Some(id) == blocked || !seen.insert(id) {
                continue;
            }
            if id == end {
                return true;
            }
            todo.extend(edges.iter().filter(|(a, _)| *a == id).map(|(_, b)| *b));
        }
        false
    }
    reaches(&edges, source, monitor, Some(sink)) && reaches(&edges, monitor, sink, None)
}
fn notice(root: &Path, message: &str) {
    let args = vec![
        "quickshell".into(),
        "ipc".into(),
        "--path".into(),
        root.join("shell").to_string_lossy().into_owned(),
        "call".into(),
        "foundation".into(),
        "showAudioNotice".into(),
        message.into(),
    ];
    let _ = process::checked_capture(&args, Duration::from_millis(300), 4096);
}
fn recover(a: &mut Native, fade: u64) -> Result<()> {
    let path = a.directory.join("journal.json");
    if path.try_exists()? {
        let mut journal: Journal = serde_json::from_slice(&read(&path)?)?;
        restore(a, &mut journal, fade)?;
    }
    Ok(())
}
struct Subscription {
    child: Child,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Duck,
    Monitor,
}
impl Mode {
    fn off_notice(self) -> &'static str {
        match self {
            Self::Duck => "Transparency Off",
            Self::Monitor => "Monitoring Off",
        }
    }
}
fn toggle_mode(desired: &mut Option<Mode>, message: &[u8]) {
    let mode = match message {
        b"toggle" => Mode::Duck,
        b"monitor" => Mode::Monitor,
        _ => return,
    };
    *desired = if *desired == Some(mode) {
        None
    } else {
        Some(mode)
    };
}
fn serve(root: &Path) -> Result<()> {
    let pid = std::process::id().to_string();
    if env::var("LISTEN_PID").as_deref() != Ok(&pid) || env::var("LISTEN_FDS").as_deref() != Ok("1")
    {
        return Err(invalid("Transparency owner requires its systemd socket"));
    }
    // SAFETY: fd3 is the one owned socket supplied by systemd to this process.
    let socket = unsafe { UnixDatagram::from_raw_fd(3) };
    socket.set_nonblocking(true)?;
    // Consume activation messages before fallible setup/recovery. A rejected
    // request must not remain queued and repeatedly reactivate a failing unit.
    let mut desired = None;
    let mut initial = [0; 32];
    loop {
        match socket.recv(&mut initial) {
            Ok(n) => toggle_mode(&mut desired, &initial[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e.into()),
        }
    }
    let config = config(root)?;
    let mut audio = Native::new()?;
    state::private_directory(&audio.directory)?;
    let _owner = state::lock(
        &audio.directory.join("owner.lock"),
        Duration::from_millis(100),
    )?;
    recover(&mut audio, config.fade_ms)?;
    let mut subscription = Subscription {
        child: Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    };
    let mut events = subscription
        .child
        .stdout
        .take()
        .ok_or_else(|| invalid("Missing PipeWire event stream"))?;
    // SAFETY: fcntl only modifies the owned event-pipe descriptor.
    if unsafe { libc::fcntl(events.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut journal = None;
    let mut active = None;
    let mut event_buffer = String::new();
    let result = (|| -> Result<()> {
        loop {
            let mut message = [0; 32];
            loop {
                match socket.recv(&mut message) {
                    Ok(n) => toggle_mode(&mut desired, &message[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e.into()),
                }
            }
            if active != desired {
                let previous = active;
                if let Some(j) = &mut journal {
                    restore(&mut audio, j, config.fade_ms)?;
                }
                journal = None;
                active = desired;
                if let Some(mode) = active {
                    journal = Some(activate_mode(&mut audio, &config, mode == Mode::Duck)?);
                    notice(
                        root,
                        if mode == Mode::Duck {
                            "Transparency On"
                        } else {
                            "Monitoring On"
                        },
                    );
                } else {
                    if let Some(mode) = previous {
                        notice(root, mode.off_notice());
                    }
                    return Ok(());
                }
            }
            if active.is_none() {
                return Ok(());
            }
            let mut fds = [
                libc::pollfd {
                    fd: socket.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: events.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            // SAFETY: exact initialized descriptor array; no active-mode polling timer.
            let polled = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
            if polled < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.into());
            }
            if fds[1].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                return Err(invalid("PipeWire event connection closed"));
            }
            if fds[1].revents & libc::POLLIN != 0 {
                let mut buf = [0; 8192];
                while let Ok(n) = events.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    event_buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if event_buffer.len() > 65536 {
                        return Err(invalid("Audio event backlog exceeded"));
                    }
                }
                let mut relevant = false;
                while let Some(end) = event_buffer.find('\n') {
                    let line = event_buffer.drain(..=end).collect::<String>();
                    relevant |= ["on sink-input", "on source #", "on sink #", "on server #"]
                        .iter()
                        .any(|kind| line.contains(kind));
                }
                if relevant && let Some(j) = &mut journal {
                    let devices = audio.devices(&config.microphone_match)?;
                    if devices != (j.microphone.clone(), j.output.clone()) {
                        return Err(invalid("Transparency audio device changed"));
                    }
                    let monitor = audio.monitor()?;
                    if monitor.instance != j.monitor_instance || !monitor.enabled {
                        return Err(invalid("EasyEffects monitoring stopped"));
                    }
                    audio.verify_path(&j.microphone, &j.output)?;
                    if active == Some(Mode::Duck) {
                        reconcile(&mut audio, j, &config)?;
                    }
                }
            }
        }
    })();
    if let Err(error) = result {
        if let Some(j) = &mut journal {
            let _ = restore(&mut audio, j, 0);
        }
        notice(root, "Transparency unavailable");
        return Err(error);
    }
    Ok(())
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    // SAFETY: no pointer arguments.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Run Transparency Mode as your normal desktop user"));
    }
    match args.first().map(String::as_str) {
        Some("toggle" | "monitor") if args.len() == 1 => {
            capture(&[
                "systemctl",
                "--user",
                "start",
                "desktop-foundation-transparency.socket",
            ])?;
            let socket = UnixDatagram::unbound()?;
            socket.set_write_timeout(Some(Duration::from_secs(1)))?;
            socket.connect(runtime()?.join("desktop-foundation-transparency.sock"))?;
            socket.send(args[0].as_bytes())?;
            Ok(())
        }
        Some("serve") if args.len() == 1 => {
            let result = serve(root);
            if result.is_err() {
                notice(root, "Transparency unavailable");
            }
            result
        }
        Some("recover") if args.len() == 1 => {
            let mut audio = Native::new()?;
            state::private_directory(&audio.directory)?;
            let _owner = state::lock(&audio.directory.join("owner.lock"), Duration::from_secs(3))?;
            recover(&mut audio, 0)
        }
        Some("check") if args.len() == 1 => {
            let c = config(root)?;
            let mut a = Native::new()?;
            let (microphone, output) = a.devices(&c.microphone_match)?;
            let monitor = a.monitor()?;
            println!(
                "{}",
                json!({"available":true,"microphone":microphone,"output":output,"monitoring":monitor.enabled,"monitoringGain":monitor.gain})
            );
            Ok(())
        }
        _ => Err(invalid(
            "Expected transparency toggle|monitor|check|serve|recover",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcuts_toggle_their_mode_and_switch_between_modes() {
        let mut mode = None;
        toggle_mode(&mut mode, b"monitor");
        assert_eq!(mode, Some(Mode::Monitor));
        toggle_mode(&mut mode, b"toggle");
        assert_eq!(mode, Some(Mode::Duck));
        toggle_mode(&mut mode, b"monitor");
        assert_eq!(mode, Some(Mode::Monitor));
        toggle_mode(&mut mode, b"unknown");
        assert_eq!(mode, Some(Mode::Monitor));
        toggle_mode(&mut mode, b"monitor");
        assert_eq!(mode, None);
    }
    #[test]
    fn each_mode_has_its_own_off_notice() {
        assert_eq!(Mode::Duck.off_notice(), "Transparency Off");
        assert_eq!(Mode::Monitor.off_notice(), "Monitoring Off");
    }
    #[test]
    fn selects_default_or_unique_explicit_physical_input() {
        let info = json!({"default_source_name":"mic-b"});
        let sources = json!([
            {"name":"mic-a", "mute":false},
            {"name":"mic-b", "mute":false},
            {"name":"virtual", "properties":{"node.virtual":"true"}},
            {"name":"sink.monitor"}
        ]);
        assert_eq!(select_microphone(&info, &sources, "").unwrap(), "mic-b");
        assert_eq!(
            select_microphone(&info, &sources, "mic-a").unwrap(),
            "mic-a"
        );
        assert!(select_microphone(&info, &sources, "mic-").is_err());
        assert!(
            select_microphone(&json!({"default_source_name":"virtual"}), &sources, "").is_err()
        );
        assert!(select_microphone(&info, &json!([{"name":"mic-b", "mute":true}]), "").is_err());
    }
    #[test]
    fn virtual_default_uses_only_unambiguous_physical_input() {
        let info = json!({"default_source_name":"effects"});
        let virtual_mic = json!({"name":"effects", "properties":{"node.virtual":"true"}});
        let mic = json!({"name":"usb-mic", "mute":false, "properties":{"device.class":"sound"}});
        let other =
            json!({"name":"other-mic", "mute":false, "properties":{"device.class":"sound"}});
        assert_eq!(
            select_microphone(&info, &json!([virtual_mic, mic]), "").unwrap(),
            "usb-mic"
        );
        assert!(select_microphone(&info, &json!([virtual_mic, mic, other]), "").is_err());
        assert!(select_microphone(&json!({}), &json!([mic]), "").is_err());
    }
    #[test]
    fn graph_proof_requires_the_dedicated_monitor_branch() {
        let node = |id, name| json!({"id":id,"type":"PipeWire:Interface:Node","info":{"props":{"node.name":name}}});
        let link = |from, to| json!({"type":"PipeWire:Interface:Link","info":{"state":"active","output-node-id":from,"input-node-id":to}});
        let mut rows = vec![
            node(1, "tc30"),
            node(2, "ee_sie_output_level"),
            node(3, "ee_mic_monitor_output_level"),
            node(4, "daybreak"),
            link(1, 2),
            link(2, 3),
            link(3, 4),
        ];
        assert!(path_ready(&json!(rows), "tc30", "daybreak"));
        rows.pop();
        assert!(!path_ready(&json!(rows), "tc30", "daybreak"));
        rows.push(link(2, 4));
        assert!(!path_ready(&json!(rows), "tc30", "daybreak"));
        assert!(!path_ready(
            &json!(rows),
            "different-microphone",
            "daybreak"
        ));
    }
}
