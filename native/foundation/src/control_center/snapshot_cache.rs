//! Last observed display data, never desired settings or permission to mutate.
use crate::{Result, invalid, state};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const LIMIT: u64 = 1024 * 1024;

fn path() -> Result<PathBuf> {
    Ok(state::xdg("XDG_CACHE_HOME", ".cache")?
        .join("desktop-foundation/control-center/snapshot-v1.json"))
}
fn text(value: &Value) -> bool {
    value.as_str().is_some_and(|s| s.len() <= 4096)
}
fn list(value: &Value, valid: impl Fn(&Value) -> bool) -> bool {
    value
        .as_array()
        .is_some_and(|rows| rows.len() <= 256 && rows.iter().all(valid))
}
fn endpoint(value: &Value) -> bool {
    text(&value["name"])
        && value["volume"]
            .as_f64()
            .is_some_and(|n| (0.0..=100.0).contains(&n))
        && value["muted"].is_boolean()
        && ["description", "label"]
            .iter()
            .all(|k| value.get(k).is_none_or(text))
}
fn audio(value: &Value) -> bool {
    value["available"] == true
        && list(&value["outputs"], endpoint)
        && list(&value["inputs"], endpoint)
        && list(&value["applications"], |v| {
            endpoint(v)
                && v["id"].as_u64().is_some()
                && v["serial"].as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_digit())
                })
        })
        && ["output", "input"]
            .iter()
            .all(|k| value[k].is_null() || text(&value[k]))
}
fn network(value: &Value) -> bool {
    value["available"] == true
        && value["enabled"].is_boolean()
        && value["wifi"].is_boolean()
        && list(&value["devices"], |v| {
            ["name", "kind", "state", "connection"]
                .iter()
                .all(|k| text(&v[k]))
        })
        && list(&value["connections"], |v| {
            text(&v["name"])
                && text(&v["kind"])
                && v["active"].is_boolean()
                && v["uuid"].as_str().is_some_and(super::uuid)
        })
}
fn valid(value: &Value) -> bool {
    value.as_object().is_some_and(|v| {
        !v.is_empty() && v.keys().all(|k| matches!(k.as_str(), "audio" | "network"))
    }) && value.get("audio").is_none_or(audio)
        && value.get("network").is_none_or(network)
}
fn read_file(path: &Path) -> Result<Option<Value>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata()?;
    // SAFETY: geteuid has no arguments and does not retain pointers.
    let uid = unsafe { libc::geteuid() };
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.uid() != uid
        || meta.mode() & 0o077 != 0
        || meta.len() > LIMIT
    {
        return Err(invalid(
            "Control Center cache is not a bounded private file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("Control Center cache is too large"));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if value["version"] != 1 || !valid(&value["snapshot"]) {
        return Err(invalid(
            "Control Center cache has an unsupported or invalid format",
        ));
    }
    Ok(Some(value["snapshot"].clone()))
}
pub fn read() -> Result<Value> {
    // Missing/corrupt caches cannot prevent a normal fresh service query.
    let snapshot = read_file(&path()?).unwrap_or(None);
    Ok(json!({"snapshot":snapshot}))
}
pub fn write() -> Result<Value> {
    let mut bytes = Vec::new();
    std::io::stdin().take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("Control Center snapshot is too large"));
    }
    let snapshot: Value = serde_json::from_slice(&bytes)?;
    if !valid(&snapshot) {
        return Err(invalid("Invalid confirmed Control Center snapshot"));
    }
    let file = path()?;
    let directory = file.parent().unwrap();
    state::private_directory(directory.parent().unwrap())?;
    state::private_directory(directory)?;
    let _lock = state::lock(&directory.join("snapshot.lock"), Duration::from_secs(3))?;
    // Preserve foreign, corrupt and future-format files rather than overwriting.
    let previous = read_file(&file)?;
    let mut merged = previous.clone().unwrap_or_else(|| json!({}));
    for (key, value) in snapshot.as_object().unwrap() {
        merged[key] = value.clone();
    }
    if previous == Some(merged.clone()) {
        return Ok(json!({"success":true}));
    }
    let encoded = serde_json::to_vec(&json!({"version":1,"snapshot":merged}))?;
    if encoded.len() as u64 > LIMIT {
        return Err(invalid("Combined Control Center snapshot is too large"));
    }
    state::atomic(&file, &encoded, 0o600)?;
    Ok(json!({"success":true}))
}
