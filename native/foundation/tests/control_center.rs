//! Inert audio and network fixtures; never calls the desktop services.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const FAKE: &str = r##"#!/bin/sh
name=${0##*/}
{ printf '%s' "$name"; for arg do printf '\t%s' "$arg"; done; printf '\n'; } >> "$DF_CONTROL_LOG"
[ "${DF_FAIL_CONTROL:-}" != "$name" ] || exit 8
case "$name $*" in
'pactl --format=json info') printf '%s\n' '{"default_sink_name":"speakers","default_source_name":"mic"}';;
'pactl --format=json list sinks') printf '%s\n' '[{"index":1,"name":"speakers","description":"Speakers","mute":false,"volume":{"mono":{"value":32768}}}]';;
'pactl --format=json list sources') printf '%s\n' '[{"index":1,"name":"mic","description":"Microphone","mute":false,"volume":{"mono":{"value":32768}}},{"name":"speakers.monitor"}]';;
'pactl --format=json list sink-inputs') printf '%s\n' '[{"index":42,"properties":{"object.serial":"100","application.name":"Music"},"volume":{"mono":{"value":32768}}},{"index":43,"properties":{"object.serial":"101","application.name":"Music","media.name":"Game"},"volume":{"mono":{"value":16384}}}]';;
'pactl --format=json list source-outputs') printf '%s\n' '[{"index":42}]';;
'nmcli '*general) printf '%s\n' 'enabled:enabled:connected:full';;
'nmcli '*device\ status) printf '%s\n' 'enp4s0:ethernet:connected:Office\:LAN' 'wlan0:wifi:disconnected:';;
'nmcli '*connection\ show) printf '%s\n' 'Office\:LAN:12345678-abcd-1234-abcd-123456789abc:802-3-ethernet:enp4s0';;
esac
"##;
struct Fixture {
    base: PathBuf,
    log: PathBuf,
    bin: PathBuf,
    runtime: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-control-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let bin = base.join("bin");
        let runtime = base.join("runtime");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["pactl", "nmcli", "quickshell", "arch-update"] {
            let file = bin.join(name);
            fs::write(&file, FAKE).unwrap();
            fs::set_permissions(file, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            log: base.join("calls"),
            base,
            bin,
            runtime,
        }
    }
    fn call(&self, args: &[&str], failure: Option<&str>) -> Output {
        self.command(args, failure).output().unwrap()
    }
    fn command(&self, args: &[&str], failure: Option<&str>) -> Command {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut command = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        command
            .args(["--root", root.to_str().unwrap(), "control-center"])
            .args(args)
            .env("HOME", &self.base)
            .env("XDG_CACHE_HOME", self.base.join("cache"))
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("PATH", &self.bin)
            .env("DF_CONTROL_LOG", &self.log)
            .env_remove("DF_FAIL_CONTROL");
        if let Some(value) = failure {
            command.env("DF_FAIL_CONTROL", value);
        }
        command
    }
    fn cache_write(&self, value: &Value) -> Output {
        use std::io::Write;
        let mut child = self
            .command(&["snapshot-cache", "write"], None)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(value).unwrap())
            .unwrap();
        child.wait_with_output().unwrap()
    }
    fn cache_file(&self) -> PathBuf {
        self.base
            .join("cache/desktop-foundation/control-center/snapshot-v1.json")
    }
    fn calls(&self) -> Vec<Vec<String>> {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(|line| line.split('\t').map(String::from).collect())
            .collect()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let output = self.call(args, None);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}
