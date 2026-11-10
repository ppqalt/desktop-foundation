use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn public_monitor_toggle_uses_the_shared_shell_router_without_starting_a_service() {
    let directory =
        std::env::temp_dir().join(format!("foundation-monitor-routing-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let calls = directory.join("calls");
    let executable = directory.join("quickshell");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DF_ROUTING_CALLS\"\nprintf '%s\\n' ready\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["--root", directory.to_str().unwrap(), "monitor", "toggle"])
        .env("PATH", &directory)
        .env("DF_ROUTING_CALLS", &calls)
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .unwrap();
    let requests = fs::read_to_string(&calls).unwrap();
    fs::remove_dir_all(directory).unwrap();
    assert!(output.status.success(), "{output:?}");
    let requests: Vec<_> = requests.lines().collect();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].ends_with("foundation toggleMonitor"));
}

#[test]
fn immediate_sidebar_dispatch_does_not_retry_an_unknown_toggle_outcome() {
    let directory = std::env::temp_dir().join(format!(
        "foundation-sidebar-dispatch-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let executable = directory.join("quickshell");
    let calls = directory.join("calls");
    fs::write(
        &executable,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$DF_ROUTING_CALLS"
exit 1
"#,
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    // Configuration is read before the direct Control Center dispatch.
    fs::create_dir(directory.join("config")).unwrap();
    fs::write(
        directory.join("config/monitor.toml"),
        include_str!("../../../config/monitor.toml"),
    )
    .unwrap();
    for args in [["monitor", "toggle"], ["control-center", "toggle"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&directory)
            .args(args)
            .env("PATH", &directory)
            .env("DF_ROUTING_CALLS", &calls)
            .env("XDG_CONFIG_HOME", directory.join("user-config"))
            .env_remove("XDG_RUNTIME_DIR")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("quickshell"),
            "{output:?}"
        );
    }
    let requests = fs::read_to_string(&calls).unwrap();
    assert_eq!(requests.lines().count(), 2);
    assert!(!requests.contains("foundation status"));
    assert!(requests.contains("foundation toggleMonitor"));
    assert!(requests.contains("foundation toggleControlCenter"));
    fs::remove_dir_all(directory).unwrap();
}
