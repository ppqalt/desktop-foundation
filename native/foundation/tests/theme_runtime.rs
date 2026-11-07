//! Native transaction checks: private files and callbacks, never the live desktop.
use desktop_foundationctl::{backdrop, invalid, theme_render as render, theme_runtime as runtime};
use image::{ImageFormat, RgbImage};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "foundation-theme-rust-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn revision(home: &Path, name: &str, owner: &Path) -> PathBuf {
    let path = home.join(name);
    fs::create_dir_all(path.join("brave")).unwrap();
    render::write_json(&path.join("metadata.json"), &json!({"root":owner})).unwrap();
    render::write(&path.join("brave/manifest.json"), name).unwrap();
    path
}
#[test]
fn publication_keeps_stable_theme_identity_and_previous() {
    let f = Fixture::new();
    let old = revision(&f.0, "old", &f.0);
    let new = revision(&f.0, "new", &f.0);
    runtime::publish_with(&f.0, &f.0, &old, || Ok(())).unwrap();
    runtime::publish_with(&f.0, &f.0, &new, || Ok(())).unwrap();
    assert_eq!(runtime::current(&f.0).unwrap(), Some(new));
    assert_eq!(f.path("previous").canonicalize().unwrap(), old);
    assert_eq!(
        fs::read_to_string(f.path("brave/manifest.json")).unwrap(),
        "new"
    );
    assert!(!f.path("brave").is_symlink());
}
#[test]
fn reload_failure_restores_pointer_manifest_and_previous() {
    let f = Fixture::new();
    let old = revision(&f.0, "old", &f.0);
    let new = revision(&f.0, "new", &f.0);
    runtime::publish_with(&f.0, &f.0, &old, || Ok(())).unwrap();
    let mut calls = 0;
    assert!(
        runtime::publish_with(&f.0, &f.0, &new, || {
            calls += 1;
            Err(invalid("inert reload failure"))
        })
        .is_err()
    );
    assert_eq!(calls, 2);
    assert_eq!(runtime::current(&f.0).unwrap(), Some(old));
    assert_eq!(
        fs::read_to_string(f.path("brave/manifest.json")).unwrap(),
        "old"
    );
    assert!(!f.path("previous").exists());
}
#[test]
fn first_publication_failure_leaves_no_active_theme() {
    let f = Fixture::new();
    let new = revision(&f.0, "new", &f.0);
    assert!(runtime::publish_with(&f.0, &f.0, &new, || Err(invalid("inert failure"))).is_err());
    assert!(runtime::current(&f.0).unwrap().is_none());
    assert!(!f.path("brave/manifest.json").exists());
}
#[test]
fn foreign_revision_never_becomes_active() {
    let f = Fixture::new();
    let old = revision(&f.0, "old", &f.0);
    let new = revision(&f.0, "new", Path::new("/different-checkout"));
    runtime::publish_with(&f.0, &f.0, &old, || Ok(())).unwrap();
    assert!(runtime::publish_with(&f.0, &f.0, &new, || panic!("must not reload")).is_err());
    assert_eq!(runtime::current(&f.0).unwrap(), Some(old));
}
#[test]
fn renderers_keep_semantic_roles_and_reject_injection() {
    let root = root();
    let f = Fixture::new();
    let p = render::read(&root.join("theme/generated.json")).unwrap();
    render::render(&root, &f.0, &p, "default", false).unwrap();
    assert_eq!(render::read(&f.path("semantic.json")).unwrap(), p);
    let kitty = fs::read_to_string(f.path("terminal/kitty/colors.conf")).unwrap();
    assert!(kitty.contains(&format!("background {}", p["background"].as_str().unwrap())));
    assert!(
        fs::read_to_string(f.path("terminal/kitty/kitty.conf"))
            .unwrap()
            .contains("confirm_os_window_close 0")
    );
    assert!(
        fs::read_to_string(f.path("btop/desktop-foundation.theme"))
            .unwrap()
            .contains(&format!(
                "theme[hi_fg]=\"{}\"",
                p["accent"].as_str().unwrap()
            ))
    );
    let niri = fs::read_to_string(f.path("niri.kdl")).unwrap();
    assert!(niri.contains("always-center-single-column"));
    assert!(niri.contains("center-focused-column \"never\""));
    assert!(!niri.contains('@'));
    assert!(render::niri(&root, "../../outside", Some(&p)).is_err());
    let mut invalid = p.clone();
    invalid["accent"] = json!("red\nexec shell");
    assert!(render::brave(&invalid).is_err());
    match Command::new("niri")
        .args(["validate", "-c"])
        .arg(f.path("niri.kdl"))
        .output()
    {
        Ok(output) => assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Skipping Niri validation: executable unavailable");
        }
        Err(error) => panic!("Failed to run Niri validation: {error}"),
    }
}
#[test]
fn native_decoding_supports_png_and_jpeg_and_rejects_invalid_files() {
    let f = Fixture::new();
    let image = RgbImage::from_pixel(16, 12, image::Rgb([240, 70, 10]));
    for (name, format) in [
        ("orange.png", ImageFormat::Png),
        ("orange.jpg", ImageFormat::Jpeg),
    ] {
        image.save_with_format(f.path(name), format).unwrap();
        let decoded = backdrop::decode(&f.path(name)).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 12));
    }
    fs::write(f.path("bad.png"), b"not an image").unwrap();
    assert!(backdrop::decode(&f.path("bad.png")).is_err());
}
#[test]
fn staging_failure_cleans_revision_and_does_not_touch_current() {
    let f = Fixture::new();
    let p = render::read(&root().join("theme/generated.json")).unwrap();
    fs::write(f.path("bad.png"), b"invalid").unwrap();
    assert!(
        runtime::stage(
            &root(),
            &f.0,
            &p,
            &f.path("bad.png"),
            "fill",
            false,
            "default"
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(f.path("revisions")).unwrap().count(), 0);
    assert!(runtime::current(&f.0).unwrap().is_none());
}
#[test]
fn ensure_has_no_python_dependency_or_live_reloads() {
    let f = Fixture::new();
    let owner = f.path("checkout");
    fs::create_dir(&owner).unwrap();
    for name in ["theme", "terminal", "config", "compositor"] {
        render::copy_tree(&root().join(name), &owner.join(name)).unwrap();
    }
    fs::create_dir_all(owner.join("wallpapers")).unwrap();
    RgbImage::from_pixel(24, 16, image::Rgb([25, 65, 150]))
        .save(owner.join("wallpapers/test.png"))
        .unwrap();
    fs::write(
        owner.join("compositor/niri/wallpaper.toml"),
        "image = '~/unavailable/test.png'\nmode = 'fill'\n",
    )
    .unwrap();
    let bin = f.path("bin");
    fs::create_dir(&bin).unwrap();
    for program in ["niri", "fish"] {
        use std::os::unix::fs::PermissionsExt;
        let path = bin.join(program);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .args(["--root"])
            .arg(&owner)
            .args(["theme-runtime", "ensure", "default"])
            .env("HOME", &f.0)
            .env("XDG_STATE_HOME", f.path("state"))
            .env("XDG_CACHE_HOME", f.path("cache"))
            .env("PATH", &bin)
            .env_remove("NIRI_SOCKET")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let home = f.path("state/desktop-foundation/theme");
    let revision = runtime::current(&home).unwrap().unwrap();
    assert!(revision.join("overview.png").is_file());
    assert!(revision.join("terminal/kitty/kitty.conf").is_file());
    assert_eq!(
        render::read(&revision.join("metadata.json")).unwrap()["renderer"],
        "rust-v1"
    );
    assert_eq!(
        render::read(&revision.join("metadata.json")).unwrap()["input"],
        owner.join("wallpapers/test.png").to_string_lossy().as_ref()
    );
}

#[test]
fn native_generation_caches_once_and_failure_keeps_active_files() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let bin = f.path("bin");
    fs::create_dir(&bin).unwrap();
    let output_file = f.path("matugen.json");
    render::write_json(&output_file,&json!({"colors":{"dark":render::read(&root().join("theme/fallback/material.json")).unwrap()}})).unwrap();
    let log = f.path("calls");
    let tool = bin.join("matugen");
    fs::write(
        &tool,
        "#!/bin/sh\nprintf 'called\\n' >> \"$DF_THEME_CALLS\"\n/bin/cat \"$DF_THEME_OUTPUT\"\n",
    )
    .unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let image = f.path("blue.png");
    RgbImage::from_pixel(24, 16, image::Rgb([20, 60, 150]))
        .save(&image)
        .unwrap();
    let home = f.path("state/desktop-foundation/theme");
    fs::create_dir_all(&home).unwrap();
    let old = revision(&home, "old", &root());
    runtime::publish_with(&root(), &home, &old, || Ok(())).unwrap();
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(root())
            .args(["theme-runtime", "generate"])
            .arg(&image)
            .env("HOME", &f.0)
            .env("PATH", &bin)
            .env("XDG_STATE_HOME", f.path("state"))
            .env("XDG_CACHE_HOME", f.path("cache"))
            .env("DF_THEME_OUTPUT", &output_file)
            .env("DF_THEME_CALLS", &log)
            .env_remove("NIRI_SOCKET")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap()
    };
    // Supply repository wallpaper config without making it the active bundle.
    render::write(
        &old.join("wallpaper.toml"),
        &format!(
            "image = {}\nmode = \"fill\"\n",
            serde_json::to_string(&image).unwrap()
        ),
    )
    .unwrap();
    for _ in 0..2 {
        let result = invoke();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let palette: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(palette["material"].is_object());
    }
    assert_eq!(fs::read_to_string(&log).unwrap(), "called\n");
    // A new hash misses the palette cache, then malformed Matugen output fails.
    RgbImage::from_pixel(24, 16, image::Rgb([245, 85, 15]))
        .save(&image)
        .unwrap();
    fs::write(&output_file, "not JSON").unwrap();
    let result = invoke();
    assert!(!result.status.success());
    assert_eq!(runtime::current(&home).unwrap(), Some(old));
    assert_eq!(
        fs::read_to_string(home.join("brave/manifest.json")).unwrap(),
        "old"
    );
}

