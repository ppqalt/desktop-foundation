//! Inert audio and network fixtures; never calls the desktop services.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut command = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        command
            .args(["--root", root.to_str().unwrap(), "control-center"])
            .args(args)
            .env("HOME", &self.base)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("PATH", &self.bin)
            .env("DF_CONTROL_LOG", &self.log)
            .env_remove("DF_FAIL_CONTROL");
        if let Some(value) = failure {
            command.env("DF_FAIL_CONTROL", value);
        }
        command.output().unwrap()
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
