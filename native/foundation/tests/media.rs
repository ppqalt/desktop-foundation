use desktop_foundationctl::{media, theme_render, theme_runtime, wallpapers};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "foundation-media-{}-{}",
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
fn ffmpeg(args: &[&str], output: &std::path::Path) {
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y"])
        .args(args)
        .arg(output)
        .status()
        .expect("ffmpeg required for media integration test");
    assert!(status.success());
}
fn available(command: &str) -> bool {
    Command::new(command).arg("-version").output().is_ok()
}
#[test]
fn media_probe_detects_actual_audio_and_video_streams() {
    if !available("ffmpeg") || !available("ffprobe") {
        eprintln!("Skipping media integration: ffmpeg/ffprobe unavailable");
        return;
    }
    let f = Fixture::new();
    let silent = f.0.join("silent-without-video-extension");
    let audio = f.0.join("audio.data");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=32x24:r=1:d=1",
            "-c:v",
            "mpeg4",
            "-f",
            "matroska",
        ],
        &silent,
    );
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=32x24:r=1:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "mpeg4",
            "-c:a",
            "pcm_s16le",
            "-shortest",
            "-f",
            "matroska",
        ],
        &audio,
    );
    let a = media::probe(&silent).unwrap();
    let b = media::probe(&audio).unwrap();
    assert_eq!(a["hasVideo"], true);
    assert_eq!(a["hasAudio"], false);
    assert_eq!(b["hasVideo"], true);
    assert_eq!(b["hasAudio"], true);
    assert_eq!(media::first_frame(&audio).unwrap().width(), 32);
    fs::write(f.0.join("bad"), b"not media").unwrap();
    assert!(media::probe(&f.0.join("bad")).is_err());
}
#[test]
fn jpeg_xl_discovery_and_preview_use_available_decoder() {
    if !available("ffmpeg") {
        eprintln!("Skipping JPEG XL integration: ffmpeg unavailable");
        return;
    }
    let encoders = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .unwrap();
    if !String::from_utf8_lossy(&encoders.stdout).contains("libjxl ") {
        eprintln!("Skipping JPEG XL integration: encoder unavailable");
        return;
    }
    let f = Fixture::new();
    let source = f.0.join("image.jxl");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=orange:s=32x24",
            "-frames:v",
            "1",
            "-c:v",
            "libjxl",
        ],
        &source,
    );
    let result =
        wallpapers::catalog_with_previews(&f.0, Some(&source), Some(&f.0.join("cache"))).unwrap();
    assert_eq!(result["images"].as_array().unwrap().len(), 1);
    assert_eq!(result["images"][0]["selected"], true);
    assert_eq!(result["images"][0]["width"], 32);
    let owner = f.0.join("checkout");
    fs::create_dir(&owner).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    for name in ["theme", "terminal", "config", "compositor"] {
        theme_render::copy_tree(&root.join(name), &owner.join(name)).unwrap();
    }
    fs::write(
        owner.join("compositor/niri/wallpaper.toml"),
        format!(
            "image = {}\nmode = 'fill'\n",
            serde_json::to_string(&source).unwrap()
        ),
    )
    .unwrap();
    let bin = f.0.join("bin");
    fs::create_dir(&bin).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for program in ["niri", "fish"] {
        let path = bin.join(program);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .arg("--root")
        .arg(&owner)
        .args(["theme-runtime", "ensure", "default"])
        .env("HOME", &f.0)
        .env("PATH", &path)
        .env("XDG_STATE_HOME", f.0.join("state"))
        .env("XDG_CACHE_HOME", f.0.join("xdg-cache"))
        .env_remove("NIRI_SOCKET")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let revision = theme_runtime::current(&f.0.join("state/desktop-foundation/theme"))
        .unwrap()
        .unwrap();
    assert!(revision.join("overview.png").is_file());
    assert!(revision.join("wallpaper.jxl").is_file());
    let swaybg = bin.join("swaybg");
    fs::write(
        &swaybg,
        "#!/bin/sh\n[ \"$1\" = --image ] || exit 1\nprintf '%s' \"$2\" > \"$TEST_IMAGE\"\n",
    )
    .unwrap();
    fs::set_permissions(swaybg, fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .arg("--root")
        .arg(&owner)
        .args(["wallpaper", "start"])
        .env("HOME", &f.0)
        .env("PATH", path)
        .env("NIRI_SOCKET", "/tmp/fake-niri")
        .env("XDG_STATE_HOME", f.0.join("state"))
        .env("XDG_CACHE_HOME", f.0.join("xdg-cache"))
        .env("TEST_IMAGE", f.0.join("rendered-path"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rendered = fs::read_to_string(f.0.join("rendered-path")).unwrap();
    assert_eq!(image::open(rendered).unwrap().width(), 32);
}