#[test]
fn avif_theme_generation_converts_for_matugen_and_keeps_original_source() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let bin = f.path("bin");
    fs::create_dir(&bin).unwrap();
    let output = f.path("matugen.json");
    render::write_json(&output, &json!({"colors":{"dark":render::read(&root().join("theme/fallback/material.json")).unwrap()}})).unwrap();
    let tool = bin.join("matugen");
    fs::write(&tool, "#!/bin/sh\nfor arg do last=$arg; done\ncase $last in *.png) test -f \"$last\" || exit 2;; *) exit 3;; esac\n/bin/cat \"$DF_THEME_OUTPUT\"\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let source = f.path("wallpaper.avif");
    fs::write(&source, include_bytes!("fixtures/wallpaper.avif")).unwrap();
    let hdr = f.path("wallpaper.hdr");
    image::DynamicImage::ImageRgb32F(image::ImageBuffer::from_pixel(
        12,
        8,
        image::Rgb([0.3_f32, 0.4, 0.5]),
    ))
    .save(&hdr)
    .unwrap();
    let run = |source: &Path| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(root())
            .args(["theme-runtime", "generate"])
            .arg(source)
            .env("HOME", &f.0)
            .env("PATH", &bin)
            .env("XDG_STATE_HOME", f.path("state"))
            .env("XDG_CACHE_HOME", f.path("cache"))
            .env("DF_THEME_OUTPUT", &output)
            .env_remove("NIRI_SOCKET")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap()
    };
    for input in [&source, &hdr] {
        let result = run(input);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let palette: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(palette["material"].is_object());
    }
    let converted = f.path("cache/desktop-foundation/wallpaper-compatible");
    let files: Vec<_> = fs::read_dir(converted).unwrap().collect();
    assert_eq!(files.len(), 2);
    assert_eq!(
        image::open(files[0].as_ref().unwrap().path())
            .unwrap()
            .width(),
        12
    );
    assert_eq!(
        fs::read(source).unwrap(),
        include_bytes!("fixtures/wallpaper.avif")
    );
}

