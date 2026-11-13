//! Inert Qt colour/configuration transactions, not the owner's live configuration.
use desktop_foundationctl::{
    deploy::{self, Manifest, Paths},
    invalid, qt_theme, theme_render,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    paths: Paths,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-qt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = Paths {
            home: base.join("home"),
            config: base.join("config"),
            data: base.join("data"),
            state: base.join("state"),
        };
        fs::create_dir_all(&paths.state).unwrap();
        fs::create_dir_all(paths.config.join("qt6ct")).unwrap();
        fs::write(paths.config.join("kdeglobals"),"# owner comment\n[General]\nColorScheme=Ryoku\nfont=Custom Font,12\nfixed=Code Font,11\n[Icons]\nTheme=WhiteSur-dark\n[KDE]\nwidgetStyle=Darkly\ncontrast=0\n[Colors:Window]\nBackgroundNormal=#123456\n").unwrap();
        fs::write(paths.config.join("qt6ct/qt6ct.conf"),"# user settings\n[Appearance]\nstyle=Darkly\nicon_theme=WhiteSur-dark\ncustom_palette=true\ncolor_scheme_path=/old/Darkly.colors\n[Fonts]\ngeneral=My Font,12\n[Interface]\nwheel_scroll_lines=5\n").unwrap();
        Self { base, paths }
    }
    fn palette(&self) -> Value {
        theme_render::read(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../theme/generated.json"),
        )
        .unwrap()
    }
    fn current(&self) -> PathBuf {
        self.paths.state.join("qt/current").canonicalize().unwrap()
    }
    fn manifest(&self) -> Manifest {
        Manifest {
            root: self.base.clone(),
            entries: Vec::new(),
            profile: None,
            compositor: None,
            extra: BTreeMap::new(),
        }
    }
    fn adopt(&self, manifest: &mut Manifest) {
        for (path, source) in qt_theme::targets(&self.paths, manifest).unwrap() {
            deploy::install_link(&self.paths, manifest, &path, &source).unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn native_qt_palette_has_all_roles_paired_selection_and_visible_bright_text() {
    let f = Fixture::new();
    let p = f.palette();
    let text = qt_theme::palette(&p).unwrap();
    for line in text.lines().filter(|line| line.contains('=')) {
        let (name, colors) = line.split_once('=').unwrap();
        let colors: Vec<_> = colors.split(", ").collect();
        assert_eq!(colors.len(), 22);
        assert_eq!(
            colors[7],
            format!(
                "#ff{}",
                &p[if name == "disabled_colors" {
                    "muted"
                } else {
                    "foreground"
                }]
                .as_str()
                .unwrap()[1..]
            )
        );
        if name == "active_colors" {
            assert_eq!(
                colors[12],
                format!("#ff{}", &p["accent"].as_str().unwrap()[1..])
            );
            assert_eq!(
                colors[13],
                format!("#ff{}", &p["background"].as_str().unwrap()[1..])
            );
        }
    }
    let mut bad = p;
    bad["accent"] = json!("red\n[Other]");
    assert!(qt_theme::palette(&bad).is_err());
}
#[test]
fn adopting_legacy_qt_preferences_preserves_styles_fonts_and_exact_originals() {
    let f = Fixture::new();
    let p = f.palette();
    let originals: Vec<_> = ["kdeglobals", "qt6ct/qt6ct.conf"]
        .iter()
        .map(|name| {
            let path = f.paths.config.join(name);
            (
                path.clone(),
                fs::read(&path).unwrap(),
                deploy::fingerprint(&path).unwrap(),
            )
        })
        .collect();
    let mut m = f.manifest();
    assert!(!qt_theme::ready(&f.paths, &p));
    qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    assert!(qt_theme::ready(&f.paths, &p));
    f.adopt(&mut m);
    let kde = fs::read_to_string(f.paths.config.join("kdeglobals")).unwrap();
    let qt = fs::read_to_string(f.paths.config.join("qt6ct/qt6ct.conf")).unwrap();
    for expected in [
        "font=Custom Font,12",
        "fixed=Code Font,11",
        "Theme=WhiteSur-dark",
        "widgetStyle=Darkly",
        "contrast=0",
        "# owner comment",
    ] {
        assert!(kde.contains(expected), "missing {expected}");
    }
    for expected in [
        "style=Darkly",
        "icon_theme=WhiteSur-dark",
        "general=My Font,12",
        "wheel_scroll_lines=5",
        "# user settings",
    ] {
        assert!(qt.contains(expected));
    }
    assert!(!kde.contains("ColorScheme=Ryoku") && !kde.contains("#123456"));
    assert!(kde.contains("ColorScheme=DesktopFoundation"));
    assert!(qt.contains("qt6ct/colors/DesktopFoundation.conf"));
    for _ in 0..3 {
        qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    }
    assert_eq!(
        fs::read_to_string(f.paths.config.join("kdeglobals")).unwrap(),
        kde,
        "repeated generation must be idempotent"
    );
    assert_eq!(
        fs::read_to_string(f.paths.config.join("qt6ct/qt6ct.conf")).unwrap(),
        qt
    );
    deploy::restore_files(&f.paths, &mut m).unwrap();
    for (path, bytes, inode) in originals {
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(deploy::fingerprint(&path).unwrap(), inode);
    }
}
#[test]
fn rollback_uses_current_preferences_and_failed_activation_restores_pointer() {
    let f = Fixture::new();
    let p = f.palette();
    qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    let mut m = f.manifest();
    f.adopt(&mut m);
    let old = f.current();
    let qt_config = f.paths.config.join("qt6ct/qt6ct.conf");
    let original_qt = fs::read(&qt_config).unwrap();
    fs::write(&qt_config, "x".repeat(256 * 1024 + 1)).unwrap();
    assert!(qt_theme::with_assets(&f.paths, &p, || Ok(())).is_err());
    assert_eq!(f.current(), old);
    assert_eq!(fs::metadata(&qt_config).unwrap().len(), 256 * 1024 + 1);
    fs::write(&qt_config, original_qt).unwrap();

    let mut blue = p.clone();
    blue["accent"] = json!("#99bbff");
    assert!(
        qt_theme::with_assets(&f.paths, &blue, || Err::<(), _>(invalid(
            "inert reload failed"
        )))
        .is_err()
    );
    assert_eq!(f.current(), old);
    assert!(qt_theme::ready(&f.paths, &p));
    // Include in-place application settings changes, not only atomic file replacements.
    let path = f.paths.config.join("kdeglobals");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("Custom Font,12", "New Font,15");
    fs::write(&path, text).unwrap();
    qt_theme::with_assets(&f.paths, &blue, || Ok(())).unwrap();
    qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("font=New Font,15")
    );
    assert!(qt_theme::ready(&f.paths, &p));
    fs::remove_file(f.current().join("DesktopFoundation.colors")).unwrap();
    assert!(!qt_theme::ready(&f.paths, &p));
    qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    assert!(qt_theme::ready(&f.paths, &p));
}
#[test]
fn optional_qt_integration_preserves_custom_schemes_links_and_retains_bounded_history() {
    let f = Fixture::new();
    fs::write(
        f.paths.config.join("kdeglobals"),
        "[General]\nColorScheme=MyCustomScheme\n",
    )
    .unwrap();
    let qt = f.paths.config.join("qt6ct/qt6ct.conf");
    fs::remove_file(&qt).unwrap();
    symlink("missing-custom.conf", &qt).unwrap();
    let targets = qt_theme::targets(&f.paths, &f.manifest()).unwrap();
    assert!(
        !targets
            .iter()
            .any(|(p, _)| p == &qt || p == &f.paths.config.join("kdeglobals"))
    );
    let p = f.palette();
    for _ in 0..10 {
        qt_theme::with_assets(&f.paths, &p, || Ok(())).unwrap();
    }
    assert!(
        fs::read_dir(f.paths.state.join("qt/revisions"))
            .unwrap()
            .count()
            <= 7
    );
    assert_eq!(fs::read_link(qt).unwrap(), Path::new("missing-custom.conf"));
    let dir = f.paths.state.join("qt");
    fs::remove_file(dir.join("current")).unwrap();
    fs::write(dir.join("current"), "unowned content").unwrap();
    assert!(qt_theme::with_assets(&f.paths, &p, || Ok(())).is_err());
    assert_eq!(
        fs::read_to_string(dir.join("current")).unwrap(),
        "unowned content"
    );
}

#[test]
fn fresh_install_receives_native_palettes_and_existing_custom_schemes_are_optional() {
    let f = Fixture::new();
    fs::remove_file(f.paths.config.join("kdeglobals")).unwrap();
    fs::remove_file(f.paths.config.join("qt6ct/qt6ct.conf")).unwrap();
    let before = qt_theme::targets(&f.paths, &f.manifest()).unwrap();
    assert_eq!(before.len(), 4);
    qt_theme::with_assets(&f.paths, &f.palette(), || Ok(())).unwrap();
    let mut manifest = f.manifest();
    f.adopt(&mut manifest);
    assert!(
        fs::read_to_string(f.paths.config.join("kdeglobals"))
            .unwrap()
            .contains("ColorScheme=DesktopFoundation")
    );
    assert!(
        fs::read_to_string(f.paths.config.join("qt6ct/qt6ct.conf"))
            .unwrap()
            .contains("custom_palette=true")
    );
    // A different owner never loses an explicitly selected scheme to automatic setup.
    let g = Fixture::new();
    fs::write(
        g.paths.config.join("kdeglobals"),
        "[General]\nColorScheme=OtherTheme\n",
    )
    .unwrap();
    fs::write(
        g.paths.config.join("qt6ct/qt6ct.conf"),
        "[Appearance]\ncolor_scheme_path=/custom/Personal.colors\nstyle=Fusion\n",
    )
    .unwrap();
    assert_eq!(qt_theme::targets(&g.paths, &g.manifest()).unwrap().len(), 2);
}
