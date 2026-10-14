//! A private btop profile, imported once and subsequently saved by btop itself.
use crate::{Result, invalid, state};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const MAXIMUM: u64 = 256 * 1024;
const OVERRIDES: [(&str, &str); 3] = [
    ("color_theme", "\"desktop-foundation\""),
    ("theme_background", "false"),
    ("save_config_on_exit", "true"),
];

fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or retained pointers.
    unsafe { libc::geteuid() }
}

fn metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn owned_file(metadata: &Metadata, path: &Path) -> Result<()> {
    if !metadata.is_file() || metadata.uid() != uid() {
        return Err(invalid(format!(
            "The monitor configuration must be an owned regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<()> {
    if let Some(metadata) = metadata(path)?
        && (!metadata.is_dir() || metadata.uid() != uid())
    {
        return Err(invalid(format!(
            "The monitor configuration directory must be owned: {}",
            path.display()
        )));
    }
    state::private_directory(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.uid() != uid() || metadata.mode() & 0o077 != 0 {
        return Err(invalid(
            "The monitor configuration directory is not private",
        ));
    }
    Ok(())
}

fn existing(path: &Path) -> Result<bool> {
    let Some(metadata) = metadata(path)? else {
        return Ok(false);
    };
    owned_file(&metadata, path)?;
    // A nontruncating open verifies access without changing btop's saved file.
    let file = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    owned_file(&metadata, path)?;
    if metadata.mode() & 0o200 == 0 {
        return Err(invalid(format!(
            "The monitor configuration must be writable: {}",
            path.display()
        )));
    }
    Ok(true)
}

fn source(path: &Path) -> Result<Option<String>> {
    // Check presence separately: an existing broken symlink or unreadable file
    // must not silently select a different set of user preferences.
    if metadata(path)?.is_none() {
        return Ok(None);
    }
    // User dotfile symlinks are legitimate import sources. The open descriptor
    // must still refer to a bounded regular file, never a pipe or device.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAXIMUM {
        return Err(invalid(format!(
            "The btop import must be a regular file at most 256 KiB: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAXIMUM + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAXIMUM {
        return Err(invalid("The btop import exceeds 256 KiB"));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| invalid("The btop import must be UTF-8"))
}

fn comment(value: &str) -> usize {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' && quoted {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if character == '#' && !quoted {
            return index;
        }
    }
    value.len()
}

fn imported(source: &str) -> String {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut seen = [false; OVERRIDES.len()];
    let mut output = String::with_capacity(source.len() + 128);
    for line in source.split_inclusive('\n') {
        let (body, ending) = if let Some(body) = line.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = line.strip_suffix('\n') {
            (body, "\n")
        } else {
            (line, "")
        };
        let Some((key, value)) = body.split_once('=') else {
            output.push_str(line);
            continue;
        };
        let Some(index) = OVERRIDES.iter().position(|(name, _)| key.trim() == *name) else {
            output.push_str(line);
            continue;
        };
        seen[index] = true;
        let before_comment = &value[..comment(value)];
        let start = before_comment.len() - before_comment.trim_start().len();
        let end = before_comment.trim_end().len().max(start);
        output.push_str(key);
        output.push('=');
        output.push_str(&value[..start]);
        output.push_str(OVERRIDES[index].1);
        if value[end..].starts_with('#') {
            output.push(' ');
        }
        output.push_str(&value[end..]);
        output.push_str(ending);
    }
    for (index, (key, value)) in OVERRIDES.iter().enumerate() {
        if !seen[index] {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push_str(newline);
            }
            output.push_str(key);
            output.push_str(" = ");
            output.push_str(value);
            output.push_str(newline);
        }
    }
    output
}

/// Initialize the isolated monitor profile without touching normal btop files.
pub fn btop(update_ms: u32) -> Result<PathBuf> {
    let config = state::xdg("XDG_CONFIG_HOME", ".config")?;
    let application = config.join("desktop-foundation");
    private_directory(&application)?;
    let directory = application.join("monitor");
    private_directory(&directory)?;
    let lock = directory.join("btop.lock");
    if let Some(metadata) = metadata(&lock)? {
        owned_file(&metadata, &lock)?;
    }
    let guard: File = state::lock(&lock, Duration::from_secs(6))?;
    owned_file(&guard.metadata()?, &lock)?;
    let destination = directory.join("btop.conf");
    if existing(&destination)? {
        return Ok(destination);
    }
    let ordinary = config.join("btop/btop.conf");
    let contents = match source(&ordinary)? {
        Some(contents) => contents,
        None => {
            let legacy = state::xdg("XDG_STATE_HOME", ".local/state")?
                .join("desktop-foundation/monitor/btop.conf");
            source(&legacy)?.unwrap_or_else(|| {
                format!(
                    "truecolor = true\nrounded_corners = true\nterminal_sync = true\nshown_boxes = \"cpu mem net proc\"\nupdate_ms = {update_ms}\n"
                )
            })
        }
    };
    state::atomic(&destination, imported(&contents).as_bytes(), 0o600)?;
    Ok(destination)
}