#[test]
fn preflight_update_runs_without_python_or_package_manager() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let owner = f.path("checkout");
    fs::create_dir(&owner).unwrap();
    for name in ["theme", "config", "compositor"] {
        render::copy_tree(&root().join(name), &owner.join(name)).unwrap();
    }
    let binary = owner.join("native/foundation/target/release/desktop-foundationctl");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_desktop-foundationctl"), binary).unwrap();
    let bin = f.path("bin");
    fs::create_dir(&bin).unwrap();
    for program in ["niri", "quickshell", "kitty", "slangc", "fish", "fc-cache"] {
        let path = bin.join(program);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .arg("--root")
        .arg(&owner)
        .args(["preflight", "--update"])
        .env("PATH", &bin)
        .env("HOME", &f.0)
        .env_remove("NIRI_SOCKET")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).starts_with("PASS"));
    assert!(!owner.join("manifest.json").exists());
}

#[test]
fn native_spotify_binding_preserves_unowned_settings_and_restores_existing_journal() {
    let f = Fixture::new();
    let config = f.path("config/spicetify");
    fs::create_dir_all(config.join("Themes/marketplace")).unwrap();
    let original = "; custom\n[Setting]\ncurrent_theme = marketplace\ncolor_scheme = Base\nreplace_colors = 0\ninject_css = 1\n[AdditionalOptions]\ncustom_apps = marketplace|lyrics\n";
    fs::write(config.join("config-xpui.ini"), original).unwrap();
    let colors = "[Base]\ntext = ffffff\n";
    fs::write(config.join("Themes/marketplace/color.ini"), colors).unwrap();
    let invoke = |action| {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(root())
            .args(["spotify-theme", action])
            .env("HOME", &f.0)
            .env("XDG_CONFIG_HOME", f.path("config"))
            .env("XDG_STATE_HOME", f.path("state"))
            .env("PATH", f.path("empty-bin"))
            .output()
            .unwrap()
    };
    let result = invoke("install");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let modified = fs::read_to_string(config.join("config-xpui.ini")).unwrap();
    assert!(
        modified
            .contains("inject_css = 1\n[AdditionalOptions]\ncustom_apps = marketplace|lyrics\n")
    );
    assert!(
        fs::read_to_string(config.join("Themes/marketplace/color.ini"))
            .unwrap()
            .starts_with(colors)
    );
    let result = invoke("restore");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(config.join("config-xpui.ini")).unwrap(),
        original
    );
    assert_eq!(
        fs::read_to_string(config.join("Themes/marketplace/color.ini")).unwrap(),
        colors
    );
    assert!(
        !f.path("state/desktop-foundation/spotify/theme-binding.json")
            .exists()
    );
}
#[test]
fn native_spotify_refuses_externally_changed_owned_setting() {
    let f = Fixture::new();
    let config = f.path("config/spicetify");
    fs::create_dir_all(&config).unwrap();
    let original =
        "[Setting]\ncurrent_theme = marketplace\ncolor_scheme = External\nreplace_colors = 1\n";
    fs::write(config.join("config-xpui.ini"), original).unwrap();
    render::write_json(&f.path("state/desktop-foundation/spotify/theme-binding.json"),&json!({"original":{"color_scheme":"Base","replace_colors":"0"},"owned":{"color_scheme":"DesktopFoundation","replace_colors":"1"}})).unwrap();
    for action in ["install", "refresh", "restore"] {
        let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(root())
            .args(["spotify-theme", action])
            .env("HOME", &f.0)
            .env("XDG_CONFIG_HOME", f.path("config"))
            .env("XDG_STATE_HOME", f.path("state"))
            .env("PATH", f.path("empty-bin"))
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_eq!(
            fs::read_to_string(config.join("config-xpui.ini")).unwrap(),
            original
        );
    }
}
