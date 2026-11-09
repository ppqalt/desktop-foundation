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
        let mut streams = vec![
            json!({"codec_type":"video", "codec_name":"h264", "width":2560, "height":1440, "nb_read_packets":"10"}),
        ];
        streams.push(json!({"codec_type":"audio", "codec_name":"opus", "nb_read_packets":"10"}));
        if !silent {
            streams
                .push(json!({"codec_type":"audio", "codec_name":"opus", "nb_read_packets":"10"}));
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
fn normal_shortcut_stops_desktop_only_capture_and_indicator_waits_for_finalization() {
    let mut f = Fixture::new(true);
    f.start(true);
    f.wait_phase("recording");
    let arguments = fs::read_to_string(f.root.join("arguments")).unwrap();
    assert_eq!(arguments.lines().filter(|s| *s == "-a").count(), 1);
    assert!(arguments.lines().any(|s| s == "default_output"));
    assert!(!arguments.lines().any(|s| s == "default_input"));
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

fn replay_fixture(f: &Fixture) {
    f.script("gpu-screen-recorder", r##"#!/usr/bin/python3
import json,os,signal,socket,sys,time,threading
if sys.argv[1:] == ['--help']:
 print('-write-first-frame-ts -fallback-cpu-encoding -portal-session-token-filepath -ab -ipc -ro -replay-storage -bm')
 sys.exit(0)
args=sys.argv[1:]
def value(k): return args[args.index(k)+1]
root=os.environ['DF_RECORDING_FIXTURE_ROOT']; open(root+'/arguments','w').write('\n'.join(args))
if '-ipc' not in args:
 open(root+'/desktop-arguments','w').write('\n'.join(args))
 out=value('-o'); open(out,'wb').write(b'fixture media')
 signal.signal(signal.SIGINT,lambda *_: sys.exit(0))
 open(out+'.ts','w').write('monotonic_microsec realtime_microsec\n100 200\n')
 while True: time.sleep(.02)
path=value('-ipc'); out=value('-o'); listener=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); listener.bind(path);listener.listen()
open(root+'/replay-pid','w').write(str(os.getpid()))
time.sleep(.25)
print('gsr info: update fps: 60, damage fps: 60',file=sys.stderr,flush=True)
def display_changes():
 while not os.path.exists(root+'/display-changed'): time.sleep(.02)
 print('gsr info: pipewire: negotiated format:',file=sys.stderr,flush=True)
threading.Thread(target=display_changes,daemon=True).start()
counter=0
signal.signal(signal.SIGINT,lambda *_: sys.exit(0))
while True:
 c,_=listener.accept()
 with c:
  data=b''
  while b'\n' not in data: data+=c.recv(4096)
  req=json.loads(data); action=req['name']; answer={'id':1,'result':'ok'}
  with open(root+'/ipc-requests','a') as log: log.write(json.dumps(req)+'\n')
  if action in ('save-replay','stop-replay-recording'):
   counter+=1;name=('Replay_' if action=='save-replay' else 'Video_')+str(counter)+'.mkv'
   file=os.path.join(out,name);open(file,'wb').write(b'fixture media');answer['data']=file
   if action=='stop-replay-recording': open(file+'.ts','w').write('retained marker')
  c.sendall((json.dumps(answer)+'\n').encode())
"##);
}

fn wait_files(dir: &std::path::Path, count: usize) {
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        if fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "mkv"))
            .count()
            >= count
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("Expected {count} saved recordings");
}

#[test]
fn replay_and_regular_recording_share_one_owned_process() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    let arguments = fs::read_to_string(f.root.join("arguments")).unwrap();
    assert!(arguments.contains("-r\n300\n"));
    assert!(arguments.contains("-bm\ncbr\n"));
    assert_eq!(arguments.lines().filter(|s| *s == "-a").count(), 2);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.client("replay-save");
    wait_files(&f.root.join("videos"), 1);
    f.client("toggle");
    f.wait_phase("replay-recording");
    f.client("replay-save");
    wait_files(&f.root.join("videos"), 2);
    f.client("toggle");
    f.wait_phase("replay");
    wait_files(&f.root.join("videos"), 3);
    for entry in fs::read_dir(f.root.join("videos"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "mkv"))
    {
        assert_eq!(
            entry.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            entry.file_name().to_string_lossy().starts_with("Replay-")
                || entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Recording-")
        );
    }
    f.client("replay-toggle");
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
}

