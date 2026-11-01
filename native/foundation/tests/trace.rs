use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn synthetic_tracer_reports_execs_and_exits_without_live_desktop() {
    let dir = std::env::temp_dir().join(format!("foundation-trace-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let fake = dir.join("strace");
    fs::write(&fake, "#!/bin/sh\nprintf '%s\\n' '1 execve(\"quickshell\", [], 0) = 0' '1 execveat(0, \"tool\", [], 0, 0) = 0' > \"$5\"\n/bin/sleep 20 &\nwait\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args([
            "--root",
            dir.to_str().unwrap(),
            "trace",
            "--seconds",
            "0.03",
        ])
        .env("PATH", &dir)
        .output()
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("0.03s trace: 2 exec requests"), "{output}");
    assert!(output.contains("execveat("), "{output}");
}

#[test]
fn missing_tracer_reports_unavailable() {
    let dir = std::env::temp_dir().join(format!("foundation-trace-missing-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["trace", "--seconds=0.01"])
        .env("PATH", &dir)
        .output()
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Unavailable:")
    );
}
