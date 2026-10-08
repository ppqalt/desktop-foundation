//! Existing 3% steps and native OSD, with bounded native commands.
use crate::{Result, invalid, process, state};
use std::{
    env,
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::Duration,
};

// The audio lock serializes both revision changes and feedback ownership changes.
// A separate atomically replaced runtime file cannot leave a truncated revision.
fn revision(path: &Path) -> Result<u64> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() || file.metadata()?.len() > 32 {
        return Err(invalid("Output volume revision is invalid"));
    }
    let mut value = String::new();
    file.take(32).read_to_string(&mut value)?;
    value
        .trim()
        .parse()
        .map_err(|_| invalid("Output volume revision is invalid"))
}

fn feedback_lock(runtime: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(runtime.join("desktop-foundation-volume-feedback.lock"))?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Output volume feedback lock is invalid"));
    }
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

pub fn parse(text: &str) -> Result<(u32, bool)> {
    let mut words = text.split_whitespace();
    if words.next() != Some("Volume:") {
        return Err(invalid("Unable to read output volume"));
    }
    let value: f64 = words
        .next()
        .ok_or_else(|| invalid("Output volume is missing"))?
        .parse()
        .map_err(|_| invalid("Output volume is invalid"))?;
    if !value.is_finite() || !(0.0..=10.0).contains(&value) {
        return Err(invalid("Output volume is outside the supported range"));
    }
    Ok((
        (value * 100.0).round_ties_even() as u32,
        text.contains("[MUTED]"),
    ))
}

pub fn adjust(root: &Path, direction: &str) -> Result<()> {
    let step = match direction {
        "up" => "3%+",
        "down" => "3%-",
        _ => return Err(invalid("Expected volume up or down")),
    };
    let runtime =
        env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| invalid("XDG_RUNTIME_DIR is unavailable"))?;
    let runtime = Path::new(&runtime);
    if !runtime.is_absolute() || !runtime.is_dir() {
        return Err(invalid(
            "XDG_RUNTIME_DIR is not an absolute runtime directory",
        ));
    }
    let audio_path = runtime.join("desktop-foundation-volume.lock");
    let revision_path = runtime.join("desktop-foundation-volume-revision");
    let feedback = feedback_lock(runtime)?;
    let mut audio = state::lock(&audio_path, Duration::from_secs(5))?;
    // Validate the state before changing the sink, then publish only after the
    // adjustment succeeds. Atomic replacement retains valid state on failure.
    let next_revision = revision(&revision_path)?
        .checked_add(1)
        .ok_or_else(|| invalid("Output volume revision is exhausted"))?;
    process::checked(
        &[
            "wpctl",
            "set-volume",
            "-l",
            "1",
            "@DEFAULT_AUDIO_SINK@",
            step,
        ]
        .map(String::from),
        None,
        Duration::from_secs(3),
        true,
    )?;
    state::atomic(&revision_path, next_revision.to_string().as_bytes(), 0o600)?;
    // Every repeat still applies its own atomic 3% step. A slow OSD cannot hold
    // the audio lock: repeat callers let the current owner display their update.
    let owns_feedback = match feedback.try_lock() {
        Ok(()) => true,
        Err(std::fs::TryLockError::WouldBlock) => false,
        Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
    };
    if !owns_feedback {
        return Ok(());
    }

    loop {
        let displayed_revision = revision(&revision_path)?;
        let current = read_volume();
        drop(audio);
        let delivery = current.and_then(|(percent, muted)| show_volume(root, percent, muted));

        audio = state::lock(&audio_path, Duration::from_secs(5))?;
        if revision(&revision_path)? == displayed_revision {
            // Release ownership while writes are excluded. Otherwise a repeat
            // could arrive after the check, see this owner, and lose its update.
            drop(feedback);
            return delivery;
        }
        // Only another successful adjustment causes a further read/delivery.
        // The one-shot owner exits as soon as the last revision is displayed.
    }
}

fn read_volume() -> Result<(u32, bool)> {
    let status = process::checked(
        &["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"].map(String::from),
        None,
        Duration::from_secs(3),
        true,
    )?;
    parse(std::str::from_utf8(&status.stdout).map_err(|_| invalid("Output volume is not UTF-8"))?)
}

fn show_volume(root: &Path, percent: u32, muted: bool) -> Result<()> {
    let delivered = process::checked(
        &[
            "quickshell".into(),
            "ipc".into(),
            "--path".into(),
            root.join("shell").to_string_lossy().into_owned(),
            "call".into(),
            "foundation".into(),
            "showVolume".into(),
            percent.to_string(),
            muted.to_string(),
        ],
        None,
        Duration::from_secs(2),
        true,
    )
    .is_ok();
    if !delivered {
        let summary = if muted {
            format!("Muted · {percent}%")
        } else {
            format!("Volume {percent}%")
        };
        process::checked(
            &[
                "notify-send".into(),
                "--app-name".into(),
                "desktop-foundation-volume".into(),
                "--hint".into(),
                "string:x-canonical-private-synchronous:desktop-foundation-volume".into(),
                "--expire-time".into(),
                "1500".into(),
                summary,
            ],
            None,
            Duration::from_secs(3),
            true,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_rounding_match_previous_osd() {
        assert_eq!(parse("Volume: 0.325 [MUTED]\n").unwrap(), (32, true));
        assert_eq!(parse("Volume: 1.00\n").unwrap(), (100, false));
        for text in [
            "Volume: NaN",
            "Volume: -1",
            "Volume: inf",
            "unexpected",
            "Volume: 11",
        ] {
            assert!(parse(text).is_err());
        }
    }
}
