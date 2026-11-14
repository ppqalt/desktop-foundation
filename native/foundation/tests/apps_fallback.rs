//! Isolated XDG application routing; never starts a real application.
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
#[test]
fn missing_bundled_apps_use_existing_xdg_defaults_without_changing_them() {
    let base = std::env::temp_dir().join(format!("df-app-defaults-{}", std::process::id()));
    let root = base.join("root");
    let bin = base.join("bin");
    let data = base.join("data");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(data.join("applications")).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::copy(
        source.join("config/application-roles.json"),
        root.join("config/application-roles.json"),
    )
    .unwrap();
    for (name, text) in [
        ("xdg-mime", "#!/bin/sh\nprintf 'existing.desktop\\n'\n"),
        ("gio", "#!/bin/sh\nexit 99\n"),
    ] {
        let path = bin.join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let desktop = data.join("applications/existing.desktop");
    let content = "[Desktop Entry]\nType=Application\nName=Existing app\nExec=existing-app %U\n";
    fs::write(&desktop, content).unwrap();
    for role in ["browser", "files"] {
        let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .args([
                "--root",
                root.to_str().unwrap(),
                "apps",
                "launch",
                role,
                "--check",
            ])
            .env("PATH", &bin)
            .env("HOME", base.join("home"))
            .env("XDG_STATE_HOME", base.join("state"))
            .env("XDG_DATA_HOME", &data)
            .env("XDG_DATA_DIRS", &data)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let argv: Vec<String> = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(argv[1..4], ["gio", "launch", desktop.to_str().unwrap()]);
        if role == "files" {
            assert_eq!(argv[4], base.join("home").to_str().unwrap());
        }
    }
    assert_eq!(fs::read_to_string(desktop).unwrap(), content);
    assert!(!base.join("state").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn stable_browser_mime_migration_preserves_foreign_defaults_and_rolls_back() {
    let base = std::env::temp_dir().join(format!("df-browser-mime-{}", std::process::id()));
    let root = base.join("root");
    let bin = base.join("bin");
    let data = base.join("data");
    let config = base.join("config");
    for path in [
        root.join("config"),
        bin.clone(),
        data.join("applications"),
        config.clone(),
    ] {
        fs::create_dir_all(path).unwrap();
    }
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::copy(
        source.join("config/application-roles.json"),
        root.join("config/application-roles.json"),
    )
    .unwrap();
    for (name, body) in [
        ("brave-origin", "#!/bin/sh\nexit 99\n"),
        ("xdg-mime", "#!/bin/sh\nprintf 'brave-origin.desktop\\n'\n"),
    ] {
        let path = bin.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(
        data.join("applications/brave-origin.desktop"),
        "[Desktop Entry]\nType=Application\nName=Stable\nExec=brave-origin %U\n",
    )
    .unwrap();
    let original = "# keep comment\n[Default Applications]\nx-scheme-handler/https=brave-origin-nightly.desktop;\ntext/html=firefox.desktop;\ninode/directory=dolphin.desktop;\n";
    fs::write(config.join("mimeapps.list"), original).unwrap();
    let invoke = |action| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .args(["--root", root.to_str().unwrap(), "app-roles", action])
            .env("PATH", &bin)
            .env("HOME", base.join("home"))
            .env("XDG_STATE_HOME", base.join("state"))
            .env("XDG_CONFIG_HOME", &config)
            .env("XDG_DATA_HOME", &data)
            .env("XDG_DATA_DIRS", &data)
            .output()
            .unwrap()
    };
    for _ in 0..2 {
        let result = invoke("browser-default");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let text = fs::read_to_string(config.join("mimeapps.list")).unwrap();
        assert!(text.contains("x-scheme-handler/https=brave-origin.desktop;"));
        assert!(text.contains("text/html=firefox.desktop;"));
        assert!(text.contains("inode/directory=dolphin.desktop;"));
    }
    let result = invoke("restore");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(config.join("mimeapps.list")).unwrap(),
        original
    );
    fs::remove_dir_all(base).unwrap();
}
