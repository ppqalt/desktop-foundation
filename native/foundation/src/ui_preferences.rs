//! Read-only, on-demand desktop accessibility preferences. No watcher/daemon.
use serde_json::{Value, json};
use std::time::Duration;

pub fn reduced_motion(value: &str) -> Option<bool> {
    match value.trim() {
        "false" => Some(true),
        "true" => Some(false),
        _ => None,
    }
}

pub fn read() -> Value {
    let reduced = std::env::var("DF_REDUCED_MOTION")
        .ok()
        .and_then(|v| match v.as_str() {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        })
        .or_else(|| {
            let output = crate::process::run(
                &[
                    "gsettings".into(),
                    "get".into(),
                    "org.gnome.desktop.interface".into(),
                    "enable-animations".into(),
                ],
                None,
                Duration::from_secs(2),
                true,
            )
            .ok()?;
            output.status.success().then_some(())?;
            reduced_motion(std::str::from_utf8(&output.stdout).ok()?)
        });
    json!({"reducedMotion": reduced})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animation_preference_is_explicit() {
        assert_eq!(reduced_motion("false\n"), Some(true));
        assert_eq!(reduced_motion("true\n"), Some(false));
        assert_eq!(reduced_motion(""), None);
        assert_eq!(reduced_motion("error"), None);
    }
}
