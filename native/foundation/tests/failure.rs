use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
};

struct Sleeper(Child);
impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn validates_recovery_using_only_an_owned_child_and_inert_commands() {
    let dir = std::env::temp_dir().join(format!("foundation-failure-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let bin = dir.join("bin");
    fs::create_dir(&bin).unwrap();
    let scripts = dir.join("scripts");
    fs::create_dir(&scripts).unwrap();
    let mut child = Sleeper(
        Command::new("/bin/sleep")
            .arg("30")
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    executable(
        &bin.join("systemctl"),
        "#!/bin/sh\nif [ -e \"$DF_FAKE_CALLED\" ]; then printf '%s\\n' \"$DF_TEST_RESTART\"; else : > \"$DF_FAKE_CALLED\"; printf '%s\\n' \"$DF_TEST_CHILD\"; fi\n",
    );
    executable(
        &bin.join("niri"),
        "#!/bin/sh\nprintf '%s\\n' '[{\"id\":7}]'\n",
    );
    executable(
        &bin.join("quickshell"),
        "#!/bin/sh\nif [ \"$1\" = ipc ]; then printf '%s\\n' '{\"ready\":true}'; else printf '%s\\n' ERROR >&2; exit 1; fi\n",
    );
    executable(
        &scripts.join("run-shell"),
        "#!/bin/sh\nif [ \"$DF_COMPOSITOR\" = unsupported ]; then echo Unsupported >&2; else echo unavailable >&2; fi\nexit 1\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["--root", dir.to_str().unwrap(), "failure-check"])
        .env("PATH", format!("{}:/usr/bin", bin.display()))
        .env("NIRI_SOCKET", dir.join("fixture.sock"))
        .env("DF_FAKE_CALLED", dir.join("called"))
        .env("DF_TEST_CHILD", child.0.id().to_string())
        .env("DF_TEST_RESTART", std::process::id().to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("PASS: supervised crash/restart")
    );
    assert!(!child.0.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}
