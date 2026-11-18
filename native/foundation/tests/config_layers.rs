use desktop_foundationctl::config_layers;
use std::{fs, path::Path, process::Command};

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "df-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn partial_layers_are_ordered_and_fail_closed_without_touching_files() {
    let dir = Directory::new();
    let root = dir.path();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("profiles/test")).unwrap();
    fs::create_dir_all(root.join("user/desktop-foundation/sample.d")).unwrap();
    fs::write(
        root.join("config/sample.toml"),
        "[section]\na=1\nb=2\nenabled=true\n",
    )
    .unwrap();
    fs::write(root.join("profiles/test/sample.toml"), "[section]\na=3\n").unwrap();
    fs::write(
        root.join("user/desktop-foundation/sample.d/10.toml"),
        "[section]\na=4\n",
    )
    .unwrap();
    fs::write(
        root.join("user/desktop-foundation/sample.d/20.toml"),
        "[section]\na=5\n",
    )
    .unwrap();
    let user = root.join("user/desktop-foundation/sample.toml");
    fs::write(&user, "[section]\nb=8\n").unwrap();
    let run = || config_layers::load_at(root, "sample", Some("test"), &root.join("user"));
    let value = run().unwrap();
    assert_eq!(value.settings["section"]["a"], 5);
    assert_eq!(value.settings["section"]["b"], 8);
    assert_eq!(value.settings["section"]["enabled"], true);
    assert_eq!(value.sources.len(), 5);
    for bad in [
        "[section]\nunknown=1\n",
        "[section]\nenabled=1\n",
        "section=1\n",
        "section={",
        "#",
    ] {
        let contents = if bad == "#" {
            "#".repeat(65537)
        } else {
            bad.into()
        };
        fs::write(&user, &contents).unwrap();
        assert!(run().is_err());
        assert_eq!(fs::read_to_string(&user).unwrap(), contents);
    }
    assert!(
        config_layers::load_at(root, "sample", Some("../../outside"), &root.join("user")).is_err()
    );
}

#[test]
fn appearance_and_desktop_commands_validate_preferences_and_stock_recovery() {
    let dir = Directory::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = dir.path().join("config/desktop-foundation");
    fs::create_dir_all(&config).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&root)
            .args(args)
            .env("HOME", dir.path())
            .env("XDG_CONFIG_HOME", dir.path().join("config"))
            .env("XDG_STATE_HOME", dir.path().join("state"))
            .output()
            .unwrap()
    };
    let render = || {
        let o = run(&["render", "niri", "default"]);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    assert!(!render().contains("workspace-feather"));
    fs::write(
        config.join("niri-package-input.toml"),
        "overview_ctrl_scroll=true\n",
    )
    .unwrap();
    assert!(render().contains("workspace-feather 4;"));
    fs::write(
        config.join("window-appearance.toml"),
        "[overview]\nfeather_enabled=false\n",
    )
    .unwrap();
    assert!(render().contains("workspace-feather 0;"));
    // The custom appearance capability survives disabling the zoom shortcut.
    fs::write(config.join("input.toml"), "overview_ctrl_scroll=false\n").unwrap();
    let appearance = config.join("window-appearance.toml");
    let contents = "rounding=18\n[overview]\nfeather_enabled=true\nfeather_width=9\ncorner_radius=20\n[shadow]\nspread=6\ninactiveOpacity=0.5\n";
    fs::write(&appearance, contents).unwrap();
    let rendered = render();
    assert!(rendered.contains("workspace-corner-radius 20; workspace-feather 9;"));
    assert!(rendered.contains("geometry-corner-radius 18"));
    assert!(rendered.contains("spread 6;"));
    assert!(rendered.contains("inactive-color \"#00000080\""));
    assert!(!rendered.contains("ctrl-scroll-workspaces true"));
    fs::write(
        config.join("niri-package-input.toml"),
        "overview_ctrl_scroll=false\n",
    )
    .unwrap();
    assert!(!render().contains("workspace-feather"));
    assert_eq!(fs::read_to_string(&appearance).unwrap(), contents);
    for bad in [
        "[overview]\nfeather_width=129\n",
        "[overview]\nfeather_enabled=1\n",
        "[shadow]\nspread=-1\n",
    ] {
        fs::write(&appearance, bad).unwrap();
        let output = run(&["render", "niri", "default"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let desktop = config.join("desktop.toml");
    fs::write(
        &desktop,
        "[media]\nhome_player='vlc'\n[screen_corners]\nradius=12\n",
    )
    .unwrap();
    let output = run(&["config", "desktop"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["settings"]["media"]["home_player"], "vlc");
    assert_eq!(value["settings"]["screen_corners"]["enabled"], true);
    assert_eq!(
        value["settings"]["screen_corners"]["cover_fullscreen"],
        true
    );
    for bad in [
        "[screen_corners]\nradius=-1",
        "[screen_corners]\nradius=129",
        "[media]\nhome_player='bad player'",
        "[media]\nunknown=true",
    ] {
        fs::write(&desktop, bad).unwrap();
        assert!(!run(&["config", "desktop"]).status.success());
    }
}
