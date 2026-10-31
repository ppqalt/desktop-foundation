//! Private installation fixtures; never opens Spotify or reads account prefs.
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
#[test]
fn native_check_validates_pins_paths_and_marketplace_without_reading_prefs() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let home =
        std::env::temp_dir().join(format!("foundation-spotify-check-{}", std::process::id()));
    fs::create_dir(&home).unwrap();
    let root = home.join("repo");
    let config = home.join("config");
    let apps = home.join("data/desktop-foundation/apps");
    for dir in [
        root.join("apps/spotify"),
        config.join("spotify"),
        config.join("spicetify/CustomApps/marketplace"),
        apps.join("spotify"),
        apps.join("spicetify"),
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    let pins = json!({"spotify":{"version":"fixture"},"spicetify":{"version":"fixture-version"}});
    fs::write(
        root.join("apps/spotify/sources.json"),
        serde_json::to_vec(&pins).unwrap(),
    )
    .unwrap();
    for name in ["spotify", "spicetify"] {
        fs::write(
            apps.join(name).join(".foundation-source.json"),
            serde_json::to_vec(&pins[name]).unwrap(),
        )
        .unwrap();
    }
    let binary = apps.join("spicetify/spicetify");
    fs::write(&binary, "#!/bin/sh\nprintf '%s\\n' fixture-version\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let ini = format!(
        "[Setting]\nspotify_path = {}\nprefs_path = {}\ncurrent_theme = marketplace\ncolor_scheme = DesktopFoundation\n",
        apps.join("spotify/usr/share/spotify").display(),
        config.join("spotify/prefs").display()
    );
    fs::write(config.join("spicetify/config-xpui.ini"), &ini).unwrap();
    for name in ["manifest.json", "index.js"] {
        fs::write(
            config.join("spicetify/CustomApps/marketplace").join(name),
            "",
        )
        .unwrap();
    }
    // Unreadable prefs prove check only inspects existence, not account contents.
    let prefs = config.join("spotify/prefs");
    fs::write(&prefs, "private fixture").unwrap();
    fs::set_permissions(&prefs, fs::Permissions::from_mode(0o0)).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&root)
            .arg("spotify-check")
            .env_clear()
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &config)
            .env("XDG_DATA_HOME", home.join("data"))
            .env("PATH", "/no-python")
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_file(&prefs).unwrap();
    let result = run();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("WARN first login"));
    fs::write(
        config.join("spicetify/config-xpui.ini"),
        ini.replace("DesktopFoundation", "Other"),
    )
    .unwrap();
    let result = run();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("scheme not selected"));
    fs::remove_dir_all(home).unwrap();
}
