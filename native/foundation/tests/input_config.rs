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
