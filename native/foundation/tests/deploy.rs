//! Isolated ownership-journal tests. No real service, preference or config writes.
use desktop_foundationctl::{
    deploy::{self, Entry, Manifest, Paths},
    deploy_units, theme_render,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    paths: Paths,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-deploy-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let paths = Paths {
            home: base.join("home"),
            config: base.join("home/config"),
            data: base.join("home/data"),
            state: base.join("home/state/desktop-foundation"),
        };
        fs::create_dir_all(&paths.state).unwrap();
        let root = base.join("source");
        fs::create_dir(&root).unwrap();
        Self { base, paths, root }
    }
    fn manifest(&self) -> Manifest {
        serde_json::from_value(json!({"root":self.root,"entries":[],"profile":"default","compositor":"niri","customMetadata":{"preserved":true}})).unwrap()
    }
    fn original(&self) -> PathBuf {
        let path = self.paths.config.join("kitty");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("original"), b"keep this").unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
#[test]
fn native_links_rerun_and_restore_keep_original_inode_and_unknown_metadata() {
    let f = Fixture::new();
    let path = f.original();
    let original = deploy::fingerprint(&path).unwrap();
    let mut saved = f.manifest();
    deploy::install_link(&f.paths, &mut saved, &path, &f.root.join("terminal")).unwrap();
    let backup = saved.entries[0].backup.clone().unwrap();
    deploy::install_link(&f.paths, &mut saved, &path, &f.root.join("terminal")).unwrap();
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(deploy::fingerprint(&backup).unwrap(), original);
    let mut loaded = deploy::read(&f.paths).unwrap().unwrap();
    assert_eq!(loaded.extra["customMetadata"], json!({"preserved":true}));
    deploy::restore_files(&f.paths, &mut loaded).unwrap();
    assert_eq!(deploy::fingerprint(&path).unwrap(), original);
    assert_eq!(fs::read(path.join("original")).unwrap(), b"keep this");
    assert!(!f.paths.state.join("manifest.json").exists());
}
#[test]
fn write_ahead_recovery_handles_each_install_and_restore_boundary() {
    for phase in 0..4 {
        let f = Fixture::new();
        let path = f.original();
        let fingerprint = deploy::fingerprint(&path).unwrap();
        let backup = f.paths.state.join("backup-0");
        let target = f.root.join("target");
        let mut manifest = f.manifest();
        manifest.entries.push(Entry {
            path: path.clone(),
            source: target.clone(),
            backup: Some(backup.clone()),
            original: fingerprint,
            pending_source: None,
            extra: BTreeMap::new(),
        });
        deploy::save(&f.paths, &manifest).unwrap();
        if phase >= 1 {
            fs::rename(&path, &backup).unwrap();
        }
        if phase >= 2 {
            symlink(&target, &path).unwrap();
        }
        if phase == 3 {
            fs::remove_file(&path).unwrap();
            fs::rename(&backup, &path).unwrap();
        }
        deploy::restore_files(&f.paths, &mut manifest).unwrap();
        assert_eq!(deploy::fingerprint(&path).unwrap(), fingerprint);
        assert!(path.join("original").is_file());
    }
}
#[test]
fn foreign_replacement_stops_entire_restore_before_touching_other_targets() {
    let f = Fixture::new();
    let path = f.original();
    let other = f.paths.config.join("other");
    let mut manifest = f.manifest();
    deploy::install_link(&f.paths, &mut manifest, &path, &f.root.join("first")).unwrap();
    deploy::install_link(&f.paths, &mut manifest, &other, &f.root.join("second")).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"outside change").unwrap();
    assert!(deploy::restore_files(&f.paths, &mut manifest).is_err());
    assert!(other.is_symlink());
    assert_eq!(fs::read(&path).unwrap(), b"outside change");
    assert_eq!(manifest.entries.len(), 2);
}
#[test]
fn missing_backup_is_not_silently_discarded() {
    let f = Fixture::new();
    let path = f.original();
    let mut manifest = f.manifest();
    deploy::install_link(&f.paths, &mut manifest, &path, &f.root.join("first")).unwrap();
    fs::remove_dir_all(manifest.entries[0].backup.as_ref().unwrap()).unwrap();
    assert!(deploy::restore_files(&f.paths, &mut manifest).is_err());
    assert!(path.is_symlink());
    assert!(f.paths.state.join("manifest.json").exists());
}
#[test]
fn interrupted_source_swap_preserves_backup_and_finishes_pending_journal() {
    let f = Fixture::new();
    let path = f.original();
    let mut manifest = f.manifest();
    let first = f.root.join("first");
    let second = f.root.join("second");
    deploy::install_link(&f.paths, &mut manifest, &path, &first).unwrap();
    let original = manifest.entries[0].original;
    manifest.entries[0].pending_source = Some(second.clone());
    deploy::save(&f.paths, &manifest).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(&second, &path).unwrap();
    deploy::install_link(&f.paths, &mut manifest, &path, &second).unwrap();
    assert_eq!(manifest.entries[0].source, second);
    assert!(manifest.entries[0].pending_source.is_none());
    assert_eq!(manifest.entries[0].original, original);
    deploy::restore_files(&f.paths, &mut manifest).unwrap();
    assert_eq!(deploy::fingerprint(&path).unwrap(), original);
}
#[test]
fn manifest_rejects_duplicate_destinations_and_escaping_backups() {
    let f = Fixture::new();
    let p = f.paths.config.join("test");
    let e = json!({"path":p,"source":"/source","backup":null,"original":null});
    theme_render::write_json(
        &f.paths.state.join("manifest.json"),
        &json!({"root":f.root,"entries":[e.clone(),e]}),
    )
    .unwrap();
    assert!(deploy::read(&f.paths).is_err());
    theme_render::write_json(&f.paths.state.join("manifest.json"),&json!({"root":f.root,"entries":[{"path":p,"source":"/source","backup":"/outside/backup-0","original":null}]})).unwrap();
    assert!(deploy::read(&f.paths).is_err());
}
#[test]
fn existing_oo7_links_and_gnome_masks_are_not_adopted() {
    let f = Fixture::new();
    theme_render::copy_tree(&source().join("config"), &f.root.join("config")).unwrap();
    let user = f.paths.config.join("systemd/user");
    fs::create_dir_all(user.join("default.target.wants")).unwrap();
    symlink("/dev/null", user.join("gnome-keyring-daemon.service")).unwrap();
    symlink(
        "/usr/lib/systemd/user/oo7-daemon.service",
        user.join("default.target.wants/oo7-daemon.service"),
    )
    .unwrap();
    let targets = deploy_units::targets(
        &f.root,
        &f.paths.config,
        &f.paths.state,
        &f.paths.data,
        &Default::default(),
        false,
        "default",
    )
    .unwrap();
    assert!(
        !targets
            .iter()
            .any(|(p, _)| p == &user.join("gnome-keyring-daemon.service"))
    );
    assert!(
        !targets
            .iter()
            .any(|(p, _)| p == &user.join("default.target.wants/oo7-daemon.service"))
    );
    assert!(!f.paths.state.join("session-units").exists());
}
#[test]
fn alternate_compositor_rejected_before_state_writes_and_dry_run_is_inert() {
    let f = Fixture::new();
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(source())
            .args(args)
            .env("HOME", &f.paths.home)
            .env("XDG_STATE_HOME", f.paths.state.parent().unwrap())
            .env("XDG_CONFIG_HOME", &f.paths.config)
            .env("XDG_DATA_HOME", &f.paths.data)
            .output()
            .unwrap()
    };
    let r = invoke(&["deploy", "--compositor", "hyprland"]);
    assert!(!r.status.success());
    assert!(!f.paths.state.join("manifest.json").exists());
    let r = invoke(&["deploy", "--dry-run"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(String::from_utf8_lossy(&r.stdout).contains("niri/config.kdl"));
    assert!(!f.paths.config.exists());
    assert!(!f.paths.state.join("session-units").exists());
}
#[test]
fn missing_backends_fail_without_creating_state() {
    let f = Fixture::new();
    assert!(deploy::require_backends(&f.root).is_err());
    assert!(!f.paths.state.join("manifest.json").exists());
}
#[test]
fn complete_native_deploy_rerun_update_and_restore_are_isolated() {
    // All external commands and deployment paths are isolated fixtures.
    let f = Fixture::new();
    for name in ["config", "theme", "terminal", "session", "compositor"] {
        theme_render::copy_tree(&source().join(name), &f.root.join(name)).unwrap();
    }
    for name in ["fonts", "shell", "scripts", "wallpapers"] {
        fs::create_dir_all(f.root.join(name)).unwrap();
    }
    image::RgbImage::from_pixel(24, 16, image::Rgb([40, 80, 160]))
        .save(f.root.join("wallpapers/wallhaven-135w7w.png"))
        .unwrap();
    fs::write(
        f.root.join("compositor/niri/wallpaper.toml"),
        "image = '~/missing/wallhaven-135w7w.png'\nmode = 'fill'\n",
    )
    .unwrap();
    let binary = f
        .root
        .join("native/foundation/target/release/desktop-foundationctl");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    symlink(env!("CARGO_BIN_EXE_desktop-foundationctl"), binary).unwrap();
    let nothing = f
        .root
        .join("native/nothing/target/release/foundation-nothing");
    fs::create_dir_all(nothing.parent().unwrap()).unwrap();
    fs::write(&nothing, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(nothing, fs::Permissions::from_mode(0o700)).unwrap();
    let bin = f.base.join("bin");
    fs::create_dir(&bin).unwrap();
    for command in ["systemctl", "fc-cache", "mako", "fish", "niri"] {
        let p = bin.join(command);
        fs::write(&p,"#!/bin/sh\nprintf '%s %s\\n' \"$0\" \"$*\" >> \"$DF_COMMAND_LOG\"\ncase \"$*\" in *is-active*) exit 3;; esac\n").unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let monitor = f.root.join("scripts/monitor");
    fs::write(&monitor, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(monitor, fs::Permissions::from_mode(0o700)).unwrap();
    let settings = f.base.join("settings");
    fs::create_dir(&settings).unwrap();
    let gs = bin.join("gsettings");
    fs::write(&gs,"#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DF_SETTINGS_LOG\"\ncase \"$1\" in get) if [ -f \"$DF_SETTINGS/$3\" ]; then /bin/cat \"$DF_SETTINGS/$3\"; else printf \"'Original'\\n\"; fi;; set) printf '%s\\n' \"$4\" > \"$DF_SETTINGS/$3\";; esac\n").unwrap();
    fs::set_permissions(gs, fs::Permissions::from_mode(0o700)).unwrap();
    let original = f.original();
    let fingerprint = deploy::fingerprint(&original).unwrap();
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&f.root)
            .args(args)
            .env("HOME", &f.paths.home)
            .env("XDG_STATE_HOME", f.paths.state.parent().unwrap())
            .env("XDG_CONFIG_HOME", &f.paths.config)
            .env("XDG_DATA_HOME", &f.paths.data)
            .env("XDG_CACHE_HOME", f.base.join("cache"))
            .env("PATH", &bin)
            .env("DF_SETTINGS", &settings)
            .env("DF_SETTINGS_LOG", f.base.join("settings.log"))
            .env("DF_COMMAND_LOG", f.base.join("commands.log"))
            .env_remove("NIRI_SOCKET")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap()
    };
    for _ in 0..2 {
        let r = invoke(&["deploy", "install"]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    let saved = deploy::read(&f.paths).unwrap().unwrap();
    let entry = saved.entries.iter().find(|e| e.path == original).unwrap();
    assert_eq!(entry.original, fingerprint);
    fs::write(
        f.paths.state.join("terminal/fish/fish_variables"),
        "retained writable state",
    )
    .unwrap();
    let settings_before = fs::read(f.base.join("settings.log")).unwrap();
    let r = invoke(&["deploy", "--update"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(
        fs::read(f.base.join("settings.log")).unwrap(),
        settings_before
    );
    assert_eq!(
        fs::read_to_string(f.paths.state.join("terminal/fish/fish_variables")).unwrap(),
        "retained writable state"
    );
    let r = invoke(&["deploy", "restore"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(deploy::fingerprint(&original).unwrap(), fingerprint);
    assert_eq!(fs::read(original.join("original")).unwrap(), b"keep this");
    assert!(!f.paths.state.join("manifest.json").exists());
}

#[test]
fn legacy_original_fingerprint_is_recovered_only_when_the_backup_exists() {
    for missing in [false, true] {
        let f = Fixture::new();
        let path = f.original();
        let original = deploy::fingerprint(&path).unwrap();
        let mut saved = f.manifest();
        deploy::install_link(&f.paths, &mut saved, &path, &f.root.join("source")).unwrap();
        saved.entries[0].original = None;
        if missing {
            fs::remove_dir_all(saved.entries[0].backup.as_ref().unwrap()).unwrap();
        }
        deploy::save(&f.paths, &saved).unwrap();
        if missing {
            assert!(deploy::restore_files(&f.paths, &mut saved).is_err());
            assert!(path.is_symlink());
        } else {
            deploy::restore_files(&f.paths, &mut saved).unwrap();
            assert_eq!(deploy::fingerprint(&path).unwrap(), original);
        }
    }
}
#[test]
fn unsupported_screenshot_backend_fails_without_loading_configuration() {
    let result = desktop_foundationctl::screenshot::capture(
        std::path::Path::new("/missing-checkout"),
        "hyprland",
        "region",
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Niri screenshots only")
    );
}

#[test]
fn profile_keyring_sources_are_scoped_and_do_not_leak_into_default() {
    let f = Fixture::new();
    theme_render::copy_tree(&source().join("config"), &f.root.join("config")).unwrap();
    let directory = f.root.join("profiles/lucky38/session/keyring");
    fs::create_dir_all(&directory).unwrap();
    for name in [
        "gnome-keyring-pkcs11.desktop",
        "gnome-keyring-secrets.desktop",
        "org.freedesktop.secrets.service",
    ] {
        fs::write(directory.join(name), "host-specific keyring").unwrap();
    }
    for profile in ["default", "lucky38"] {
        let targets = deploy_units::targets(
            &f.root,
            &f.paths.config,
            &f.paths.state,
            &f.paths.data,
            &Default::default(),
            false,
            profile,
        )
        .unwrap();
        let sources: Vec<_> = targets
            .iter()
            .filter(|(_, s)| s.to_string_lossy().contains("session/keyring"))
            .map(|(_, s)| s)
            .collect();
        assert_eq!(sources.len(), 3);
        for source in sources {
            assert_eq!(source.starts_with(&directory), profile == "lucky38");
        }
    }
    fs::remove_file(directory.join("org.freedesktop.secrets.service")).unwrap();
    symlink(
        "/etc/passwd",
        directory.join("org.freedesktop.secrets.service"),
    )
    .unwrap();
    assert!(
        deploy_units::targets(
            &f.root,
            &f.paths.config,
            &f.paths.state,
            &f.paths.data,
            &Default::default(),
            false,
            "lucky38"
        )
        .is_err()
    );
}

#[test]
fn shipped_default_keyring_and_lucky38_provider_remain_distinct() {
    let root = source();
    let default =
        fs::read_to_string(root.join("session/keyring/org.freedesktop.secrets.service")).unwrap();
    assert!(default.contains("SystemdService=dbus-org.freedesktop.secrets.service"));
    assert!(!default.contains("gnome-keyring-daemon"));
    let lucky = fs::read_to_string(
        root.join("profiles/lucky38/session/keyring/org.freedesktop.secrets.service"),
    )
    .unwrap();
    assert!(lucky.contains("gnome-keyring-daemon"));
    for name in [
        "gnome-keyring-pkcs11.desktop",
        "gnome-keyring-secrets.desktop",
    ] {
        assert!(
            fs::read_to_string(root.join("session/keyring").join(name))
                .unwrap()
                .contains("Hidden=true")
        );
    }
}

#[test]
fn legacy_theme_migration_preserves_custom_settings_and_exact_rollback() {
    let f = Fixture::new();
    let gtk3 = f.paths.config.join("gtk-3.0/gtk.css");
    let gtk4 = f.paths.config.join("gtk-4.0/gtk.css");
    let old = f.paths.config.join("environment.d/ryoku-session.conf");
    let new = f
        .paths
        .config
        .join("environment.d/session-desktop-foundation.conf");
    for parent in [
        gtk3.parent().unwrap(),
        gtk4.parent().unwrap(),
        old.parent().unwrap(),
    ] {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(
        &gtk3,
        "/* Ryoku palette -> classic GTK 3. */\n@define-color accent_color #80d5d2;\n",
    )
    .unwrap();
    fs::write(
        &gtk4,
        "/* Custom user styling, keep it */\nbutton { border-radius: 4px; }\n",
    )
    .unwrap();
    fs::write(&old, "# Ryoku session environment, read by systemd\nEDITOR=vim\nLOCAL_VALUE=keep\nXCURSOR_THEME=Bibata-Modern-Ice\nRYOKU_POLKIT_AGENT=1\nQML_IMPORT_PATH=$HOME/.local/lib/qt6/qml\nQT_QPA_PLATFORMTHEME=qt6ct\nGSK_RENDERER=gl\n").unwrap();
    fs::write(&new, "EDITOR=helix\nUSER_EXTENSION=keep-too\n").unwrap();
    fs::write(
        f.paths.config.join("environment.d/cursor.conf"),
        "XCURSOR_THEME=Adwaita\n",
    )
    .unwrap();
    let originals: Vec<_> = [&gtk3, &gtk4, &old, &new]
        .into_iter()
        .map(|p| {
            (
                p.clone(),
                fs::read(p).unwrap(),
                deploy::fingerprint(p).unwrap(),
            )
        })
        .collect();
    let mut manifest = f.manifest();
    let dry = deploy::targets(&source(), &f.paths, &manifest, false).unwrap();
    assert!(!f.paths.state.join("legacy").exists());
    assert!(dry.iter().any(|(p, _)| p == &gtk3));
    assert!(!dry.iter().any(|(p, _)| p == &gtk4));
    let targets = deploy::targets(&source(), &f.paths, &manifest, true).unwrap();
    let migrated =
        fs::read_to_string(f.paths.state.join("legacy/session-environment.conf")).unwrap();
    for expected in [
        "EDITOR=vim",
        "LOCAL_VALUE=keep",
        "EDITOR=helix",
        "USER_EXTENSION=keep-too",
        "QT_QPA_PLATFORMTHEME=qt6ct",
        "GSK_RENDERER=gl",
    ] {
        assert!(migrated.contains(expected));
    }
    assert!(migrated.find("EDITOR=helix").unwrap() > migrated.find("EDITOR=vim").unwrap());
    assert!(!migrated.contains("RYOKU_") && !migrated.contains("QML_IMPORT_PATH"));
    assert_eq!(
        fs::metadata(f.paths.state.join("legacy/session-environment.conf"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    for (path, source) in targets
        .iter()
        .filter(|(p, _)| [&gtk3, &old, &new].contains(&p))
    {
        if path == &gtk3 {
            theme_render::write(source, "/* Foundation test palette */").unwrap();
        }
        deploy::install_link(&f.paths, &mut manifest, path, source).unwrap();
    }
    assert!(!fs::read_to_string(&old).unwrap().contains("RYOKU_POLKIT"));
    assert!(
        fs::read_to_string(&new)
            .unwrap()
            .contains("LOCAL_VALUE=keep")
    );
    // Preserve Lucky38's existing precedence over cursor.conf.
    let mut files: Vec<_> = fs::read_dir(f.paths.config.join("environment.d"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let mut cursor = String::new();
    for path in files {
        for line in fs::read_to_string(path).unwrap().lines() {
            if let Some(value) = line.strip_prefix("XCURSOR_THEME=") {
                cursor = value.into();
            }
        }
    }
    assert_eq!(cursor, "Bibata-Modern-Ice");
    deploy::targets(&source(), &f.paths, &manifest, true).unwrap();
    assert_eq!(
        fs::read_to_string(f.paths.state.join("legacy/session-environment.conf")).unwrap(),
        migrated
    );
    deploy::restore_files(&f.paths, &mut manifest).unwrap();
    for (path, bytes, inode) in originals {
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(deploy::fingerprint(&path).unwrap(), inode);
    }
}

#[test]
fn gtk_migration_preserves_unknown_and_broken_user_symlinks() {
    let f = Fixture::new();
    let css = f.paths.config.join("gtk-3.0/gtk.css");
    fs::create_dir_all(css.parent().unwrap()).unwrap();
    symlink("missing-custom.css", &css).unwrap();
    let targets = deploy::targets(&source(), &f.paths, &f.manifest(), false).unwrap();
    assert!(!targets.iter().any(|(path, _)| path == &css));
    assert!(
        targets
            .iter()
            .any(|(path, _)| path == &f.paths.config.join("gtk-4.0/gtk.css"))
    );
    assert_eq!(
        fs::read_link(css).unwrap(),
        PathBuf::from("missing-custom.css")
    );
}

#[test]
fn interrupted_legacy_environment_migration_keeps_later_unowned_edits() {
    let f = Fixture::new();
    let old = f.paths.config.join("environment.d/ryoku-session.conf");
    let new = f
        .paths
        .config
        .join("environment.d/session-desktop-foundation.conf");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, "# Ryoku session environment\nEDITOR=vim\n").unwrap();
    fs::write(&new, "EDITOR=emacs\n").unwrap();
    let mut manifest = f.manifest();
    let targets = deploy::targets(&source(), &f.paths, &manifest, true).unwrap();
    let (_, stub) = targets.iter().find(|(p, _)| p == &old).unwrap();
    deploy::install_link(&f.paths, &mut manifest, &old, stub).unwrap();
    // Failure happened before adopting the new file; it still belongs to its owner.
    fs::write(&new, "EDITOR=helix\nLATER_EDIT=preserve\n").unwrap();
    let targets = deploy::targets(&source(), &f.paths, &manifest, true).unwrap();
    let (_, prepared) = targets.iter().find(|(p, _)| p == &new).unwrap();
    let contents = fs::read_to_string(prepared).unwrap();
    assert!(contents.contains("EDITOR=helix") && contents.contains("LATER_EDIT=preserve"));
    assert!(!contents.contains("EDITOR=emacs"));
    deploy::install_link(&f.paths, &mut manifest, &new, prepared).unwrap();
    deploy::restore_files(&f.paths, &mut manifest).unwrap();
    assert_eq!(
        fs::read_to_string(&new).unwrap(),
        "EDITOR=helix\nLATER_EDIT=preserve\n"
    );
    assert_eq!(
        fs::read_to_string(&old).unwrap(),
        "# Ryoku session environment\nEDITOR=vim\n"
    );
}
