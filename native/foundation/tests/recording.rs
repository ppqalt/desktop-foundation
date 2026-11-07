//! Synthetic lifecycle fixtures only: no real capture, microphone, GPU or session bus.
use serde_json::{Value, json};
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, PermissionsExt},
            net::UnixDatagram,
            process::CommandExt,
        },
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn clock_indicator_and_user_override_follow_published_state_offscreen() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Skipping recording-state QML fixture: Quickshell unavailable");
        return;
    }
    let runner =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-recording-state");
    let output = Command::new(runner).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    root: PathBuf,
    socket: UnixDatagram,
    owner: Option<Child>,
}
impl Fixture {
    fn new(silent: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "df-recorder-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .unwrap();
        for name in [
            "config",
            "bin",
            "runtime",
            "runtime/desktop-foundation-recording",
            "state",
            "user-config",
            "videos",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(
            root.join("config/recording.toml"),
            format!(
                "directory = {}\nstartup_timeout_seconds = 10\nstop_timeout_seconds = 5\n",
                serde_json::to_string(&root.join("videos")).unwrap()
            ),
        )
        .unwrap();
        let mut streams = vec![json!({"codec_type":"video"})];
        if !silent {
            streams.extend([json!({"codec_type":"audio"}), json!({"codec_type":"audio"})]);
        }
        fs::write(
            root.join("probe.json"),
            json!({"format":{"duration":"1.5"},"streams":streams}).to_string(),
        )
        .unwrap();
        fs::write(root.join("block-finalization"), "").unwrap();
        let socket =
            UnixDatagram::bind(root.join("runtime/desktop-foundation-recording/requests.sock"))
                .unwrap();
        let fixture = Self {
            root,
            socket,
            owner: None,
        };
        fixture.script(
            "gpu-screen-recorder",
            r#"#!/bin/sh
if [ "$1" = --help ]; then
    echo '-write-first-frame-ts -fallback-cpu-encoding -portal-session-token-filepath -ab'
    exit 0
fi
printf '%s\n' "$@" > "$DF_RECORDING_FIXTURE_ROOT/arguments"
while [ "$#" -gt 0 ]; do
    if [ "$1" = -o ]; then shift; output=$1; fi
    shift
done
if [ -e "$DF_RECORDING_FIXTURE_ROOT/wait-for-source" ]; then
    trap '' INT TERM
    : > "$DF_RECORDING_FIXTURE_ROOT/prompt-ready"
    while :; do /bin/sleep 0.02; done
fi
printf 'synthetic video fixture\n' > "$output"
trap 'printf finished >> "$output"; exit 0' INT TERM
if [ -e "$DF_RECORDING_FIXTURE_ROOT/legacy-marker" ]; then
    printf '100 200\n' > "$output.ts"
else
    printf 'monotonic_microsec\trealtime_microsec\n100\t200\n' > "$output.ts"
fi
if [ -e "$DF_RECORDING_FIXTURE_ROOT/finish-immediately" ]; then exit 0; fi
while :; do /bin/sleep 0.02; done
"#,
        );
        fixture.script(
            "ffprobe",
            r#"#!/bin/sh
: > "$DF_RECORDING_FIXTURE_ROOT/probe-started"
while [ -e "$DF_RECORDING_FIXTURE_ROOT/block-finalization" ]; do /bin/sleep 0.02; done
/bin/cat "$DF_RECORDING_FIXTURE_ROOT/probe.json"
"#,
        );
        fixture.script("niri", "#!/bin/sh\nprintf '%s\\n' '{\"name\":\"Synthetic-7\",\"current_mode\":0,\"modes\":[{\"width\":2560,\"height\":1440,\"refresh_rate\":144000}]}'\n");
        fixture.script("systemctl", "#!/bin/sh\nexit 0\n");
        fixture.script("notify-send", "#!/bin/sh\nexit 0\n");
        fixture
    }
    fn script(&self, name: &str, text: &str) {
        let path = self.root.join("bin").join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn configure(&self, command: &mut Command) {
        command
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("user-config"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.join("bin").display()),
            )
            .env("DF_RECORDING_FIXTURE_ROOT", &self.root)
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("NIRI_SOCKET")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
    }
    fn start(&mut self, silent: bool) {
        self.socket
            .send_to(
                if silent { b"silent" } else { b"toggle" },
                self.root
                    .join("runtime/desktop-foundation-recording/requests.sock"),
            )
            .unwrap();
        self.launch_owner();
    }
    fn launch_owner(&mut self) {
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "export LISTEN_PID=$$ LISTEN_FDS=1 INVOCATION_ID=synthetic; exec \"$@\"",
                "fixture",
            ])
            .arg(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&self.root)
            .args(["recording", "serve"])
            .process_group(0);
        self.configure(&mut command);
        let fd = self.socket.as_raw_fd();
        // SAFETY: only async-signal-safe descriptor operations before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        self.owner = Some(command.spawn().unwrap());
    }
    fn client(&self, action: &str) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        cmd.arg("--root")
            .arg(&self.root)
            .args(["recording", action]);
        self.configure(&mut cmd);
        assert!(cmd.status().unwrap().success());
    }
    fn state(&self) -> Value {
        fs::read(
            self.root
                .join("runtime/desktop-foundation-recording/status.json"),
        )
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
    }
    fn wait_phase(&self, phase: &str) {
        let until = Instant::now() + Duration::from_secs(8);
        while Instant::now() < until {
            if self.state()["phase"] == phase {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("Did not reach {phase}: {}", self.state());
    }
    fn wait_exit(&mut self) -> bool {
        let until = Instant::now() + Duration::from_secs(8);
        while Instant::now() < until {
            if let Some(status) = self.owner.as_mut().unwrap().try_wait().unwrap() {
                return status.success();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("Synthetic recording owner did not exit");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.owner
            && child.try_wait().ok().flatten().is_none()
        {
            // SAFETY: own unreaped child's private process group, never a live recorder.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn normal_shortcut_stops_silent_capture_and_indicator_waits_for_finalization() {
    let mut f = Fixture::new(true);
    f.start(true);
    f.wait_phase("recording");
    let arguments = fs::read_to_string(f.root.join("arguments")).unwrap();
    assert!(!arguments.lines().any(|s| s == "-a"));
    assert!(!arguments.lines().any(|s| s == "-s"));
    f.client("toggle");
    f.wait_phase("stopping");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(f.state()["phase"], "stopping");
    assert!(f.owner.as_mut().unwrap().try_wait().unwrap().is_none());
    // Extra stop presses must not be replayed as a fresh socket activation.
    f.client("toggle");
    f.client("silent");
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
    assert!(PathBuf::from(f.state()["output"].as_str().unwrap()).is_file());
    f.socket.set_nonblocking(true).unwrap();
    assert_eq!(
        f.socket.recv(&mut [0; 32]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn normal_capture_has_independent_tracks_and_failure_stays_visible_until_acknowledged() {
    let mut f = Fixture::new(false);
    f.start(false);
    f.wait_phase("recording");
    let arguments = fs::read_to_string(f.root.join("arguments")).unwrap();
    assert_eq!(arguments.lines().filter(|s| *s == "-a").count(), 2);
    fs::write(f.root.join("probe.json"), "{\"streams\":[]}").unwrap();
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.client("toggle");
    assert!(!f.wait_exit());
    assert_eq!(f.state()["phase"], "error");
    let file = PathBuf::from(f.state()["output"].as_str().unwrap());
    assert!(file.is_file());
    f.client("acknowledge");
    assert_eq!(f.state()["phase"], "idle");
    assert!(file.is_file());
}

#[test]
fn queued_stop_cancels_before_any_capture_process_starts() {
    let mut f = Fixture::new(false);
    let path = f
        .root
        .join("runtime/desktop-foundation-recording/requests.sock");
    f.socket.send_to(b"toggle", &path).unwrap();
    f.socket.send_to(b"toggle", &path).unwrap();
    f.launch_owner();
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
    assert!(!f.root.join("arguments").exists());
}

#[test]
fn first_frame_is_checked_even_when_the_synthetic_encoder_exits_immediately() {
    let mut f = Fixture::new(true);
    fs::write(f.root.join("finish-immediately"), "").unwrap();
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.start(true);
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
    assert!(f.root.join("probe-started").exists());
}

#[test]
fn legacy_numeric_marker_still_transitions_to_recording() {
    let mut f = Fixture::new(true);
    fs::write(f.root.join("legacy-marker"), "").unwrap();
    f.start(true);
    f.wait_phase("recording");
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.client("stop");
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
}

#[test]
fn concurrent_status_initialization_always_publishes_complete_json() {
    let f = Fixture::new(true);
    let mut children = Vec::new();
    for _ in 0..12 {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        cmd.arg("--root").arg(&f.root).args(["recording", "status"]);
        f.configure(&mut cmd);
        children.push(cmd.spawn().unwrap());
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    assert_eq!(f.state()["phase"], "idle");
}

#[test]
fn stopping_an_unstarted_portal_selector_cancels_without_waiting_for_the_save_timeout() {
    let mut f = Fixture::new(false);
    fs::write(f.root.join("wait-for-source"), "").unwrap();
    f.start(false);
    let until = Instant::now() + Duration::from_secs(3);
    while !f.root.join("prompt-ready").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(f.root.join("prompt-ready").exists());
    let stop = Instant::now();
    f.client("toggle");
    assert!(f.wait_exit());
    assert!(stop.elapsed() < Duration::from_secs(3));
    assert_eq!(f.state()["phase"], "idle");
    assert!(!f.root.join("probe-started").exists());
    assert!(!PathBuf::from(f.state()["output"].as_str().unwrap()).exists());
}
