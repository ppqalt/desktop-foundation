//! Real, finite media jobs. No screen capture, PipeWire or user session mutation.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const ID: &str = "0123456789abcdef00000001";
struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Option<Self> {
        if !desktop_foundationctl::apps::executable("ffmpeg") {
            eprintln!("Real replay media fixtures require FFmpeg");
            return None;
        }
        let root = std::env::temp_dir().join(format!(
            "df-editor-media-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for path in ["config", "user", "state", "runtime", "videos"] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(root.join("runtime"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            root.join("config/recording.toml"),
            format!(
                "directory = {}\n",
                serde_json::to_string(&root.join("videos")).unwrap()
            ),
        )
        .unwrap();
        let source = root
            .join("state/desktop-foundation/replay-editor")
            .join(ID)
            .join("source.mkv");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::set_permissions(source.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
        let out = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=60:duration=6",
                "-f",
                "lavfi",
                "-i",
                r"aevalsrc=if(between(t\,2\,2.1)\,0.5*sin(2*PI*440*t)\,0):s=48000:d=6",
                "-f",
                "lavfi",
                "-i",
                r"aevalsrc=if(between(t\,2.5\,2.6)\,0.5*sin(2*PI*880*t)\,0):s=48000:d=6",
                "-t",
                "6",
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-map",
                "2:a",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-threads",
                "1",
                "-c:a",
                "libopus",
                "-b:a",
                "128k",
                "-ac:a:0",
                "2",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        Some(Self { root, source })
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .arg("--root")
            .arg(&self.root)
            .args(["recording", "editor"])
            .args(args)
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("user"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("NIRI_SOCKET")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .output()
            .unwrap()
    }
    fn run(&self, args: &[&str]) -> Value {
        let out = self.command(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn real_thumbnails_scrubbing_and_packet_preserving_full_export() {
    let Some(f) = Fixture::new() else { return };
    let source = fs::read(&f.source).unwrap();
    let info = f.run(&["resume", ID]);
    assert_eq!(info["width"], 320);
    assert_eq!(info["height"], 180);
    assert_eq!(info["audio_tracks"], 2);
    let thumbs = info["thumbnails"].as_array().unwrap();
    assert_eq!(thumbs.len(), 10);
    let first = f.source.parent().unwrap().join("thumb-000000.jpg");
    assert_eq!(image::open(&first).unwrap().width(), 240);
    let last = f.source.parent().unwrap().join(
        thumbs.last().unwrap()["url"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap(),
    );
    assert_ne!(fs::read(first).unwrap(), fs::read(last).unwrap());
    let preview = f.run(&["preview", ID, "3.1"]);
    assert_eq!(preview["seconds"], 3.1);
    assert_eq!(
        image::open(f.source.parent().unwrap().join("preview-000186.jpg"))
            .unwrap()
            .width(),
        640
    );
    let end = info["duration"].to_string();
    let last_preview = f.run(&["preview", ID, &end]);
    assert!(last_preview["seconds"].as_f64().unwrap() < info["duration"].as_f64().unwrap());
    let saved = f.run(&["save", ID, "0", &end]);
    assert_eq!(saved["mode"], "copy");
    let output = PathBuf::from(saved["path"].as_str().unwrap());
    assert_eq!(fs::read(&output).unwrap(), source);
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read(&f.source).unwrap(), source);
    let second = f.run(&["save", ID, "0", &end]);
    assert_ne!(saved["path"], second["path"]);
    f.run(&["discard", ID]);
    assert!(!f.source.exists());
    assert!(output.is_file());
}
#[test]
fn invalid_ranges_ids_and_symlink_sources_never_publish_a_clip() {
    let Some(f) = Fixture::new() else { return };
    f.run(&["resume", ID]);
    for args in [
        ["save", ID, "-1", "4"],
        ["save", ID, "NaN", "4"],
        ["save", ID, "5", "4"],
        ["save", ID, "0", "20"],
    ] {
        assert!(!f.command(&args).status.success());
    }
    assert!(!f.command(&["discard", "../../outside"]).status.success());
    let original = f.root.join("outside.mkv");
    fs::rename(&f.source, &original).unwrap();
    std::os::unix::fs::symlink(&original, &f.source).unwrap();
    assert!(!f.command(&["resume", ID]).status.success());
    assert!(original.is_file());
    assert_eq!(fs::read_dir(f.root.join("videos")).unwrap().count(), 0);
}
fn onset(path: &str, track: u8) -> f64 {
    let out = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path,
            "-map",
            &format!("0:a:{track}"),
            "-ac",
            "1",
            "-ar",
            "48000",
            "-f",
            "f32le",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let samples: Vec<_> = out
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    samples
        .chunks(480)
        .position(|c| c.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / c.len() as f64 > 0.01)
        .expect("audible marker") as f64
        / 100.0
}
#[test]
fn nvenc_precise_cut_preserves_native_frames_channels_and_audio_sync() {
    if std::env::var("DF_TEST_NVENC").as_deref() != Ok("1") {
        eprintln!(
            "NVENC hardware acceptance: run with DF_TEST_NVENC=1 on the NVIDIA reference host"
        );
        return;
    }
    let f = Fixture::new().expect("FFmpeg is required for hardware acceptance");
    f.run(&["resume", ID]);
    let saved = f.run(&["save", ID, "1.25", "4.75"]);
    assert_eq!(saved["mode"], "nvenc");
    assert!((saved["duration"].as_f64().unwrap() - 3.5).abs() < 0.025);
    let path = saved["path"].as_str().unwrap();
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-show_streams",
            "-of",
            "json",
            path,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let info: Value = serde_json::from_slice(&out.stdout).unwrap();
    let streams = info["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 3);
    assert_eq!(streams[0]["width"], 320);
    assert_eq!(streams[0]["height"], 180);
    assert_eq!(streams[0]["nb_read_frames"], "210");
    assert_eq!(streams[1]["codec_name"], "opus");
    assert_eq!(streams[2]["codec_name"], "opus");
    assert_eq!(streams[1]["channels"], 2);
    assert_eq!(streams[2]["channels"], 1);
    assert!((onset(path, 0) - 0.75).abs() < 0.04);
    assert!((onset(path, 1) - 1.25).abs() < 0.04);
    assert!(f.source.is_file());
}

#[test]
fn production_editor_handles_real_thumbnails_and_range_input_offscreen() {
    if !desktop_foundationctl::apps::executable("quickshell")
        || !desktop_foundationctl::apps::executable("ffmpeg")
    {
        eprintln!("Production QML editor fixture requires Quickshell and FFmpeg");
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(root.join("tests/test-replay-editor"))
        .env(
            "DF_EDITOR_BINARY",
            env!("CARGO_BIN_EXE_desktop-foundationctl"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