#[test]
fn remembered_snapshots_round_trip_privately_without_querying_or_reapplying() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new();
    assert!(f.ok(&["snapshot-cache", "read"])["snapshot"].is_null());
    assert!(f.calls().is_empty());
    let snapshot = f.ok(&["snapshot"]);
    fs::remove_file(&f.log).unwrap();
    let result = f.cache_write(&snapshot);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(f.ok(&["snapshot-cache", "read"])["snapshot"], snapshot);
    let first = fs::metadata(f.cache_file()).unwrap();
    assert_eq!(first.permissions().mode() & 0o777, 0o600);
    assert!(f.cache_write(&snapshot).status.success());
    assert_eq!(fs::metadata(f.cache_file()).unwrap().ino(), first.ino());
    assert!(
        f.calls().is_empty(),
        "Reading/writing display cache must not touch devices"
    );
    let mut audio = snapshot["audio"].clone();
    audio["outputs"][0]["volume"] = 63.into();
    assert!(
        f.cache_write(&serde_json::json!({"audio": audio}))
            .status
            .success()
    );
    let restored = f.ok(&["snapshot-cache", "read"]);
    assert_eq!(restored["snapshot"]["audio"]["outputs"][0]["volume"], 63);
    assert_eq!(restored["snapshot"]["network"], snapshot["network"]);
}
#[test]
fn corrupt_future_and_unsafe_snapshot_files_are_ignored_and_preserved() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let snapshot = f.ok(&["snapshot"]);
    assert!(f.cache_write(&snapshot).status.success());
    let path = f.cache_file();
    for bytes in [
        b"broken".to_vec(),
        b"{\"version\":99}".to_vec(),
        vec![b' '; 1024 * 1024 + 1],
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(f.ok(&["snapshot-cache", "read"])["snapshot"].is_null());
        assert!(!f.cache_write(&snapshot).status.success());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_file(&path).unwrap();
    let foreign = f.base.join("foreign");
    fs::write(&foreign, "keep").unwrap();
    symlink(&foreign, &path).unwrap();
    assert!(f.ok(&["snapshot-cache", "read"])["snapshot"].is_null());
    assert!(!f.cache_write(&snapshot).status.success());
    assert_eq!(fs::read_to_string(foreign).unwrap(), "keep");
}
#[test]
fn invalid_cached_device_values_never_replace_a_good_snapshot() {
    let f = Fixture::new();
    let snapshot = f.ok(&["snapshot"]);
    assert!(f.cache_write(&snapshot).status.success());
    let original = fs::read(f.cache_file()).unwrap();
    let mut invalid = snapshot.clone();
    invalid["audio"]["outputs"][0]["volume"] = 101.into();
    assert!(!f.cache_write(&invalid).status.success());
    invalid = snapshot.clone();
    invalid["network"]["connections"][0]["uuid"] = "--help".into();
    assert!(!f.cache_write(&invalid).status.success());
    invalid = snapshot.clone();
    invalid["audio"]["applications"][0]["serial"] = serde_json::Value::Null;
    assert!(!f.cache_write(&invalid).status.success());
    assert_eq!(fs::read(f.cache_file()).unwrap(), original);
    let mut large_network = snapshot["network"].clone();
    large_network["devices"] = serde_json::json!(vec![
        serde_json::json!({
            "name":"n".repeat(3000),"kind":"wifi","state":"connected","connection":"Example"
        });
        200
    ]);
    assert!(
        f.cache_write(&serde_json::json!({"network":large_network}))
            .status
            .success()
    );
    let retained = fs::read(f.cache_file()).unwrap();
    let mut large_audio = snapshot["audio"].clone();
    large_audio["outputs"] = serde_json::json!(vec![
        serde_json::json!({
            "name":"a".repeat(3000),"volume":50,"muted":false
        });
        200
    ]);
    assert!(
        !f.cache_write(&serde_json::json!({"audio":large_audio}))
            .status
            .success()
    );
    assert_eq!(fs::read(f.cache_file()).unwrap(), retained);
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn snapshot_handles_escaped_names_and_isolates_service_failures() {
    let f = Fixture::new();
    let value = f.ok(&["snapshot"]);
    assert_eq!(value["audio"]["outputs"][0]["volume"], 50);
    assert_eq!(value["audio"]["applications"][0]["name"], "Music");
    assert_eq!(value["audio"]["applications"][0]["serial"], "100");
    assert_eq!(value["audio"]["applications"].as_array().unwrap().len(), 2);
    assert_eq!(value["audio"]["applications"][1]["volume"], 25);
    assert_eq!(value["audio"]["applications"][1]["label"], "Music · Game");
    assert_eq!(value["audio"]["inputs"].as_array().unwrap().len(), 1);
    assert_eq!(value["network"]["connections"][0]["name"], "Office:LAN");
    assert_eq!(value["network"]["connections"][0]["active"], true);
    assert_eq!(value["network"]["devices"][0]["connection"], "Office:LAN");
    assert!(!f.calls().iter().flatten().any(|s| s.contains("password")));
    for (failed, available, unavailable) in
        [("pactl", "network", "audio"), ("nmcli", "audio", "network")]
    {
        let output = f.call(&["snapshot"], Some(failed));
        assert!(output.status.success());
        let data: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(data[available]["available"], true);
        assert_eq!(data[unavailable]["available"], false);
        assert!(data[unavailable]["error"].is_string());
    }
}
#[test]
fn audio_controls_route_streams_and_reject_invalid_values() {
    let f = Fixture::new();
    for (args, expected) in [
        (
            vec!["volume", "output", "72"],
            vec!["pactl", "set-sink-volume", "@DEFAULT_SINK@", "72%"],
        ),
        (
            vec!["volume", "input", "22"],
            vec!["pactl", "set-source-volume", "@DEFAULT_SOURCE@", "22%"],
        ),
        (
            vec!["mute", "input", "on"],
            vec!["pactl", "set-source-mute", "@DEFAULT_SOURCE@", "1"],
        ),
        (
            vec!["select", "output", "speakers"],
            vec!["pactl", "move-sink-input", "42", "speakers"],
        ),
        (
            vec!["select", "input", "mic"],
            vec!["pactl", "move-source-output", "42", "mic"],
        ),
    ] {
        f.ok(&args);
        assert!(
            f.calls().iter().any(|call| call == &expected),
            "missing {expected:?}"
        );
    }
    for expected in [
        vec!["pactl", "set-default-sink", "speakers"],
        vec!["pactl", "set-default-source", "mic"],
    ] {
        assert!(f.calls().iter().any(|call| call == &expected));
    }
    f.ok(&["stream-volume", "42", "100", "37"]);
    f.ok(&["stream-volume", "43", "101", "70"]);
    assert!(
        f.calls()
            .iter()
            .any(|v| v == &["pactl", "set-sink-input-volume", "43", "70%"])
    );
    assert!(
        f.calls()
            .iter()
            .any(|v| v == &["pactl", "set-sink-input-volume", "42", "37%"])
    );
    assert!(
        !f.call(&["stream-volume", "42", "101", "37"], None)
            .status
            .success()
    );
    let before = f.calls();
    for args in [
        vec!["volume", "output", "101"],
        vec!["volume", "output", "-1"],
        vec!["volume", "output", "nan"],
        vec!["mute", "input", "toggle"],
    ] {
        assert!(!f.call(&args, None).status.success());
    }
    assert_eq!(f.calls(), before);
    let before = f.calls().len();
    assert!(
        !f.call(&["select", "output", "--bogus"], None)
            .status
            .success()
    );
    assert!(!f.calls()[before..].iter().any(|v| {
        v.get(1)
            .is_some_and(|s| s == "set-default-sink" || s == "move-sink-input")
    }));
}
#[test]
fn network_actions_validate_identifiers_and_use_separate_arguments() {
    let f = Fixture::new();
    for args in [
        vec!["connection", "up", "12345678-abcd-1234-abcd-123456789abc"],
        vec!["wifi", "off"],
        vec!["networking", "off"],
        vec!["device", "disconnect", "enp4s0"],
    ] {
        f.ok(&args);
    }
    assert!(f.calls().iter().any(|v| v
        == &[
            "nmcli",
            "--wait",
            "10",
            "connection",
            "up",
            "uuid",
            "12345678-abcd-1234-abcd-123456789abc"
        ]));
    let before = f.calls();
    for args in [
        vec!["connection", "up", "--bogus"],
        vec!["device", "connect", "--bogus"],
        vec!["device", "connect", "eth0;reboot"],
        vec!["networking", "invalid"],
    ] {
        assert!(!f.call(&args, None).status.success());
    }
    assert_eq!(f.calls(), before);
}
#[test]
fn toggle_sends_shared_style_without_starting_monitor() {
    let f = Fixture::new();
    let output = f.call(&["toggle"], None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = f.calls();
    assert!(calls.iter().all(|call| call[0] == "quickshell"));
    let last = calls.last().unwrap();
    assert_eq!(last[last.len() - 2], "toggleControlCenter");
    let style: Value = serde_json::from_str(last.last().unwrap()).unwrap();
    assert_eq!(
        style,
        serde_json::json!({"width":720,"padding":12,"radius":14,"opacity":0.75,"openingMs":240,"closingMs":180})
    );
}

#[test]
fn update_check_only_queries_availability_and_reports_errors() {
    let f = Fixture::new();
    let output = f.call(&["check-updates"], None);
    assert!(output.status.success());
    assert!(
        fs::read_to_string(&f.log)
            .unwrap()
            .contains("arch-update\t--check")
    );
    assert!(
        !f.call(&["check-updates"], Some("arch-update"))
            .status
            .success()
    );
}

#[test]
fn sidebar_entry_is_presented_before_discovery_and_can_be_cancelled() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Quickshell unavailable; cannot exercise private QML sidebar");
        return;
    }
    let output =
        Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-control-center"))
            .env("DF_CONTROL_OPENING_TEST", "1")
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn control_center_keyboard_navigation_preserves_focus_and_reveals_controls() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Quickshell unavailable; cannot exercise private QML keyboard input");
        return;
    }
    let output =
        Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-control-center"))
            .env("DF_CONTROL_KEYBOARD_TEST", "1")
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// The real native watch protocol against inert event-producing services.
const WATCH_FAKE: &str = r#"#!/usr/bin/python3
import json, os, pathlib, sys, time
base = pathlib.Path(os.environ['DF_CONTROL_WATCH'])
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
with open(base / 'calls', 'a') as log:
    log.write(name + '\t' + '\t'.join(args) + '\n')
