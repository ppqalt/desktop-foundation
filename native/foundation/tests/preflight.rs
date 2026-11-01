//! Dependency preflight against isolated tools and a generated fixture checkout.
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    bin: PathBuf,
    log: PathBuf,
}
impl Fixture {
    fn new(missing: &[&str]) -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-preflight-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("checkout");
        let bin = base.join("bin");
        let log = base.join("niri.log");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(root.join("native/foundation/target/release")).unwrap();
        symlink(
            env!("CARGO_BIN_EXE_desktop-foundationctl"),
            root.join("native/foundation/target/release/desktop-foundationctl"),
        )
        .unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["config", "theme", "profiles", "compositor"] {
            symlink(source.join(name).canonicalize().unwrap(), root.join(name)).unwrap();
        }
        let tools = [
            "pactl",
            "busctl",
            "blueman-manager",
            "niri",
            "quickshell",
            "kitty",
            "slangc",
            "fish",
            "fastfetch",
            "lua",
            "wl-copy",
            "wl-paste",
            "wl-clip-persist",
            "swaybg",
            "mako",
            "grim",
            "slurp",
            "wpctl",
            "playerctl",
            "notify-send",
            "dbus-update-activation-environment",
            "gsettings",
            "xdg-mime",
            "fc-cache",
            "pacman-conf",
            "xwayland-satellite",
        ];
        for name in tools {
            if missing.contains(&name) {
                continue;
            }
            let path = bin.join(name);
            if name == "lua" {
                symlink("/usr/bin/lua", path).unwrap();
            } else if name == "niri" {
                fs::write(
                    &path,
                    "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DF_PREFLIGHT_LOG\"\nexit 0\n",
                )
                .unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            } else {
                symlink("/bin/true", path).unwrap();
            }
        }
        Self {
            base,
            root,
            bin,
            log,
        }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .args(["--root", self.root.to_str().unwrap(), "preflight"])
            .args(args)
            .env("PATH", &self.bin)
            .env("DF_PREFLIGHT_LOG", &self.log)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn update_allows_missing_optional_tools_but_installer_rejects_them() {
    if !std::path::Path::new("/usr/bin/lua").is_file() {
        eprintln!("Skipping preflight rendering fixture: lua unavailable");
        return;
    }
    let f = Fixture::new(&["playerctl"]);
    let output = f.run(&["--update"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("playerctl"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("PASS"));
    let log = fs::read_to_string(&f.log).unwrap();
    assert_eq!(log.lines().count(), 1);
    assert!(log.starts_with("validate -c "));
    fs::remove_file(&f.log).unwrap();
    let output = f.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Missing runtime tools: playerctl"));
    assert!(!f.log.exists());
}
#[test]
fn update_rejects_missing_required_tool_before_validation() {
    let f = Fixture::new(&["fish"]);
    let output = f.run(&["--update"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Missing deployment tools: fish"));
    assert!(!f.log.exists());
}