#[test]
fn early_replay_save_waits_until_the_buffer_has_frames() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-save",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    wait_files(&f.root.join("videos"), 1);
    f.client("replay-toggle");
    assert!(f.wait_exit());
}

#[test]
fn display_renegotiation_saves_active_recording_and_stops_replay() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    f.client("toggle");
    f.wait_phase("replay-recording");
    fs::write(f.root.join("display-changed"), "").unwrap();
    assert!(!f.wait_exit());
    assert_eq!(f.state()["phase"], "error");
    assert!(
        f.state()["error"]
            .as_str()
            .unwrap()
            .contains("Display capture format changed")
    );
    wait_files(&f.root.join("videos"), 1);
}

#[test]
fn crashed_replay_is_reported_and_does_not_silently_restart() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    let pid: i32 = fs::read_to_string(f.root.join("replay-pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(pid > 1);
    // SAFETY: pid is the live private test recorder created by this fixture.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    assert!(!f.wait_exit());
    assert_eq!(f.state()["phase"], "error");
    assert!(
        f.state()["error"]
            .as_str()
            .unwrap()
            .contains("partial files retained")
    );
}

#[test]
fn desktop_only_shortcut_pauses_replay_without_recording_microphone_then_resumes_it() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    fs::write(f.root.join("probe.json"),json!({"format":{"duration":"1.5"},"streams":[{"codec_type":"video","codec_name":"h264","width":2560,"height":1440,"nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus", "nb_read_packets":"10"}]}).to_string()).unwrap();
    f.client("silent");
    f.wait_phase("recording");
    let args = fs::read_to_string(f.root.join("desktop-arguments")).unwrap();
    assert_eq!(args.lines().filter(|v| *v == "-a").count(), 1);
    assert!(args.lines().any(|v| v == "default_output"));
    assert!(!args.lines().any(|v| v == "default_input"));
    f.client("toggle");
    f.wait_phase("replay");
    wait_files(&f.root.join("videos"), 1);
    assert!(
        fs::read_dir(f.root.join("videos"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().starts_with("Desktop-"))
    );
    f.client("replay-toggle");
    assert!(f.wait_exit());
}

#[test]
fn invalid_replay_is_retained_privately_and_never_reported_saved() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    f.script(
        "notify-send",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DF_RECORDING_FIXTURE_ROOT/notices\"\n",
    );
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    fs::write(f.root.join("probe.json"), "{\"streams\":[]}").unwrap();
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    f.client("replay-save");
    let until = Instant::now() + Duration::from_secs(8);
    while !f.root.join("notices").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    let notices = fs::read_to_string(f.root.join("notices")).unwrap();
    assert!(notices.contains("Replay not saved"), "{notices}");
    assert!(!notices.contains("Saved "), "{notices}");
    assert_eq!(
        fs::read_dir(f.root.join("videos"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "mkv"))
            .count(),
        0
    );
    assert!(
        fs::read_dir(f.root.join("videos"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".foundation-replay-"))
    );
    f.client("replay-toggle");
    assert!(f.wait_exit());
}

#[test]
fn service_shutdown_gives_owner_time_to_finalize_its_recorder() {
    let definitions = desktop_foundationctl::deploy_units::definitions(
        std::path::Path::new("/checkout"),
        std::path::Path::new("/state"),
    );
    let service = &definitions
        .iter()
        .find(|(name, _)| name == "desktop-foundation-recording.service")
        .unwrap()
        .1;
    assert!(service.contains("KillMode=mixed\n"));
    assert!(service.contains("KillSignal=SIGINT\n"));
    assert!(service.contains("TimeoutStopSec=75\n"));
}

#[test]
fn shutdown_saves_active_regular_recording_and_stops_replay_cleanly() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    f.client("toggle");
    f.wait_phase("replay-recording");
    let pid = f.owner.as_ref().unwrap().id() as i32;
    // SAFETY: the owner is the private child started by this fixture.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
    wait_files(&f.root.join("videos"), 1);
}

