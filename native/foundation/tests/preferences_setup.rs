use desktop_foundationctl::theme_render;
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "foundation-preference-setup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        for name in ["bin", "settings"] {
            fs::create_dir(p.join(name)).unwrap();
        }
        let gs = p.join("bin/gsettings");
        fs::write(&gs,"#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DF_SETTINGS_LOG\"\ncase \"$1\" in get) /bin/cat \"$DF_SETTINGS/$3\";; set) printf '%s\\n' \"$4\" > \"$DF_SETTINGS/$3\";; *) exit 2;; esac\n").unwrap();
        fs::set_permissions(gs, fs::Permissions::from_mode(0o700)).unwrap();
        for (key, value) in [
            ("font-name", "'Original Font 11'"),
            ("color-scheme", "'default'"),
            ("gtk-theme", "'Adwaita'"),
            ("icon-theme", "'UserIcons'"),
        ] {
            fs::write(p.join("settings").join(key), format!("{value}\n")).unwrap();
        }
        Self(p)
    }
    fn invoke(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .arg("preferences-setup")
            .args(args)
            .env("HOME", &self.0)
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("PATH", self.0.join("bin"))
            .env("DF_SETTINGS", self.0.join("settings"))
            .env("DF_SETTINGS_LOG", self.0.join("calls"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap()
    }
    fn setting(&self, key: &str) -> String {
        fs::read_to_string(self.0.join("settings").join(key))
            .unwrap()
            .trim()
            .into()
    }
    fn writes(&self) -> usize {
        fs::read_to_string(self.0.join("calls"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("set "))
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn native_theme_preferences_roundtrip_and_rerun_preserve_originals() {
    let f = Fixture::new();
    for _ in 0..2 {
        let r = f.invoke(&["install", "--theme-only"]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    assert_eq!(f.setting("font-name"), "'Google Sans Flex 10'");
    assert_eq!(f.setting("icon-theme"), "'UserIcons'");
    assert_eq!(f.writes(), 3);
    let saved: Value = serde_json::from_slice(
        &fs::read(f.0.join("state/desktop-foundation/preferences.json")).unwrap(),
    )
    .unwrap();
    let entries = saved.as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries.iter().find(|e| e["key"][1] == "font-name").unwrap()["original"],
        "'Original Font 11'"
    );
    let r = f.invoke(&["restore"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(f.setting("font-name"), "'Original Font 11'");
    assert_eq!(f.setting("color-scheme"), "'default'");
    assert_eq!(f.setting("gtk-theme"), "'Adwaita'");
    assert_eq!(f.setting("icon-theme"), "'UserIcons'");
    assert!(
        !f.0.join("state/desktop-foundation/preferences.json")
            .exists()
    );
}
#[test]
fn external_preference_change_stops_restore_before_any_write() {
    let f = Fixture::new();
    assert!(f.invoke(&["install", "--theme-only"]).status.success());
    let writes = f.writes();
    fs::write(f.0.join("settings/gtk-theme"), "'External'\n").unwrap();
    let r = f.invoke(&["restore"]);
    assert!(!r.status.success());
    assert_eq!(f.writes(), writes);
    assert_eq!(f.setting("gtk-theme"), "'External'");
    assert_eq!(f.setting("font-name"), "'Google Sans Flex 10'");
    assert!(
        f.0.join("state/desktop-foundation/preferences.json")
            .is_file()
    );
}
#[test]
fn invalid_mime_journal_does_not_restore_preferences_first() {
    let f = Fixture::new();
    assert!(f.invoke(&["install", "--theme-only"]).status.success());
    let writes = f.writes();
    theme_render::write_json(
        &f.0.join("state/desktop-foundation/application-roles.json"),
        &serde_json::json!({"invalid":"journal"}),
    )
    .unwrap();
    let r = f.invoke(&["restore"]);
    assert!(!r.status.success());
    assert_eq!(f.writes(), writes);
    assert_eq!(f.setting("font-name"), "'Google Sans Flex 10'");
    assert!(
        f.0.join("state/desktop-foundation/preferences.json")
            .is_file()
    );
}
#[test]
fn invalid_browser_is_rejected_without_preference_mutation() {
    let f = Fixture::new();
    let r = f.invoke(&["install", "--browser", "unknown.desktop"]);
    assert!(!r.status.success());
    assert_eq!(f.writes(), 0);
    assert_eq!(f.setting("font-name"), "'Original Font 11'");
}
