//! Layered keyboard settings with profile and user overrides.
use crate::{Result, invalid, state};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub layout: String,
    #[serde(alias = "repeatDelay")]
    pub repeat_delay: u32,
    #[serde(alias = "repeatRate")]
    pub repeat_rate: u32,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Override {
    layout: Option<String>,
    repeat_delay: Option<u32>,
    repeat_rate: Option<u32>,
}
#[derive(Serialize)]
pub struct EffectiveInput {
    pub settings: Input,
    pub sources: Vec<PathBuf>,
}
fn overlay(path: &Path, result: &mut EffectiveInput) -> Result<()> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut text = String::new();
    file.take(65537).read_to_string(&mut text)?;
    if text.len() > 65536 {
        return Err(invalid(format!(
            "Input settings too large: {}",
            path.display()
        )));
    }
    let change: Override = toml::from_str(&text).map_err(|error| {
        invalid(format!(
            "Invalid input settings {}: {error}",
            path.display()
        ))
    })?;
    if let Some(value) = change.layout {
        result.settings.layout = value;
    }
    if let Some(value) = change.repeat_delay {
        result.settings.repeat_delay = value;
    }
    if let Some(value) = change.repeat_rate {
        result.settings.repeat_rate = value;
    }
    result.sources.push(path.to_owned());
    Ok(())
}
pub fn load(
    base: Value,
    base_path: &Path,
    profile: &Path,
    config: &Path,
) -> Result<EffectiveInput> {
    let relevant = serde_json::json!({"layout":base["layout"],"repeatDelay":base["repeatDelay"],"repeatRate":base["repeatRate"]});
    let settings: Input = serde_json::from_value(relevant)
        .map_err(|error| invalid(format!("Invalid base input settings: {error}")))?;
    let mut result = EffectiveInput {
        settings,
        sources: vec![base_path.to_owned()],
    };
    overlay(&profile.join("input.toml"), &mut result)?;
    let directory = config.join("desktop-foundation/input.d");
    match fs::read_dir(&directory) {
        Ok(entries) => {
            let mut paths = Vec::new();
            for entry in entries {
                let path = entry?.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
                {
                    paths.push(path);
                }
            }
            paths.sort();
            for path in paths {
                overlay(&path, &mut result)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    overlay(&config.join("desktop-foundation/input.toml"), &mut result)?;
    if result.settings.layout.trim().is_empty()
        || result.settings.layout.chars().any(char::is_control)
    {
        return Err(invalid(
            "Keyboard layout must be a nonempty string without control characters",
        ));
    }
    Ok(result)
}
pub fn user_config() -> Result<PathBuf> {
    state::xdg("XDG_CONFIG_HOME", ".config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn layers_partial_settings_and_reports_sources() {
        let dir = std::env::temp_dir().join(format!("foundation-input-{}", std::process::id()));
        fs::create_dir_all(dir.join("profile")).unwrap();
        fs::create_dir_all(dir.join("config/desktop-foundation/input.d")).unwrap();
        fs::write(dir.join("profile/input.toml"), "layout = 'us'\n").unwrap();
        fs::write(
            dir.join("config/desktop-foundation/input.d/10.toml"),
            "repeat_rate = 40\n",
        )
        .unwrap();
        fs::write(
            dir.join("config/desktop-foundation/input.toml"),
            "repeat_delay = 280\n",
        )
        .unwrap();
        let base = json!({"layout":"fi","repeatRate":35,"repeatDelay":350});
        let result = load(
            base.clone(),
            Path::new("base"),
            &dir.join("profile"),
            &dir.join("config"),
        )
        .unwrap();
        assert_eq!(
            result.settings,
            Input {
                layout: "us".into(),
                repeat_rate: 40,
                repeat_delay: 280
            }
        );
        assert_eq!(result.sources.len(), 4);
        fs::write(
            dir.join("config/desktop-foundation/input.toml"),
            "unexpected = 1\n",
        )
        .unwrap();
        assert!(
            load(
                base,
                Path::new("base"),
                &dir.join("profile"),
                &dir.join("config")
            )
            .is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