#[test]
fn shutdown_during_desktop_only_does_not_restart_replay() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-toggle",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    fs::write(f.root.join("probe.json"), json!({"format":{"duration":"1.5"},"streams":[{"codec_type":"video","codec_name":"h264","width":2560,"height":1440,"nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus","nb_read_packets":"10"}]}).to_string()).unwrap();
    f.client("silent");
    f.wait_phase("recording");
    let pid = f.owner.as_ref().unwrap().id() as i32;
    // SAFETY: the owner is the private child started by this fixture.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
    wait_files(&f.root.join("videos"), 1);
}

#[test]
fn replay_state_reports_buffering_then_ready_and_validated_saves() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    let config = f.root.join("config/recording.toml");
    let contents = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{contents}replay_seconds = 2\n")).unwrap();
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-on",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    assert_eq!(f.state()["replay_status"], "buffering");
    f.client("replay-save");
    wait_files(&f.root.join("videos"), 1);
    assert_eq!(f.state()["save_status"], "saved");
    let until = Instant::now() + Duration::from_secs(5);
    while f.state()["replay_status"] != "ready" && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(f.state()["replay_status"], "ready");
    f.client("replay-on");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(f.state()["replay_status"], "ready");
    f.client("replay-off");
    assert!(f.wait_exit());
    assert_eq!(f.state()["replay_status"], "off");
}

#[test]
fn desktop_only_pauses_replay_and_explicit_off_prevents_resume() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.socket
        .send_to(
            b"replay-on",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    f.launch_owner();
    f.wait_phase("replay");
    fs::write(f.root.join("probe.json"),json!({"format":{"duration":"1.5"},"streams":[{"codec_type":"video","codec_name":"h264","width":2560,"height":1440,"nb_read_packets":"10"},{"codec_type":"audio","codec_name":"opus","nb_read_packets":"10"}]}).to_string()).unwrap();
    f.client("silent");
    f.wait_phase("recording");
    assert_eq!(f.state()["replay_status"], "paused");
    f.client("replay-off");
    let until = Instant::now() + Duration::from_secs(3);
    while f.state()["replay_status"] != "off" && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.state()["replay_status"], "off");
    f.client("toggle");
    assert!(f.wait_exit());
    assert_eq!(f.state()["phase"], "idle");
}

