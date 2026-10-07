//! Reversible native desktop preferences. No session bus or daemon is retained.
use crate::{Result, invalid, process, state, theme};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Read,
    os::{
        fd::{FromRawFd, RawFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    time::Duration,
};

const SCHEMA: &str = "org.gnome.desktop.interface";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Deserialize, Serialize)]
struct Entry {
    key: [String; 2],
    original: String,
    managed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn directory() -> Result<PathBuf> {
    Ok(state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation"))
}

fn key_valid(key: &[String; 2]) -> Result<()> {
    if !matches!(key[0].as_str(), "gsettings" | "xdg-mime")
        || key[1].is_empty()
        || key[1].starts_with('-')
        || key[1].chars().any(char::is_control)
    {
        return Err(invalid("Invalid native preference key"));
    }
    Ok(())
}

fn checked(args: Vec<String>) -> Result<String> {
    let output = process::checked(&args, None, TIMEOUT, true)?;
    Ok(String::from_utf8(output.stdout)
        .map_err(|_| invalid("Native preference returned invalid UTF-8"))?
        .trim()
        .to_string())
}

fn get(key: &[String; 2]) -> Result<String> {
    key_valid(key)?;
    checked(if key[0] == "gsettings" {
        vec![
            "gsettings".into(),
            "get".into(),
            SCHEMA.into(),
            key[1].clone(),
        ]
    } else {
        vec![
            "xdg-mime".into(),
            "query".into(),
            "default".into(),
            key[1].clone(),
        ]
    })
}

fn set(key: &[String; 2], value: &str) -> Result<()> {
    if key[0] == "gsettings" {
        checked(vec![
            "gsettings".into(),
            "set".into(),
            SCHEMA.into(),
            key[1].clone(),
            value.into(),
        ])?;
    } else if !value.is_empty() {
        checked(vec![
            "xdg-mime".into(),
            "default".into(),
            value.into(),
            key[1].clone(),
        ])?;
    }
    // Empty legacy MIME values are restored by the separate MIME file journal.
    Ok(())
}

fn lock(directory: &Path, inherited: Option<RawFd>) -> Result<File> {
    if !directory.is_absolute() {
        return Err(invalid("Preference state directory must be absolute"));
    }
    state::private_directory(directory)?;
    let path = directory.join("preferences.lock");
    let Some(fd) = inherited else {
        return state::lock(&path, TIMEOUT);
    };
    // The Python recovery bridge holds the same open-file-description lock
    // across MIME restoration. Duplicate its inherited descriptor so dropping
    // this handle cannot close the bridge's descriptor or release its lock.
    // SAFETY: fcntl validates the caller-supplied descriptor and returns our own.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: duplicate is a new descriptor owned exclusively by this File.
    let file = unsafe { File::from_raw_fd(duplicate) };
    let actual = file.metadata()?;
    let expected = fs::symlink_metadata(&path)?;
    if !expected.is_file()
        || expected.file_type().is_symlink()
        || actual.dev() != expected.dev()
        || actual.ino() != expected.ino()
    {
        return Err(invalid("Inherited preference lock is not the state lock"));
    }
    // Keep the inherited lock out of gsettings/xdg-mime child processes. FD
    // flags are local to this process; the bridge retains its original handle.
    // SAFETY: fd was validated above and fcntl does not retain any pointer.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    file.try_lock()
        .map_err(|_| invalid("Preference state is busy"))?;
    Ok(file)
}

fn entries(directory: &Path) -> Result<Vec<Entry>> {
    let path = directory.join("preferences.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1024 * 1024 {
        return Err(invalid("Invalid preference journal"));
    }
    let mut data = Vec::new();
    File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 1024 * 1024 {
        return Err(invalid("Preference journal is too large"));
    }
    let entries: Vec<Entry> = serde_json::from_slice(&data)?;
    let mut keys = HashSet::new();
    for entry in &entries {
        key_valid(&entry.key)?;
        if !keys.insert(entry.key.clone()) {
            return Err(invalid("Duplicate preference journal key"));
        }
    }
    Ok(entries)
}

fn publish(directory: &Path, entries: &[Entry]) -> Result<()> {
    let mut content = serde_json::to_vec_pretty(entries)?;
    content.push(b'\n');
    state::atomic(&directory.join("preferences.json"), &content, 0o600)
}

fn owns(entry: &Entry, current: &str) -> Result<()> {
    if current != entry.managed && entry.pending.as_deref() != Some(current) {
        return Err(invalid(format!(
            "Preference changed outside installation: {}",
            entry.key[1]
        )));
    }
    Ok(())
}

fn validate(entries: &[Entry]) -> Result<()> {
    for entry in entries {
        owns(entry, &get(&entry.key)?)?
    }
    Ok(())
}

fn manage(directory: &Path, key: [String; 2], value: &str) -> Result<bool> {
    key_valid(&key)?;
    let mut entries = entries(directory)?;
    let current = get(&key)?;
    let index = if let Some(index) = entries.iter().position(|entry| entry.key == key) {
        owns(&entries[index], &current)?;
        index
    } else {
        entries.push(Entry {
            key: key.clone(),
            original: current.clone(),
            managed: current.clone(),
            pending: None,
            extra: BTreeMap::new(),
        });
        entries.len() - 1
    };
    entries[index].pending = Some(value.into());
    publish(directory, &entries)?;
    let changed = current != value;
    if changed {
        set(&key, value)?
    }
    entries[index].managed = entries[index].pending.take().unwrap();
    publish(directory, &entries)?;
    Ok(changed)
}

pub fn manage_setting(
    directory: &Path,
    key: [String; 2],
    value: &str,
    inherited: Option<RawFd>,
) -> Result<Value> {
    let _lock = lock(directory, inherited)?;
    Ok(json!({"changed": manage(directory, key, value)?}))
}

pub fn check(directory: &Path, inherited: Option<RawFd>) -> Result<Value> {
    let _lock = lock(directory, inherited)?;
    let entries = entries(directory)?;
    validate(&entries)?;
    Ok(json!({"checked": entries.len()}))
}

/// The separate MIME-role adapter may finish restoring a legacy MIME key before
/// the preference loop reaches it. Record that expected value before it runs,
/// while preserving strict ownership checks for every other value.
pub fn prepare_restore(directory: &Path, inherited: Option<RawFd>) -> Result<Value> {
    let _lock = lock(directory, inherited)?;
    let mut entries = entries(directory)?;
    validate(&entries)?;
    let mut changed = false;
    for entry in &mut entries {
        if entry.key[0] == "xdg-mime" {
            let current = get(&entry.key)?;
            owns(entry, &current)?;
            entry.managed = current;
            entry.pending = Some(entry.original.clone());
            changed = true;
        }
    }
    if changed {
        publish(directory, &entries)?
    }
    Ok(json!({"checked": entries.len()}))
}

pub fn restore(directory: &Path, inherited: Option<RawFd>) -> Result<Value> {
    let _lock = lock(directory, inherited)?;
    let mut entries = entries(directory)?;
    validate(&entries)?;
    let count = entries.len();
    while !entries.is_empty() {
        let current = get(&entries[0].key)?;
        owns(&entries[0], &current)?;
        entries[0].pending = Some(entries[0].original.clone());
        publish(directory, &entries)?;
        if current != entries[0].original {
            set(&entries[0].key, &entries[0].original)?
        }
        entries.remove(0);
        publish(directory, &entries)?;
    }
    match fs::remove_file(directory.join("preferences.json")) {
        Ok(()) => File::open(directory)?.sync_all()?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(json!({"restored": count}))
}

pub fn gtk_refresh(directory: &Path) -> Result<Value> {
    let revision = directory.join("theme/current/semantic.json");
    if !revision.is_file() {
        return Ok(json!({"status": "absent"}));
    }
    let supported = match checked(vec![
        "gsettings".into(),
        "range".into(),
        SCHEMA.into(),
        "accent-color".into(),
    ]) {
        Ok(supported) => supported,
        Err(_) => return Ok(json!({"status": "unavailable"})),
    };
    let mut input = Vec::new();
    File::open(revision)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut input)?;
    if input.len() > 64 * 1024 {
        return Err(invalid("Palette JSON is too large"));
    }
    let palette: Value = serde_json::from_slice(&input)?;
    let accent = theme::gtk_accent(&palette)?;
    let quoted = format!("'{accent}'");
    if !supported.lines().any(|line| line.trim() == quoted) {
        return Ok(json!({"status": "unsupported", "accent": accent}));
    }
    let _lock = lock(directory, None)?;
    let changed = manage(
        directory,
        ["gsettings".into(), "accent-color".into()],
        &quoted,
    )?;
    Ok(json!({"status": "applied", "accent": accent, "changed": changed}))
}
