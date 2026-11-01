use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn benchmark_collects_real_process_measurements_and_traces_fake_shell() {
    let dir =
        std::env::temp_dir().join(format!("foundation-benchmark-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let quickshell = dir.join("quickshell");
    fs::write(&quickshell, r##"#!/bin/sh
if [ "$1" = ipc ]; then
    case "$6" in
        status) if [ -f "$DF_BENCH_STATE" ]; then probe=true; else probe=false; fi; printf '{"ready":true,"probeAlive":%s}\n' "$probe";;
        showProbe) : > "$DF_BENCH_STATE";;
        hideProbe) /bin/rm -f "$DF_BENCH_STATE";;
    esac
    exit 0
fi
printf '%s' "$$" > "$DF_BENCH_PID"
trap 'exit 0' TERM INT
while :; do /bin/sleep 1; done
"##).unwrap();
    fs::set_permissions(&quickshell, fs::Permissions::from_mode(0o700)).unwrap();
    let strace = dir.join("strace");
    fs::write(&strace, "#!/bin/sh\nprintf '%s\\n' '1 execve(\"quickshell\", [], 0) = 0' > \"$5\"\n/bin/sleep 20 &\nwait\n").unwrap();
    fs::set_permissions(&strace, fs::Permissions::from_mode(0o700)).unwrap();
    let output_file = dir.join("nested/report.json");
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args([
            "--root",
            dir.to_str().unwrap(),
            "bench",
            "--runs=1",
            "--seconds",
            "0.02",
            "--output",
        ])
        .arg(&output_file)
        .arg("--trace")
        .env("PATH", &dir)
        .env("DF_BENCH_STATE", dir.join("probe"))
        .env("DF_BENCH_PID", dir.join("pid"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&output_file).unwrap()).unwrap();
    assert_eq!(json["runs"].as_array().unwrap().len(), 1);
    assert_eq!(json["runs"][0]["idle"].as_array().unwrap().len(), 2);
    assert!(json["runs"][0]["idle"][0]["Rss_KiB"].as_u64().unwrap() > 0);
    assert!(json["runs"][0]["idle"][0]["all_surviving_threads_context_switches_per_s"].is_number());
    assert!(json["ready_median_ms"].is_number());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("3.0s trace: 1 exec requests")
    );
    let pid: u32 = fs::read_to_string(dir.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "owned shell remained alive"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn launcher_benchmark_uses_shared_measurements_and_closes_fixture_launcher() {
    let dir = std::env::temp_dir().join(format!(
        "foundation-launcher-benchmark-test-{}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    let systemctl = dir.join("systemctl");
    fs::write(&systemctl, "#!/bin/sh\nprintf '%s\\n' \"$DF_TEST_PID\"\n").unwrap();
    fs::set_permissions(&systemctl, fs::Permissions::from_mode(0o700)).unwrap();
    let quickshell = dir.join("quickshell");
    fs::write(&quickshell, r##"#!/bin/sh
case "$6" in
    status) if [ -f "$DF_TEST_LAUNCHER" ]; then state=true; else state=false; fi; printf '{"launcherAlive":%s}\n' "$state";;
    showLauncher) : > "$DF_TEST_LAUNCHER";;
    hideLauncher) /bin/rm -f "$DF_TEST_LAUNCHER";;
esac
"##).unwrap();
    fs::set_permissions(&quickshell, fs::Permissions::from_mode(0o700)).unwrap();
    let launcher_state = dir.join("launcher");
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .arg("launcher-bench")
        .env("PATH", &dir)
        .env("DF_TEST_PID", std::process::id().to_string())
        .env("DF_TEST_LAUNCHER", &launcher_state)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["samples"].as_array().unwrap().len(), 3);
    assert!(report["creation_roundtrip_ms"].is_number());
    assert!(report["exit_and_destruction_roundtrip_ms"].is_number());
    assert!(!launcher_state.exists());
    fs::remove_dir_all(dir).unwrap();
}
