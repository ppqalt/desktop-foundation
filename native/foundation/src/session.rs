//! Finite session startup decisions, followed by native process ownership.
use crate::{Result, invalid, process, state};
use serde::Deserialize;
use std::{
    env,
    ffi::{CString, OsStr},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_CONFIG: u64 = 64 * 1024;

#[derive(Deserialize)]
struct WallpaperConfig {
    image: String,
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String {
    "fill".into()
}

fn require_niri(action: &str) -> Result<()> {
    if env::var_os("NIRI_SOCKET").is_none_or(|value| value.is_empty()) {
        return Err(invalid(format!("{action} startup requires a Niri session")));
    }
    Ok(())
}

fn text_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("Path is not UTF-8: {}", path.display())))
}

fn parse_config(contents: &str) -> Result<WallpaperConfig> {
    let config: WallpaperConfig = toml::from_str(contents)
        .map_err(|error| invalid(format!("Invalid wallpaper configuration: {error}")))?;
    if config.image.is_empty() || config.image.contains('\0') {
        return Err(invalid("Wallpaper image must be a nonempty path"));
    }
    if !matches!(config.mode.as_str(), "fill" | "fit" | "center" | "tile") {
        return Err(invalid(
            "Choose an aspect-preserving wallpaper mode: fill, fit, center or tile",
        ));
    }
    Ok(config)
}

fn read_config(path: &Path) -> Result<WallpaperConfig> {
    // current is an intentional symlink to an immutable runtime bundle. Follow
    // it while refusing non-regular targets; nonblocking open avoids a FIFO wait.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Wallpaper configuration must be a regular file"));
    }
    if file.metadata()?.len() > MAX_CONFIG {
        return Err(invalid("Wallpaper configuration exceeds 64 KiB"));
    }
    let mut contents = String::new();
    Read::by_ref(&mut file)
        .take(MAX_CONFIG + 1)
        .read_to_string(&mut contents)?;
    if contents.len() as u64 > MAX_CONFIG {
        return Err(invalid("Wallpaper configuration exceeds 64 KiB"));
    }
    parse_config(&contents)
}

fn named_home(name: &str) -> Result<PathBuf> {
    let name = CString::new(name).map_err(|_| invalid("Invalid home-directory name"))?;
    let mut size = 16 * 1024;
    loop {
        let mut buffer = vec![0_u8; size];
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: the input string is terminated, the buffer is writable, and
        // all returned pointers are copied before the backing buffer is dropped.
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && size < 1024 * 1024 {
            size *= 2;
            continue;
        }
        if status != 0 || found.is_null() {
            return Err(invalid("Named user's home directory is unavailable"));
        }
        // SAFETY: getpwnam_r succeeded and returned a live passwd record whose
        // pw_dir points to a terminated string in buffer for this scope.
        let directory = unsafe {
            let entry = entry.assume_init();
            if entry.pw_dir.is_null() {
                return Err(invalid("Named user's home directory is unavailable"));
            }
            std::ffi::CStr::from_ptr(entry.pw_dir).to_bytes()
        };
        if directory.is_empty() {
            return Err(invalid("Named user's home directory is unavailable"));
        }
        return Ok(PathBuf::from(OsStr::from_bytes(directory)));
    }
}

fn expand_user(image: &str) -> Result<PathBuf> {
    if let Some(remainder) = image.strip_prefix('~') {
        let (name, suffix) = remainder.split_once('/').unwrap_or((remainder, ""));
        let home = if name.is_empty() {
            state::home()?
        } else {
            named_home(name)?
        };
        return Ok(home.join(suffix));
    }
    Ok(PathBuf::from(image))
}

fn image_path(root: &Path, config: &WallpaperConfig, runtime: bool) -> Result<PathBuf> {
    let mut image = expand_user(&config.image)?;
    if !image.is_file() && !runtime {
        let name = image
            .file_name()
            .ok_or_else(|| invalid("Wallpaper image must name a file"))?;
        image = root.join("wallpapers").join(name);
    }
    if !image.is_file() {
        return Err(invalid(format!(
            "Wallpaper image unavailable: {}",
            image.display()
        )));
    }
    Ok(image)
}

