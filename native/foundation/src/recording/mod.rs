//! On-demand recording, discovered hardware, and an event-driven clock contract.
mod runtime;
use crate::{Result, apps, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

pub const UNIT: &str = "desktop-foundation-recording.service";
const SOCKET: &str = "desktop-foundation-recording.socket";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    shortcut: String,
    silent_shortcut: String,
    replay_shortcut: String,
    replay_toggle_shortcut: String,
    replay_seconds: u32,
    replay_bitrate_kbps: u32,
    capture: String,
    resolution: String,
    fps: u32,
    max_fps: u32,
    video_codec: String,
    quality: String,
    frame_mode: String,
    container: String,
    directory: String,
    cursor: bool,
    restore_portal: bool,
    desktop_audio: String,
    microphone_audio: String,
    audio_codec: String,
    audio_bitrate_kbps: u32,
    startup_timeout_seconds: u64,
    stop_timeout_seconds: u64,
    cancel_timeout_milliseconds: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            shortcut: "Ctrl+Shift+F12".into(),
            silent_shortcut: "Ctrl+Shift+Alt+F12".into(),
            replay_shortcut: "Ctrl+Shift+F11".into(),
            replay_toggle_shortcut: "Ctrl+Shift+Alt+F11".into(),
            replay_seconds: 135,
            replay_bitrate_kbps: 16000,
            capture: "portal".into(),
            resolution: "auto".into(),
            fps: 0,
            max_fps: 60,
            video_codec: "auto".into(),
            quality: "very_high".into(),
            frame_mode: "vfr".into(),
            container: "mkv".into(),
            directory: String::new(),
            cursor: true,
            restore_portal: true,
            desktop_audio: "default_output".into(),
            microphone_audio: "default_input".into(),
            audio_codec: "opus".into(),
            audio_bitrate_kbps: 512,
            startup_timeout_seconds: 120,
            stop_timeout_seconds: 30,
            cancel_timeout_milliseconds: 500,
        }
    }
}
impl Config {
    fn validate(&self) -> Result<()> {
        for shortcut in [
            &self.shortcut,
            &self.silent_shortcut,
            &self.replay_shortcut,
            &self.replay_toggle_shortcut,
        ] {
            if shortcut.len() > 128
                || !shortcut
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+_".contains(&b))
            {
                return Err(invalid(
                    "Recording shortcuts must use Niri key names and + modifiers, or be empty",
                ));
            }
        }
        let shortcuts = [
            &self.shortcut,
            &self.silent_shortcut,
            &self.replay_shortcut,
            &self.replay_toggle_shortcut,
        ];
        for (index, shortcut) in shortcuts.iter().enumerate() {
            if !shortcut.is_empty()
                && shortcuts[..index]
                    .iter()
                    .any(|other| other.eq_ignore_ascii_case(shortcut))
            {
                return Err(invalid("Recording shortcuts must be distinct"));
            }
        }
        if self.resolution != "auto" {
            return Err(invalid(
                "Recording always uses the selected source's native resolution",
            ));
        }
        if self.capture.is_empty()
            || self.capture.len() > 128
            || self.capture.starts_with('-')
            || self
                .capture
                .chars()
                .any(|c| c.is_control() || matches!(c, '|' | ';'))
            || (self.fps != 0 && !(60..=240).contains(&self.fps))
            || !(60..=240).contains(&self.max_fps)
            || !(2..=3600).contains(&self.replay_seconds)
            || !(1000..=100000).contains(&self.replay_bitrate_kbps)
            || !["auto", "h264", "hevc", "av1", "hevc_10bit", "av1_10bit"]
                .contains(&self.video_codec.as_str())
            || !["medium", "high", "very_high", "ultra"].contains(&self.quality.as_str())
            || !["cfr", "vfr", "content"].contains(&self.frame_mode.as_str())
            || self.container != "mkv"
            || self.audio_codec != "opus"
            || !(32..=512).contains(&self.audio_bitrate_kbps)
            || !(10..=600).contains(&self.startup_timeout_seconds)
            || !(5..=30).contains(&self.stop_timeout_seconds)
            || !(50..=2000).contains(&self.cancel_timeout_milliseconds)
            || (!self.directory.is_empty() && !Path::new(&self.directory).is_absolute())
        {
            return Err(invalid(
                "Invalid recording settings; see config/recording.toml",
            ));
        }
        for (source, default) in [
            (&self.desktop_audio, "default_output"),
            (&self.microphone_audio, "default_input"),
        ] {
            if source != default
                && !(source.starts_with("device:")
                    && source.len() > 7
                    && source.len() <= 512
                    && !source.chars().any(|c| c.is_control() || c == '|'))
            {
                return Err(invalid(
                    "Audio sources must use their default role or device:NAME; tracks stay separate",
                ));
            }
        }
        Ok(())
    }
}

