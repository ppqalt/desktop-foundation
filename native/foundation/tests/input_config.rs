//! The renderer consumes partial user overrides without touching the live session.
use std::{fs, path::Path, process::Command};

#[test]
fn renderer_uses_partial_input_override_and_rejects_invalid_values() {
    let base = std::env::temp_dir().join(format!("foundation-input-render-{}", std::process::id()));
    fs::create_dir_all(base.join("config/desktop-foundation")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let input = base.join("config/desktop-foundation/input.toml");
    fs::write(&input, "layout = 'us'\n").unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&root)
            .args(["render", "niri", "default"])
            .env("HOME", &base)
            .env("XDG_CONFIG_HOME", base.join("config"))
            .env("XDG_STATE_HOME", base.join("state"))
            .env("PATH", base.join("no-executables"))
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rendered = String::from_utf8(result.stdout).unwrap();
    assert!(rendered.contains("layout \"us\""));
    assert!(rendered.contains("repeat-delay 350"));
    assert!(rendered.contains("repeat-rate 35"));
    fs::write(&input, "layout = 123\n").unwrap();
    let result = run();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Invalid input settings"));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn customized_legacy_settings_are_never_silently_discarded_or_executed() {
    let base =
        std::env::temp_dir().join(format!("foundation-input-migration-{}", std::process::id()));
    let root = base.join("checkout");
    fs::create_dir_all(root.join("config")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["input.toml", "window-appearance.toml", "monitor.toml"] {
        fs::copy(
            source.join("config").join(name),
            root.join("config").join(name),
        )
        .unwrap();
    }
    let legacy = root.join("config/input.lua");
    let contents = "error('this Lua must not execute')\n";
    fs::write(&legacy, contents).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .arg("--root")
        .arg(&root)
        .args(["render", "niri", "default"])
        .env("HOME", &base)
        .env("XDG_CONFIG_HOME", base.join("config"))
        .env("XDG_STATE_HOME", base.join("state"))
        .env("PATH", base.join("no-executables"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("needs migration"));
    assert_eq!(fs::read_to_string(&legacy).unwrap(), contents);
    assert!(result.stdout.is_empty());
    fs::remove_dir_all(base).unwrap();
}
