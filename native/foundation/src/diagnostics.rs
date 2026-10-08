//! Bounded, read-only build and deployment metadata. No application data probes.
#[path = "source_identity.rs"]
mod source_identity;

use crate::{Result, invalid, state};
use serde_json::{Value, json};
use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub const FORMAT_VERSION: u32 = 1;

pub fn build_info() -> Value {
    json!({
        "formatVersion": FORMAT_VERSION,
        "package": env!("CARGO_PKG_NAME"),
        "packageVersion": env!("CARGO_PKG_VERSION"),
        "revision": env!("FOUNDATION_BUILD_REVISION"),
        "dirty": match env!("FOUNDATION_BUILD_DIRTY") { "true" => Some(true), "false" => Some(false), _ => None },
        "dirtyScope": "native/foundation",
        "sourceDigest": env!("FOUNDATION_SOURCE_DIGEST"),
        "sourceDigestVersion": 1,
        "commandProtocolVersion": 1,
    })
}

fn metadata(path: &Path, keys: &[&str]) -> Value {
    let read = || -> Result<Value> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > 64 * 1024 {
            return Err(invalid(
                "Foundation metadata must be a regular file within 64 KiB",
            ));
        }
        let mut contents = Vec::new();
        Read::by_ref(&mut file)
            .take(64 * 1024 + 1)
            .read_to_end(&mut contents)?;
        if contents.len() > 64 * 1024 {
            return Err(invalid("Foundation metadata exceeds 64 KiB"));
        }
        let value: Value = serde_json::from_slice(&contents)?;
        if !value.is_object() {
            return Err(invalid("Foundation metadata must be an object"));
        }
        let mut selected = serde_json::Map::new();
        for key in keys {
            if let Some(value) = value.get(key) {
                selected.insert((*key).to_owned(), value.clone());
            }
        }
        Ok(Value::Object(selected))
    };
    match read() {
        Ok(value) => json!({"available": true, "metadata": value}),
        Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            json!({"available": false})
        }
        Err(error) => json!({"available": false, "error": error.to_string()}),
    }
}

pub fn report(root: &Path) -> Result<Value> {
    let source = match source_identity::digest(&root.join("native/foundation")) {
        Ok(digest) => {
            json!({"available": true, "matchesBinary": digest == env!("FOUNDATION_SOURCE_DIGEST"), "digest": digest})
        }
        Err(error) => json!({"available": false, "error": error.to_string()}),
    };
    let state = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation");
    let compositor = if std::env::var_os("NIRI_SOCKET").is_some() {
        "niri"
    } else if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        "hyprland"
    } else {
        "unknown"
    };
    Ok(json!({
        "formatVersion": FORMAT_VERSION,
        "build": build_info(),
        "sources": source,
        "session": {"compositor": compositor},
        "deployment": metadata(&state.join("manifest.json"), &["root"]),
        "applications": metadata(&state.join("application-roles.json"), &["profile"]),
        "theme": {
            "revision": std::fs::read_link(state.join("theme/current")).ok().and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned())),
            "metadata": metadata(&state.join("theme/current/metadata.json"), &["profile", "wallpaperHash", "reset"]),
            "palette": metadata(&state.join("theme/current/semantic.json"), &["policy"]),
        },
    }))
}
