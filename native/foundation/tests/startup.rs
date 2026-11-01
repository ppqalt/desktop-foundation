use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn measures_readiness_and_reaps_the_owned_shell() {
    let dir = std::env::temp_dir().join(format!("foundation-startup-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let exe = dir.join("quickshell");
    fs::write(&exe, "#!/bin/sh\nif [ \"$1\" = ipc ]; then printf '%s\\n' '{\"ready\":true}'; exit 0; fi\nprintf '%s' \"$$\" > \"$DF_TEST_PID\"\ntrap 'exit 0' TERM\nwhile :; do /bin/sleep 1; done\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let pid_file = dir.join("pid");
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["--root", dir.to_str().unwrap(), "startup"])
        .env("PATH", &dir)
        .env("DF_TEST_PID", &pid_file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["backend_ready_ms"].as_f64().unwrap() >= 0.0);
    let pid: u32 = fs::read_to_string(pid_file).unwrap().parse().unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reports_early_shell_exit() {
    let dir =
        std::env::temp_dir().join(format!("foundation-startup-failure-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let exe = dir.join("quickshell");
    fs::write(&exe, "#!/bin/sh\nexit 42\n").unwrap();
    fs::set_permissions(exe, fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["--root", dir.to_str().unwrap(), "startup"])
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Quickshell exited"));
    fs::remove_dir_all(dir).unwrap();
}
