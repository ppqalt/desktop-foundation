//! On-demand native screenshots. Selectors have no deadline; captures do.
use crate::{Result, invalid, process, state};
use serde::Deserialize;
use serde_json::Value;
use std::{
    env,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_PNG: u64 = 128 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
struct Selector {
    background: String,
    border: String,
    selection: String,
    label_background: String,
    font: String,
    border_width: u32,
}

#[derive(Deserialize)]
struct Storage {
    directory: String,
    filename: String,
}

pub fn capture(root: &Path, backend: &str, action: &str) -> Result<()> {
    if !matches!(action, "region" | "window" | "output") {
        return Err(invalid(
            "Screenshot action must be region, window or output",
        ));
    }
    match backend {
        "niri" if action == "region" => niri_region(root),
        "niri" => {
            let native_action = if action == "window" {
                "screenshot-window"
            } else {
                "screenshot-screen"
            };
            process::checked(
                &strings(&[
                    "niri",
                    "msg",
                    "action",
                    native_action,
                    "--show-pointer",
                    "false",
                    "--write-to-disk",
                    "false",
                ]),
                None,
                Duration::from_secs(5),
                true,
            )?;
            Ok(())
        }
        "hyprland" => hyprland_capture(root, action),
        _ => Err(invalid(format!(
            "{backend} screenshot backend is not implemented"
        ))),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    toml::from_str(&fs::read_to_string(path)?)
        .map_err(|error| invalid(format!("Invalid screenshot configuration: {error}")))
}

fn runtime() -> Result<PathBuf> {
    let directory = env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| invalid("XDG_RUNTIME_DIR is unavailable"))?;
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(invalid(
            "XDG_RUNTIME_DIR must be an existing absolute directory",
        ));
    }
    Ok(directory)
}

/// Keep flock compatibility with the previous Python selector and exit quietly
/// on contention. The guard is held until capture and clipboard transfer finish.
fn lock(path: &Path) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    // SAFETY: flock borrows our live descriptor and stores no pointers.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        Ok(Some(file))
    } else {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error.into())
        }
    }
}

fn niri_region(root: &Path) -> Result<()> {
    let settings: Selector = config(&root.join("compositor/niri/screenshots.toml"))?;
    let directory = runtime()?;
    let Some(_lock) = lock(&directory.join("desktop-foundation-region.lock"))? else {
        return Ok(());
    };
    let command = vec![
        "slurp".into(),
        "-d".into(),
        "-b".into(),
        settings.background,
        "-c".into(),
        settings.border,
        "-s".into(),
        settings.selection,
        "-B".into(),
        settings.label_background,
        "-F".into(),
        settings.font,
        "-w".into(),
        settings.border_width.to_string(),
    ];
    // Empty stdin supplies no preselected boxes. Selection ends on drag release.
    let selected = process::interactive(&command, Some(Vec::new()))?;
    if !selected.status.success() {
        let diagnostic = String::from_utf8_lossy(&selected.stderr).trim().to_owned();
        if selected.status.code() == Some(1)
            && (diagnostic.is_empty()
                || matches!(
                    diagnostic.to_ascii_lowercase().as_str(),
                    "selection cancelled" | "selection canceled"
                ))
        {
            return Ok(());
        }
        return Err(invalid(format!(
            "Region selection failed: {}",
            if diagnostic.is_empty() {
                selected.status.to_string()
            } else {
                diagnostic
            }
        )));
    }
    let geometry = std::str::from_utf8(&selected.stdout)
        .map_err(|_| invalid("Region selection returned invalid geometry"))?
        .trim();
    if geometry.is_empty() {
        return Ok(());
    }
    if !valid_geometry(geometry) {
        return Err(invalid("Region selection returned invalid geometry"));
    }
    let image = process::checked_image(
        &strings(&["grim", "-g", geometry, "-"]),
        Duration::from_secs(20),
    )?
    .stdout;
    validate_png(&image)?;
    clipboard(image)
}

fn clipboard(image: Vec<u8>) -> Result<()> {
    // wl-copy forks its clipboard owner; inherited streams cannot hold the
    // calling QProcess open, and no screenshot helper stays resident.
    process::checked(
        &strings(&["wl-copy", "--type", "image/png"]),
        Some(image),
        Duration::from_secs(5),
        false,
    )?;
    Ok(())
}

fn valid_geometry(value: &str) -> bool {
    let Some((origin, dimensions)) = value.split_once(' ') else {
        return false;
    };
    let Some((x, y)) = origin.split_once(',') else {
        return false;
    };
    let Some((width, height)) = dimensions.split_once('x') else {
        return false;
    };
    fn digits(value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
    }
    fn signed(value: &str) -> bool {
        digits(value.strip_prefix('-').unwrap_or(value))
    }
    fn positive(value: &str) -> bool {
        matches!(value.as_bytes().first(), Some(b'1'..=b'9')) && digits(value)
    }
    signed(x) && signed(y) && positive(width) && positive(height)
}