#[test]
fn status_rejects_invalid_settings_instead_of_inventing_a_buffer_duration() {
    let f = Fixture::new(false);
    fs::write(
        f.root.join("config/recording.toml"),
        "replay_seconds = -1\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
    cmd.arg("--root").arg(&f.root).args(["recording", "status"]);
    f.configure(&mut cmd);
    assert!(!cmd.status().unwrap().success());
}

#[test]
fn editor_snapshot_pins_footage_without_interrupting_or_restarting_capture() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    f.client("replay-on");
    f.launch_owner();
    f.wait_phase("replay");
    f.client("toggle");
    f.wait_phase("replay-recording");
    let pid = fs::read_to_string(f.root.join("replay-pid")).unwrap();
    let id = "0123456789abcdef00000001";
    let session = f
        .root
        .join("state/desktop-foundation/replay-editor")
        .join(id);
    fs::create_dir_all(&session).unwrap();
    fs::set_permissions(&session, fs::Permissions::from_mode(0o700)).unwrap();
    f.socket
        .send_to(
            format!("edit:{id}").as_bytes(),
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !session.join("capture.json").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(session.join("source.mkv").is_file());
    assert_eq!(
        fs::read(session.join("source.mkv")).unwrap(),
        b"fixture media"
    );
    assert_eq!(f.state()["phase"], "replay-recording");
    assert_eq!(fs::read_to_string(f.root.join("replay-pid")).unwrap(), pid);
    let requests = fs::read_to_string(f.root.join("ipc-requests")).unwrap();
    assert!(
        requests
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .any(|r| r["name"] == "save-replay" && r["data"]["restart-replay"] == false)
    );
    f.client("toggle");
    f.wait_phase("replay");
    f.client("replay-save");
    wait_files(&f.root.join("videos"), 2);
    assert_eq!(fs::read_to_string(f.root.join("replay-pid")).unwrap(), pid);
    assert!(session.join("source.mkv").exists());
    f.client("replay-off");
    assert!(f.wait_exit());
}
#[test]
fn automatic_replay_can_be_explicitly_disabled_without_starting_capture() {
    let f = Fixture::new(false);
    let path = f.root.join("config/recording.toml");
    let config = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{config}replay_auto_start = false\n")).unwrap();
    f.client("autostart");
    assert!(!f.root.join("arguments").exists());
}

#[test]
fn a_discarded_editor_session_does_not_stop_the_recorder() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    f.client("replay-on");
    f.launch_owner();
    f.wait_phase("replay");
    let pid = fs::read_to_string(f.root.join("replay-pid")).unwrap();
    f.socket
        .send_to(
            b"edit:0123456789abcdef00000001",
            f.root
                .join("runtime/desktop-foundation-recording/requests.sock"),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !f.root.join("ipc-requests").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(f.root.join("ipc-requests").exists());
    assert!(f.owner.as_mut().unwrap().try_wait().unwrap().is_none());
    assert_eq!(f.state()["phase"], "replay");
    assert_eq!(fs::read_to_string(f.root.join("replay-pid")).unwrap(), pid);
    f.client("replay-off");
    assert!(f.wait_exit());
}

#[test]
fn direct_editor_capture_bypasses_legacy_drafts_and_replaces_only_its_workspace() {
    let mut f = Fixture::new(false);
    replay_fixture(&f);
    let probe_path = f.root.join("probe.json");
    let mut probe: Value = serde_json::from_slice(&fs::read(&probe_path).unwrap()).unwrap();
    probe["format"]["format_name"] = "matroska,webm".into();
    fs::write(probe_path, serde_json::to_vec(&probe).unwrap()).unwrap();
    f.script(
        "ffmpeg",
        "#!/bin/sh\nfor last do :; done\nprintf thumbnail > \"$last\"\n",
    );
    fs::remove_file(f.root.join("block-finalization")).unwrap();
    let base = f.root.join("state/desktop-foundation/replay-editor");
    for id in [
        "0123456789abcdef00000001",
        "0123456789abcdef00000002",
        "0123456789abcdef00000003",
    ] {
        fs::create_dir_all(base.join(id)).unwrap();
        fs::set_permissions(base.join(id), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(base.join(id).join("legacy-note"), "keep").unwrap();
    }
    f.client("replay-on");
    f.launch_owner();
    f.wait_phase("replay");
    let pid = fs::read_to_string(f.root.join("replay-pid")).unwrap();
    let capture = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        cmd.arg("--root")
            .arg(&f.root)
            .args(["recording", "editor", "capture"]);
        f.configure(&mut cmd);
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let first = capture();
    assert!(base.join(&first).join("source.mkv").exists());
    f.script("ffmpeg", "#!/bin/sh\nexit 1\n");
    let mut failed = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
    failed
        .arg("--root")
        .arg(&f.root)
        .args(["recording", "editor", "capture"]);
    f.configure(&mut failed);
    assert!(!failed.status().unwrap().success());
    assert!(base.join(&first).join("source.mkv").exists());
    f.script(
        "ffmpeg",
        "#!/bin/sh\nfor last do :; done\nprintf thumbnail > \"$last\"\n",
    );
    let second = capture();
    assert_ne!(first, second);
    assert!(!base.join(&first).exists());
    assert!(base.join(&second).join("source.mkv").exists());
    for id in [
        "0123456789abcdef00000001",
        "0123456789abcdef00000002",
        "0123456789abcdef00000003",
    ] {
        assert_eq!(
            fs::read_to_string(base.join(id).join("legacy-note")).unwrap(),
            "keep"
        );
    }
    assert_eq!(fs::read_to_string(f.root.join("replay-pid")).unwrap(), pid);
    assert_eq!(f.state()["phase"], "replay");
    f.client("replay-off");
    assert!(f.wait_exit());
}
