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
    assert_eq!(requests.len(), 2);
    assert!(requests[0].ends_with("foundation status"));
    assert!(requests[1].ends_with("foundation toggleMonitor"));
}