fn bounded(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Recording settings/state must be a regular file"));
    }
    let mut data = Vec::new();
    file.take(65537).read_to_end(&mut data)?;
    if data.len() > 65536 {
        return Err(invalid("Recording settings/state exceeds 64 KiB"));
    }
    Ok(data)
}
fn overlay(config: &mut Config, path: &Path, sources: &mut Vec<PathBuf>) -> Result<()> {
    let bytes = match bounded(path) {
        Ok(v) => v,
        Err(crate::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let text = std::str::from_utf8(&bytes).map_err(|e| invalid(e.to_string()))?;
    let changes: toml::Value =
        toml::from_str(text).map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let mut merged = toml::Value::try_from(&*config).map_err(|e| invalid(e.to_string()))?;
    for (k, v) in changes
        .as_table()
        .ok_or_else(|| invalid("Recording settings must be a TOML table"))?
    {
        merged
            .as_table_mut()
            .expect("typed config table")
            .insert(k.clone(), v.clone());
    }
    *config = merged
        .try_into()
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    sources.push(path.to_owned());
    Ok(())
}
fn settings(root: &Path) -> Result<(Config, Vec<PathBuf>)> {
    let mut c = Config::default();
    let mut sources = Vec::new();
    overlay(&mut c, &root.join("config/recording.toml"), &mut sources)?;
    let user = state::xdg("XDG_CONFIG_HOME", ".config")?.join("desktop-foundation");
    match fs::read_dir(user.join("recording.d")) {
        Ok(entries) => {
            let mut paths = entries
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            paths.retain(|p| p.extension().is_some_and(|s| s == "toml"));
            paths.sort();
            for path in paths {
                overlay(&mut c, &path, &mut sources)?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    overlay(&mut c, &user.join("recording.toml"), &mut sources)?;
    c.validate()?;
    Ok((c, sources))
}
fn output() -> Option<Value> {
    let bytes = process::checked(
        &[
            "niri".into(),
            "msg".into(),
            "--json".into(),
            "focused-output".into(),
        ],
        None,
        Duration::from_secs(2),
        true,
    )
    .ok()?
    .stdout;
    serde_json::from_slice(&bytes).ok()
}
fn video_directory() -> Result<PathBuf> {
    let discovered = process::checked(
        &["xdg-user-dir".into(), "VIDEOS".into()],
        None,
        Duration::from_secs(1),
        true,
    )
    .ok()
    .and_then(|r| String::from_utf8(r.stdout).ok())
    .map(|s| PathBuf::from(s.trim()))
    .filter(|p| p.is_absolute());
    Ok(discovered
        .unwrap_or(state::home()?.join("Videos"))
        .join("Recordings"))
}
fn selected_fps(c: &Config, display: &Value) -> u32 {
    if c.fps != 0 {
        return c.fps;
    }
    let refresh = display["current_mode"]
        .as_u64()
        .and_then(|i| display["modes"].get(i as usize))
        .and_then(|mode| mode["refresh_rate"].as_f64())
        .filter(|hz| hz.is_finite() && *hz > 0.0)
        .map(|mhz| (mhz / 1000.0).round().clamp(1.0, 240.0) as u32)
        .unwrap_or(c.max_fps);
    refresh.clamp(60, c.max_fps)
}
fn audio_defaults(silent: bool) -> Value {
    let result = process::checked(
        &["pactl".into(), "--format=json".into(), "info".into()],
        None,
        Duration::from_secs(2),
        true,
    );
    match result.and_then(|v| serde_json::from_slice::<Value>(&v.stdout).map_err(Into::into)) {
        Ok(info) => {
            json!({"default_output":info["default_sink_name"], "default_input":if silent {Value::Null} else {info["default_source_name"].clone()},
            "server_sample_specification":info["default_sample_specification"]})
        }
        Err(e) => json!({"diagnostic":e.to_string()}),
    }
}
fn command(
    c: &Config,
    display: &Value,
    silent: bool,
    file: &Path,
    token: &Path,
) -> Result<Vec<String>> {
    let capture = if c.capture == "focused-output" {
        display["name"]
            .as_str()
            .ok_or_else(|| invalid("No focused output; use portal capture or set an output name"))?
    } else {
        &c.capture
    };
    let mut args: Vec<String> = [
        "gpu-screen-recorder",
        "-w",
        capture,
        "-encoder",
        "gpu",
        "-fallback-cpu-encoding",
        "no",
        "-q",
        &c.quality,
        "-fm",
        &c.frame_mode,
        "-c",
        &c.container,
        "-cursor",
        if c.cursor { "yes" } else { "no" },
        "-write-first-frame-ts",
        "yes",
        "-v",
        "no",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    args.extend(["-f".into(), selected_fps(c, display).to_string()]);
    if c.video_codec != "auto" {
        args.extend(["-k".into(), c.video_codec.clone()]);
    }
    if c.resolution != "auto" {
        args.extend(["-s".into(), c.resolution.clone()]);
    }
    if c.capture == "portal" {
        args.extend([
            "-restore-portal-session".into(),
            if c.restore_portal { "yes" } else { "no" }.into(),
            "-portal-session-token-filepath".into(),
            token.to_string_lossy().into_owned(),
        ]);
    }
    args.extend(["-a".into(), c.desktop_audio.clone()]);
    if !silent {
        args.extend(["-a".into(), c.microphone_audio.clone()]);
    }
    args.extend([
        "-ac".into(),
        c.audio_codec.clone(),
        "-ab".into(),
        c.audio_bitrate_kbps.to_string(),
    ]);
    args.extend(["-o".into(), file.to_string_lossy().into_owned()]);
    Ok(args)
}
pub fn plan(root: &Path, silent: bool) -> Result<Value> {
    let (c, sources) = settings(root)?;
    let display = output().unwrap_or(Value::Null);
    let directory = if c.directory.is_empty() {
        video_directory()?
    } else {
        PathBuf::from(&c.directory)
    };
    let token = state::xdg("XDG_STATE_HOME", ".local/state")?
        .join("desktop-foundation/recording/portal-token");
    let file = directory.join(format!("Foundation-PREVIEW.{}", c.container));
    let args = command(&c, &display, silent, &file, &token)?;
    Ok(
        json!({"available": apps::executable("gpu-screen-recorder"), "settings": c, "sources": sources,
        "focused_display": display, "fps": selected_fps(&c, &display), "directory": directory,
        "silent": silent, "tracks": if silent {1} else {2}, "command": args, "audio_defaults": audio_defaults(silent),
        "resolution_policy": "auto preserves the actual captured source resolution; a portal selection may differ from the focused output",
        "audio_policy": if silent {"desktop output only; microphone is not captured"} else {"separate default-output and default-input tracks; no device volume changes"}}),
    )
}
pub fn bindings(root: &Path) -> Result<String> {
    let (c, _) = settings(root)?;
    let mut text = String::from("binds {\n");
    for (key, action) in [
        (&c.shortcut, "toggle"),
        (&c.silent_shortcut, "silent"),
        (&c.replay_shortcut, "replay-save"),
        (&c.replay_toggle_shortcut, "replay-toggle"),
    ] {
        if !key.is_empty() {
            let command = format!("exec \"$DF_FOUNDATION_ROOT/scripts/record\" {action}");
            text.push_str(&format!(
                "    {} repeat=false {{ spawn-sh {}; }}\n",
                serde_json::to_string(key)?,
                serde_json::to_string(&command)?
            ));
        }
    }
    text.push_str("}\n");
    Ok(text)
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["toggle"] => runtime::request(false, false),
        ["silent"] => runtime::request(true, false),
        ["stop"] => runtime::request(false, true),
        ["replay-on"] => runtime::replay_set(true),
        ["replay-off"] => runtime::replay_set(false),
        ["replay-save"] => runtime::replay_request(true),
        ["replay-toggle"] => runtime::replay_request(false),
        ["serve"] => runtime::serve(root),
        ["cleanup"] => runtime::cleanup(),
        ["acknowledge"] => runtime::acknowledge(),
        ["plan"] => {
            println!("{}", plan(root, false)?);
            Ok(())
        }
        ["plan", "--silent"] => {
            println!("{}", plan(root, true)?);
            Ok(())
        }
        ["status"] => {
            let mut state = runtime::status()?;
            if state["buffer_seconds"].is_null()
                && let Ok((config, _)) = settings(root)
            {
                state["buffer_seconds"] = config.replay_seconds.into();
            }
            println!("{state}");
            Ok(())
        }
        _ => Err(invalid(
            "Expected recording toggle|silent|stop|replay-on|replay-off|replay-save|replay-toggle|plan [--silent]|status",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_resolution_and_separate_audio_without_cpu_fallback() {
        let c = Config::default();
        let display = json!({"name":"unknown-vendor-2", "current_mode":0, "modes":[{"width":2560,"height":1440,"refresh_rate":143982}]});
        let a = command(
            &c,
            &display,
            false,
            Path::new("/tmp/movie.mkv"),
            Path::new("/tmp/token"),
        )
        .unwrap();
        assert!(!a.contains(&"-s".into()));
        assert_eq!(selected_fps(&c, &display), 60);
        assert_eq!(a.iter().filter(|v| *v == "-a").count(), 2);
        assert!(a.windows(2).any(|v| v == ["-ab", "512"]));
        assert!(a.windows(2).any(|v| v == ["-fallback-cpu-encoding", "no"]));
        let silent = command(
            &c,
            &display,
            true,
            Path::new("/tmp/movie.mkv"),
            Path::new("/tmp/token"),
        )
        .unwrap();
        assert_eq!(silent.iter().filter(|v| *v == "-a").count(), 1);
        assert!(silent.windows(2).any(|v| v == ["-a", "default_output"]));
        assert!(!silent.contains(&"default_input".to_string()));
    }
    #[test]
    fn overrides_and_missing_displays_are_explicit() {
        let mut c = Config {
            capture: "focused-output".into(),
            ..Config::default()
        };
        assert!(
            command(
                &c,
                &Value::Null,
                true,
                Path::new("/tmp/a"),
                Path::new("/tmp/t")
            )
            .is_err()
        );
        c.capture = "portal".into();
        c.resolution = "1920x1080".into();
        assert!(c.validate().is_err());
        c.resolution = "auto".into();
        c.fps = 120;
        c.validate().unwrap();
        assert_eq!(selected_fps(&c, &Value::Null), 120);
        c.microphone_audio = "default_input|default_output".into();
        assert!(c.validate().is_err());
    }
}
