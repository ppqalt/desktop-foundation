//! Bounded partial TOML overrides shared by desktop and appearance settings.
use crate::{Result, invalid, state};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
pub struct Effective {
    pub settings: Value,
    pub sources: Vec<PathBuf>,
}

fn read(path: &Path) -> Result<Value> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid(format!(
            "Configuration must be a regular file: {}",
            path.display()
        )));
    }
    let mut text = String::new();
    file.take(65537).read_to_string(&mut text)?;
    if text.len() > 65536 {
        return Err(invalid(format!(
            "Configuration exceeds 64 KiB: {}",
            path.display()
        )));
    }
    let value: toml::Value = toml::from_str(&text)
        .map_err(|e| invalid(format!("Invalid configuration {}: {e}", path.display())))?;
    Ok(serde_json::to_value(value)?)
}

fn merge(base: &mut Value, change: Value, key: &str) -> Result<()> {
    if let (Some(table), Some(values)) = (base.as_object_mut(), change.as_object()) {
        for (name, value) in values {
            let key = format!("{key}.{name}");
            let target = table
                .get_mut(name)
                .ok_or_else(|| invalid(format!("Unknown setting {key}")))?;
            merge(target, value.clone(), &key)?;
        }
    } else if (base.is_number() && change.is_number())
        || (base.is_boolean() && change.is_boolean())
        || (base.is_string() && change.is_string())
    {
        *base = change;
    } else {
        return Err(invalid(format!("Wrong type for setting {key}")));
    }
    Ok(())
}

pub fn load_at(root: &Path, name: &str, profile: Option<&str>, config: &Path) -> Result<Effective> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid configuration name"));
    }
    let base = root.join("config").join(format!("{name}.toml"));
    let mut result = Effective {
        settings: read(&base)?,
        sources: vec![base],
    };
    let mut paths = Vec::new();
    if let Some(profile) = profile {
        if profile.is_empty()
            || !profile
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(invalid("Invalid profile name"));
        }
        paths.push(
            root.join("profiles")
                .join(profile)
                .join(format!("{name}.toml")),
        );
    }
    let directory = config.join("desktop-foundation");
    match fs::read_dir(directory.join(format!("{name}.d"))) {
        Ok(entries) => {
            let mut parts = Vec::new();
            for entry in entries {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "toml") {
                    parts.push(path);
                }
            }
            parts.sort();
            paths.extend(parts);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    paths.push(directory.join(format!("{name}.toml")));
    for path in paths {
        let value = match read(&path) {
            Ok(value) => value,
            Err(crate::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        merge(&mut result.settings, value, name)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        result.sources.push(path);
    }
    Ok(result)
}

pub fn load(root: &Path, name: &str, profile: Option<&str>) -> Result<Effective> {
    load_at(
        root,
        name,
        profile,
        &state::xdg("XDG_CONFIG_HOME", ".config")?,
    )
}

pub fn desktop(root: &Path) -> Result<Effective> {
    let result = load(root, "desktop", None)?;
    let v = &result.settings;
    let player = v["media"]["home_player"]
        .as_str()
        .ok_or_else(|| invalid("Invalid Home player"))?;
    if player.len() > 128
        || !player
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(invalid(
            "Home player must be an MPRIS name fragment, or empty",
        ));
    }
    if !v["screen_corners"]["radius"]
        .as_u64()
        .is_some_and(|n| n <= 128)
        || !v["screen_corners"]["enabled"].is_boolean()
    {
        return Err(invalid(
            "Screen corner radius must be an integer from 0 to 128",
        ));
    }
    Ok(result)
}
