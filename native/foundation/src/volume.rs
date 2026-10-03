//! Existing 3% steps and native OSD, with bounded native commands.
use crate::{Result, invalid, process, state};
use std::{env, path::Path, time::Duration};

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
    let _lock = state::lock(
        &runtime.join("desktop-foundation-volume.lock"),
        Duration::from_secs(5),
    )?;
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
    let status = process::checked(
        &["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"].map(String::from),
        None,
        Duration::from_secs(3),
        true,
    )?;
    let (percent, muted) = parse(
        std::str::from_utf8(&status.stdout).map_err(|_| invalid("Output volume is not UTF-8"))?,
    )?;
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