fn executable(path: &Path) -> bool {
    if !fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: the pathname is terminated and access does not retain its pointer.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

fn swaybg(state: &Path) -> Result<PathBuf> {
    let paths = env::var_os("PATH").unwrap_or_else(|| "/bin:/usr/bin".into());
    if let Some(path) = env::split_paths(&paths)
        .map(|directory| directory.join("swaybg"))
        .find(|path| executable(path))
    {
        return Ok(path);
    }
    let fallback = state.join("bin/swaybg");
    if executable(&fallback) {
        return Ok(fallback);
    }
    Err(invalid(
        "swaybg unavailable: install it with scripts/bootstrap",
    ))
}

/// Start the one session-owned swaybg process. Rust exits through exec; it does
/// not stay resident, poll the wallpaper, or regenerate an existing bundle.
pub fn wallpaper(root: &Path) -> Result<()> {
    require_niri("Wallpaper")?;
    let state = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation");
    let current = state.join("theme/current");
    let active = current.join("wallpaper.toml");
    let runtime = active.is_file();
    let path = if runtime {
        active
    } else {
        root.join("compositor/niri/wallpaper.toml")
    };
    let config = read_config(&path)?;
    let image = image_path(root, &config, runtime)?;
    let executable = swaybg(&state)?;
    if !current.exists() {
        // Legacy installations prepare a backdrop once. Normal deployed runtime
        // bundles already contain it, so their startup does not invoke Python.
        process::checked(
            &[
                "python3".into(),
                text_path(&root.join("scripts/niri_wallpaper.py"))?,
                "--prepare-backdrop".into(),
                text_path(&image)?,
                config.mode.clone(),
            ],
            None,
            Duration::from_secs(30),
            true,
        )?;
    }
    process::replace(&[
        text_path(&executable)?,
        "--image".into(),
        text_path(&image)?,
        "--mode".into(),
        config.mode,
    ])
}

/// Start the deployed notification unit without replacing an existing provider.
pub fn notifications_start() -> Result<()> {
    require_niri("Notification")?;
    process::replace(&[
        "systemctl".into(),
        "--user".into(),
        "start".into(),
        "desktop-foundation-notifications.service".into(),
    ])
}

/// Fail closed unless the session bus explicitly reports no notification owner.
/// This probe may also be used by a user service outside a Niri launch command.
pub fn notifications_available() -> Result<()> {
    let result = process::checked(
        &[
            "busctl".into(),
            "--user".into(),
            "call".into(),
            "org.freedesktop.DBus".into(),
            "/org/freedesktop/DBus".into(),
            "org.freedesktop.DBus".into(),
            "NameHasOwner".into(),
            "s".into(),
            "org.freedesktop.Notifications".into(),
        ],
        None,
        Duration::from_secs(2),
        true,
    )?;
    let answer = std::str::from_utf8(&result.stdout)
        .map_err(|_| invalid("Invalid notification owner response"))?;
    match answer.trim() {
        "b false" => Ok(()),
        "b true" => Err(invalid(
            "A notification provider already owns the session bus",
        )),
        _ => Err(invalid("Invalid notification owner response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_config_defaults_and_preserves_paths() {
        let config = parse_config("image = '/a wallpaper/image with spaces.png'\n").unwrap();
        assert_eq!(config.image, "/a wallpaper/image with spaces.png");
        assert_eq!(config.mode, "fill");
        for mode in ["fill", "fit", "center", "tile"] {
            assert_eq!(
                parse_config(&format!("image = '/image.png'\nmode = '{mode}'\n"))
                    .unwrap()
                    .mode,
                mode
            );
        }
    }

    #[test]
    fn wallpaper_config_rejects_unsafe_or_invalid_shapes() {
        for input in [
            "image = ''",
            "image = 10",
            "mode = 'fill'",
            "image = '/image.png'\nmode = 'stretch'",
            "image = '/image.png'\nmode = 10",
            "image = \"nul\\u0000path\"",
        ] {
            assert!(parse_config(input).is_err(), "{input}");
        }
    }
}
