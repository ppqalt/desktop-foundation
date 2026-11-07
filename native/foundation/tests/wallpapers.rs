use desktop_foundationctl::{theme_render as render, wallpapers};
use image::{Rgb, RgbImage};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "foundation-wallpapers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn image(path: &Path, color: [u8; 3]) {
    RgbImage::from_pixel(80, 50, Rgb(color)).save(path).unwrap();
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
#[test]
fn catalog_is_sorted_filters_invalid_files_and_tracks_selection() {
    let f = Fixture::new();
    image(&f.0.join("B.png"), [20, 50, 150]);
    image(&f.0.join("a space.png"), [200, 50, 20]);
    fs::write(f.0.join("bad.png"), "not an image").unwrap();
    fs::write(f.0.join("notes.md"), "hello").unwrap();
    fs::create_dir(f.0.join("folder.jpg")).unwrap();
    let catalog = wallpapers::catalog(&f.0, Some(&f.0.join("B.png"))).unwrap();
    let images = catalog["images"].as_array().unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0]["name"], "a space.png");
    assert!(images[0]["url"].as_str().unwrap().contains("a%20space.png"));
    assert_eq!(images[1]["selected"], true);
}
#[test]
fn previews_are_generated_on_demand_and_reject_foreign_files() {
    let f = Fixture::new();
    let directory = f.0.join("images");
    let cache = f.0.join("cache");
    fs::create_dir(&directory).unwrap();
    let first = directory.join("a.png");
    let second = directory.join("b.png");
    let outside = f.0.join("outside.png");
    image(&first, [20, 50, 150]);
    image(&second, [200, 50, 20]);
    image(&outside, [40, 160, 70]);
    let catalog = wallpapers::catalog(&directory, None).unwrap();
    assert_eq!(catalog["images"].as_array().unwrap().len(), 2);
    assert!(!cache.exists(), "listing must not generate previews");
    let result = wallpapers::preview_for(&first, &directory, &cache).unwrap();
    assert!(result["url"].as_str().unwrap().starts_with("file://"));
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    assert!(wallpapers::preview_for(&outside, &directory, &cache).is_err());
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
}
#[test]
fn missing_directory_is_an_empty_state() {
    let f = Fixture::new();
    let catalog = wallpapers::catalog(&f.0.join("missing"), None).unwrap();
    assert_eq!(catalog["available"], false);
    assert_eq!(catalog["images"].as_array().unwrap().len(), 0);
}
#[test]
fn expanded_formats_generate_real_previews() {
    let f = Fixture::new();
    let directory = f.0.join("images");
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("test.AVIF"),
        include_bytes!("fixtures/wallpaper.avif"),
    )
    .unwrap();
    for extension in ["tga", "qoi", "ppm", "bmp"] {
        image(&directory.join(format!("test.{extension}")), [200, 60, 20]);
    }
    fs::write(directory.join("invalid.avif"), b"invalid").unwrap();
    let cache = f.0.join("cache");
    let catalog = wallpapers::catalog_with_previews(&directory, None, Some(&cache)).unwrap();
    let images = catalog["images"].as_array().unwrap();
    assert_eq!(images.len(), 5, "{catalog}");
    assert!(images.iter().any(|item| item["name"] == "test.AVIF"));
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 5);
    for entry in fs::read_dir(cache).unwrap() {
        let preview = image::open(entry.unwrap().path()).unwrap().to_rgba8();
        assert_eq!(preview.dimensions(), (480, 288));
        assert_eq!(preview.get_pixel(240, 144).0[3], 255);
    }
}
#[test]
fn native_preview_is_visible_rounded_cached_and_replaced_when_image_changes() {
    let f = Fixture::new();
    let directory = f.0.join("images");
    fs::create_dir(&directory).unwrap();
    let cache = f.0.join("cache");
    let source = directory.join("a.png");
    image(&source, [20, 50, 150]);
    wallpapers::catalog_with_previews(&directory, None, Some(&cache)).unwrap();
    let first = fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::metadata(&first).unwrap().modified().unwrap();
    let decoded = image::open(&first).unwrap().to_rgba8();
    assert_eq!(decoded.dimensions(), (480, 288));
    assert_eq!(decoded.get_pixel(0, 0).0[3], 0);
    assert_eq!(decoded.get_pixel(240, 144).0, [20, 50, 150, 255]);
    wallpapers::catalog_with_previews(&directory, None, Some(&cache)).unwrap();
    assert_eq!(fs::metadata(&first).unwrap().modified().unwrap(), original);
    image(&source, [200, 50, 20]);
    wallpapers::catalog_with_previews(&directory, None, Some(&cache)).unwrap();
    assert!(!first.exists());
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
}
#[test]
fn qml_grid_keyboard_selection_failure_and_preview_pixels_are_verified_offscreen() {
    if !Command::new("sh")
        .args(["-c", "command -v quickshell >/dev/null 2>&1"])
        .status()
        .is_ok_and(|status| status.success())
    {
        eprintln!("Skipping wallpaper QML integration test: Quickshell is unavailable");
        return;
    }
    let f = Fixture::new();
    let directory = f.0.join("images");
    fs::create_dir(&directory).unwrap();
    for (name, color) in [
        ("a.png", [20, 50, 150]),
        ("b.png", [180, 50, 20]),
        ("c.png", [40, 160, 70]),
        ("d.png", [120, 60, 170]),
    ] {
        image(&directory.join(name), color);
    }
    let cache = f.0.join("cache");
    let catalog =
        wallpapers::catalog_with_previews(&directory, Some(&directory.join("a.png")), Some(&cache))
            .unwrap();
    let data = f.0.join("catalog.json");
    render::write_json(&data, &catalog).unwrap();
    render::copy_tree(&root().join("shell"), &f.0.join("shell")).unwrap();
    let qml = fs::read_to_string(root().join("tests/qml/wallpaper-grid.qml"))
        .unwrap()
        .replace("../../shell/", "shell/");
    fs::write(f.0.join("shell.qml"), qml).unwrap();
    let backend = f.0.join("backend");
    fs::write(&backend,"#!/bin/sh\ncase \"$3 $4\" in\n'wallpapers list') /bin/cat \"$DF_WALLPAPER_CATALOG\";;\n'wallpapers preview') python3 -c 'import json,sys; items=json.load(open(sys.argv[1]))[\"images\"]; print(json.dumps(next(x for x in items if x[\"path\"] == sys.argv[2])))' \"$DF_WALLPAPER_CATALOG\" \"$5\";;\n'theme-runtime wallpaper') printf '%s\\n' \"$5\" >> \"$DF_WALLPAPER_CALLS\"; /bin/sleep 0.1; case \"$5\" in */c.png) printf 'inert failure' >&2; exit 1;; esac;;\n*) exit 2;;\nesac\n").unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
    let capture = f.0.join("capture.png");
    let calls = f.0.join("calls");
    let result = Command::new("timeout")
        .args(["12s", "quickshell", "-p"])
        .arg(f.0.join("shell.qml"))
        .env("DF_WALLPAPER_BACKEND", backend)
        .env("DF_WALLPAPER_CATALOG", data)
        .env("DF_WALLPAPER_CALLS", &calls)
        .env("DF_WALLPAPER_CAPTURE", &capture)
        .env("DF_FOUNDATION_ROOT", root())
        .env("XDG_STATE_HOME", f.0.join("state"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.status.success() && log.contains("WALLPAPER_CHECKS_PASSED"),
        "{log}"
    );
    assert!(
        !log.contains("TypeError") && !log.contains("ReferenceError"),
        "{log}"
    );
    assert_eq!(fs::read_to_string(calls).unwrap().lines().count(), 2);
    let image = image::open(&capture).unwrap().to_rgb8();
    // First two source colors must be visible in the rendered image, not merely
    // loaded into Image objects while an effect masks every pixel away.
    let pixels = image.pixels();
    let mut blue = 0;
    let mut orange = 0;
    for p in pixels {
        if p.0[2] > 100 && p.0[0] < 50 {
            blue += 1;
        }
        if p.0[0] > 100 && p.0[1] < 80 && p.0[2] < 50 {
            orange += 1;
        }
    }
    assert!(
        blue > 1000 && orange > 1000,
        "preview pixels missing: blue={blue} orange={orange}"
    );
    if let Some(path) = std::env::var_os("DF_WALLPAPER_TEST_CAPTURE") {
        fs::copy(capture, path).unwrap();
    }
}

#[test]
fn qml_only_requests_nearby_previews_and_loads_more_when_scrolled() {
    if !Command::new("sh")
        .args(["-c", "command -v quickshell >/dev/null 2>&1"])
        .status()
        .is_ok_and(|status| status.success())
    {
        eprintln!("Skipping wallpaper QML integration test: Quickshell is unavailable");
        return;
    }
    let f = Fixture::new();
    let directory = f.0.join("images");
    fs::create_dir(&directory).unwrap();
    for index in 0..36 {
        image(&directory.join(format!("{index:02}.png")), [20, 50, 150]);
    }
    let data = f.0.join("catalog.json");
    render::write_json(&data, &wallpapers::catalog(&directory, None).unwrap()).unwrap();
    render::copy_tree(&root().join("shell"), &f.0.join("shell")).unwrap();
    let qml = fs::read_to_string(root().join("tests/qml/wallpaper-lazy.qml"))
        .unwrap()
        .replace("../../shell/", "shell/");
    fs::write(f.0.join("shell.qml"), qml).unwrap();
    let backend = f.0.join("backend");
    fs::write(&backend, r##"#!/bin/sh
case "$3 $4" in
'wallpapers list') /bin/cat "$DF_WALLPAPER_CATALOG";;
'wallpapers preview') python3 -c 'import json,sys; items=json.load(open(sys.argv[1]))["images"]; print(json.dumps(next(x for x in items if x["path"] == sys.argv[2])))' "$DF_WALLPAPER_CATALOG" "$5";;
*) exit 2;;
esac
"##).unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new("timeout")
        .args(["8s", "quickshell", "-p"])
        .arg(f.0.join("shell.qml"))
        .env("DF_WALLPAPER_BACKEND", backend)
        .env("DF_WALLPAPER_CATALOG", data)
        .env("DF_FOUNDATION_ROOT", root())
        .env("XDG_STATE_HOME", f.0.join("state"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.status.success() && log.contains("LAZY_WALLPAPER_CHECKS_PASSED"),
        "{log}"
    );
    assert!(
        !log.contains("TypeError") && !log.contains("ReferenceError"),
        "{log}"
    );
}
