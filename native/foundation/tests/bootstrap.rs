//! Bootstrap validation uses an isolated checkout and fake package tools.
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

#[test]
fn invalid_firmware_manifest_fails_before_any_package_operation() {
    let temporary =
        std::env::temp_dir().join(format!("foundation-bootstrap-{}", std::process::id()));
    fs::create_dir(&temporary).unwrap();
    let root = temporary.as_path();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::create_dir(root.join("packages")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/bootstrap"),
        root.join("scripts/bootstrap"),
    )
    .unwrap();
    fs::write(
        root.join("packages/core.toml"),
        "packages = [\n    \"fixture\",\n]\n",
    )
    .unwrap();
    fs::write(root.join("packages/firmware.toml"), "not valid TOML\n").unwrap();
    let pacman = root.join("bin/pacman");
    fs::write(&pacman, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&pacman, fs::Permissions::from_mode(0o755)).unwrap();
    let sudo = root.join("bin/sudo");
    fs::write(&sudo, "#!/bin/sh\nprintf called > \"$TEST_ROOT/called\"\n").unwrap();
    fs::set_permissions(&sudo, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new("/bin/bash")
        .arg(root.join("scripts/bootstrap"))
        .arg("--firmware")
        .env("TEST_ROOT", root)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid package manifest"));
    assert!(!root.join("called").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn minimal_bootstrap_does_not_read_optional_firmware() {
    let temporary = std::env::temp_dir().join(format!(
        "foundation-bootstrap-minimal-{}",
        std::process::id()
    ));
    fs::create_dir_all(temporary.join("scripts")).unwrap();
    fs::create_dir_all(temporary.join("packages")).unwrap();
    fs::create_dir_all(temporary.join("bin")).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/bootstrap"),
        temporary.join("scripts/bootstrap"),
    )
    .unwrap();
    fs::write(
        temporary.join("packages/core.toml"),
        "packages = [\n    \"fixture\",\n]\n",
    )
    .unwrap();
    fs::write(
        temporary.join("packages/firmware.toml"),
        "invalid firmware\n",
    )
    .unwrap();
    for tool in ["pacman", "sudo", "rustup"] {
        let path = temporary.join("bin").join(tool);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = Command::new("/bin/bash")
        .arg(temporary.join("scripts/bootstrap"))
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", temporary.join("bin").display()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(temporary).unwrap();
}