if args == (['subscribe'] if name == 'pactl' else ['monitor']):
    (base / (name + '.pid')).write_text(str(os.getpid()))
    with os.fdopen(os.open(base / (name + '.events'), os.O_RDWR)) as events:
        for event in events:
            if event.strip() == 'EXIT': sys.exit(0)
            print(event, end='', flush=True)
    sys.exit(0)
if name == 'pactl' and (base / 'query-failed').exists(): sys.exit(8)
state = json.loads((base / 'live.json').read_text())
if name == 'pactl':
    if args[-1] == 'info': print(json.dumps({'default_sink_name':'speakers','default_source_name':'mic'}))
    elif args[-1] == 'sinks':
        if (base / 'slow').exists():
            (base / 'entered').touch()
            time.sleep(.25)
        print(json.dumps([{'index':1,'name':'speakers','description':'Speakers','mute':False,'volume':{'mono':{'value':state['volume'] * 65536 // 100}}}]))
    elif args[-1] == 'sources': print('[]')
    elif args[-1] == 'sink-inputs': print('[]')
else:
    if args[-1] == 'general': print(('enabled' if state['network'] else 'disabled') + ':enabled:connected:full')
    elif args[-1] == 'status': print('enp4s0:ethernet:connected:Office')
    elif args[-1] == 'show': print('')
"#;

#[test]
fn live_state_updates_while_views_are_closed_without_idle_polling_and_recovers() {
    use std::{
        io::{BufRead, BufReader, Write},
        sync::mpsc,
        time::{Duration, Instant},
    };
    struct Watch(std::process::Child);
    impl Drop for Watch {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn wait_for(mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !predicate() {
            assert!(Instant::now() < deadline, "event source timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let f = Fixture::new();
    for name in ["pactl", "nmcli"] {
        fs::write(f.bin.join(name), WATCH_FAKE).unwrap();
        let path = std::ffi::CString::new(
            f.base
                .join(format!("{name}.events"))
                .as_os_str()
                .as_encoded_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    }
    let state = |volume: u64, network: bool| {
        let pending = f.base.join("live.pending");
        fs::write(
            &pending,
            serde_json::json!({"volume":volume,"network":network}).to_string(),
        )
        .unwrap();
        fs::rename(pending, f.base.join("live.json")).unwrap();
    };
    state(27, false);
    let mut child = Watch(
        f.command(&["watch"], None)
            .env("DF_CONTROL_WATCH", &f.base)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut input = child.0.stdin.take().unwrap();
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx
                .send(serde_json::from_str::<Value>(&line.unwrap()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let receive = || {
        rx.recv_timeout(Duration::from_secs(4))
            .expect("missing state event")
    };
    let first = receive();
    let second = receive();
    assert_eq!(first["audio"]["outputs"][0]["volume"], 27);
    assert_eq!(second["network"]["enabled"], false);
    for name in ["pactl", "nmcli"] {
        wait_for(|| f.base.join(format!("{name}.pid")).exists());
    }
    let event = |name: &str, text: &str| {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.base.join(format!("{name}.events")))
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    };
    std::thread::sleep(Duration::from_millis(80));
    let calls = f.calls();
    event("pactl", "Event 'new' on client #1\n");
    std::thread::sleep(Duration::from_millis(180));
    assert_eq!(
        f.calls(),
        calls,
        "idle/client events must not query devices"
    );
    state(63, true);
    event("pactl", "Event 'change' on sink #1\n");
    assert_eq!(receive()["audio"]["outputs"][0]["volume"], 63);
    event("nmcli", "enp4s0: connected\n");
    assert_eq!(receive()["network"]["enabled"], true);
    // A command arriving during a slow read invalidates the obsolete result.
    fs::write(f.base.join("slow"), "").unwrap();
    event("pactl", "Event 'change' on sink #1\n");
    wait_for(|| f.base.join("entered").exists());
    state(81, true);
    input.write_all(b"{\"audio_epoch\":1}\n").unwrap();
    fs::remove_file(f.base.join("slow")).unwrap();
    let fresh = receive();
    assert_eq!(fresh["audio_epoch"], 1);
    assert_eq!(fresh["audio"]["outputs"][0]["volume"], 81);
    // A failed read can recover even if the surviving event stream stays quiet.
    fs::write(f.base.join("query-failed"), "").unwrap();
    event("pactl", "Event 'change' on sink #1\n");
    assert_eq!(receive()["audio"]["available"], false);
    fs::remove_file(f.base.join("query-failed")).unwrap();
    assert_eq!(receive()["audio"]["available"], true);
    // Only the failed service goes unavailable, then its owned monitor reconnects.
    event("pactl", "EXIT\n");
    assert_eq!(receive()["audio"]["available"], false);
    assert_eq!(receive()["audio"]["available"], true);
    let pids: Vec<i32> = ["pactl", "nmcli"]
        .iter()
        .map(|name| {
            fs::read_to_string(f.base.join(format!("{name}.pid")))
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    drop(input); // Shell teardown, no detached subscriptions left behind.
    wait_for(|| child.0.try_wait().unwrap().is_some());
    assert!(child.0.wait().unwrap().success());
    reader.join().unwrap();
    for pid in pids {
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "monitor child leaked");
    }
}