fn validate_png(image: &[u8]) -> Result<()> {
    if image.len() < 24 || &image[..8] != b"\x89PNG\r\n\x1a\n" || &image[12..16] != b"IHDR" {
        return Err(invalid("Capture did not produce a valid PNG"));
    }
    Ok(())
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct CaptureDirectory(PathBuf);
impl CaptureDirectory {
    fn create(runtime: &Path) -> Result<Self> {
        loop {
            let path = runtime.join(format!(
                "foundation-capture-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
impl Drop for CaptureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn hyprland_capture(root: &Path, action: &str) -> Result<()> {
    let settings: Storage = config(&root.join("config/screenshots.toml"))?;
    let directory = runtime()?;
    let Some(_lock) = lock(&directory.join("desktop-foundation-screenshot.lock"))? else {
        return Ok(());
    };
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none_or(|value| value.is_empty()) {
        return Err(invalid("Hyprland session unavailable"));
    }
    let temporary = CaptureDirectory::create(&directory)?;
    let capture = temporary.0.join("capture.png");
    let mut command = strings(&["grim", "-t", "png"]);
    match action {
        "region" => {
            let selected = process::interactive(
                &strings(&[
                    "slurp",
                    "-b",
                    "#080b1080",
                    "-c",
                    "#b8ceeebb",
                    "-s",
                    "#b8ceee18",
                    "-w",
                    "1",
                ]),
                None,
            )?;
            if !selected.status.success() {
                let diagnostic = String::from_utf8_lossy(&selected.stderr).trim().to_owned();
                if diagnostic == "selection cancelled" {
                    return Ok(());
                }
                return Err(invalid(if diagnostic.is_empty() {
                    "Region selection failed".into()
                } else {
                    diagnostic
                }));
            }
            let geometry = std::str::from_utf8(&selected.stdout)
                .map_err(|_| invalid("No valid rectangular region selected"))?
                .trim();
            if !valid_geometry(geometry) {
                return Err(invalid("No valid rectangular region selected"));
            }
            command.extend(strings(&["-g", geometry]));
        }
        "output" => {
            let monitors = snapshot("monitors")?;
            let monitor = monitors
                .as_array()
                .ok_or_else(|| invalid("Invalid monitor snapshot"))?
                .iter()
                .find(|monitor| flag(monitor, "focused") && !flag(monitor, "disabled"))
                .ok_or_else(|| invalid("No focused output available"))?;
            let name = monitor["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| invalid("Focused output name is unavailable"))?;
            command.extend(strings(&["-o", name]));
        }
        "window" => command.extend(strings(&["-g", &window_geometry()?])),
        _ => unreachable!("capture validates the action"),
    }
    command.push(
        capture
            .to_str()
            .ok_or_else(|| invalid("Screenshot runtime path must be UTF-8"))?
            .to_owned(),
    );
    process::checked(&command, None, Duration::from_secs(20), true)?;
    let mut image = Vec::new();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&capture)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Capture must be a regular PNG file"));
    }
    file.take(MAX_PNG + 1).read_to_end(&mut image)?;
    if image.len() as u64 > MAX_PNG {
        return Err(invalid("Capture exceeded 128 MiB"));
    }
    validate_png(&image)?;
    let destination = save_image(&image, &settings)?;
    if clipboard(image).is_err() {
        return Err(invalid(format!(
            "Image saved at {}, but copying to the clipboard failed",
            destination.display()
        )));
    }
    println!("{}", destination.display());
    Ok(())
}

fn flag(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap_or(false)
}

fn snapshot(resource: &str) -> Result<Value> {
    let output = process::checked(
        &strings(&["hyprctl", "-j", resource]),
        None,
        Duration::from_secs(3),
        true,
    )?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn number(value: &Value) -> Result<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid("Invalid focused window/output geometry"))
}

fn coordinate(value: f64) -> Result<i64> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= i64::MAX as f64 {
        return Err(invalid("Invalid focused window/output geometry"));
    }
    Ok(value as i64)
}

fn window_geometry() -> Result<String> {
    let window = snapshot("activewindow")?;
    if window["address"].as_str().is_none_or(str::is_empty)
        || !flag(&window, "mapped")
        || flag(&window, "hidden")
    {
        return Err(invalid("No focused window available"));
    }
    let monitors = snapshot("monitors")?;
    let monitor = monitors
        .as_array()
        .ok_or_else(|| invalid("Invalid monitor snapshot"))?
        .iter()
        .find(|monitor| !monitor["id"].is_null() && monitor["id"] == window["monitor"])
        .ok_or_else(|| invalid("Focused window output is unavailable"))?;
    let (mut width, mut height) = (number(&monitor["width"])?, number(&monitor["height"])?);
    if monitor["transform"].as_i64().unwrap_or(0) % 2 != 0 {
        std::mem::swap(&mut width, &mut height);
    }
    let scale = number(&monitor["scale"])?;
    if scale <= 0.0 || width <= 0.0 || height <= 0.0 {
        return Err(invalid("Invalid focused window/output geometry"));
    }
    let (mx, my) = (number(&monitor["x"])?, number(&monitor["y"])?);
    let (wx, wy) = (number(&window["at"][0])?, number(&window["at"][1])?);
    let (ww, wh) = (number(&window["size"][0])?, number(&window["size"][1])?);
    // This crops visible screen pixels, not a hidden window buffer. Clip to the
    // owning transformed/scaled output just as the previous backend did.
    let x = coordinate(wx.max(mx).ceil())?;
    let y = coordinate(wy.max(my).ceil())?;
    let right = coordinate((wx + ww).min(mx + width / scale).floor())?;
    let bottom = coordinate((wy + wh).min(my + height / scale).floor())?;
    if right <= x || bottom <= y {
        return Err(invalid("Focused window has no visible area"));
    }
    let width = right
        .checked_sub(x)
        .ok_or_else(|| invalid("Invalid screenshot width"))?;
    let height = bottom
        .checked_sub(y)
        .ok_or_else(|| invalid("Invalid screenshot height"))?;
    Ok(format!("{x},{y} {width}x{height}"))
}

fn save_image(image: &[u8], settings: &Storage) -> Result<PathBuf> {
    let directory = if settings.directory == "~" {
        state::home()?
    } else if let Some(relative) = settings.directory.strip_prefix("~/") {
        state::home()?.join(relative)
    } else {
        PathBuf::from(&settings.directory)
    };
    if !directory.is_absolute() {
        return Err(invalid(
            "Screenshot directory must be absolute or start with ~/",
        ));
    }
    let filename = filename(&settings.filename)?;
    let path = Path::new(&filename);
    if path.file_name().and_then(|name| name.to_str()) != Some(filename.as_str())
        || !filename.ends_with(".png")
    {
        return Err(invalid("Screenshot filename must be a PNG basename"));
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    let destination = directory.join(filename);
    let (mut stream, temporary) = loop {
        let candidate = directory.join(format!(
            ".capture-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&candidate)
        {
            Ok(stream) => break (stream, TemporaryFile(candidate)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };
    stream.write_all(image)?;
    stream.sync_all()?;
    // Atomic no-overwrite publication: a filename collision cannot replace an
    // earlier image, and incomplete captures never appear as final screenshots.
    fs::hard_link(&temporary.0, &destination)?;
    File::open(&directory)?.sync_all()?;
    Ok(destination)
}

fn filename(format: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Screenshot clock predates the Unix epoch"))?;
    let seconds = libc::time_t::try_from(now.as_secs())
        .map_err(|_| invalid("Screenshot clock is out of range"))?;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers reference properly sized live objects, and the
    // successful localtime_r call initializes every field of the output.
    let local = unsafe {
        if libc::localtime_r(&seconds, local.as_mut_ptr()).is_null() {
            return Err(invalid("Unable to format screenshot local time"));
        }
        local.assume_init()
    };
    let micros = format!("{:06}", now.subsec_micros());
    let mut expanded = String::new();
    let mut characters = format.chars();
    while let Some(character) = characters.next() {
        expanded.push(character);
        if character == '%' {
            match characters.next() {
                Some('f') => {
                    expanded.pop();
                    expanded.push_str(&micros);
                }
                Some(character) => expanded.push(character),
                None => {}
            }
        }
    }
    let format =
        CString::new(expanded).map_err(|_| invalid("Screenshot filename format contains NUL"))?;
    for capacity in [256, 1024, 8192] {
        let mut bytes = vec![0_u8; capacity];
        // SAFETY: output points to capacity writable bytes, format is a C
        // string, and local is a fully initialized tm returned by libc above.
        let count =
            unsafe { libc::strftime(bytes.as_mut_ptr().cast(), capacity, format.as_ptr(), &local) };
        if count > 0 {
            bytes.truncate(count);
            return String::from_utf8(bytes)
                .map_err(|_| invalid("Screenshot filename is not UTF-8"));
        }
    }
    Err(invalid("Screenshot filename is empty or too long"))
}
