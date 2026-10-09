//! Per-key MIME defaults with durable ownership and legacy recovery journals.
use crate::{Result, apps, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const SECTION: &str = "Default Applications";
const LIMIT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Deserialize, Serialize)]
struct Entry {
    original: Option<String>,
    managed: Option<String>,
    // Flattening preserves an explicit null pending restoration as distinct
    // from no pending write, as well as older journals' extra metadata.
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl Entry {
    fn pending(&self) -> Result<Option<Option<&str>>> {
        self.extra.get("pending").map(optional_string).transpose()
    }

    fn accepts(&self, current: Option<&str>, restoring: bool) -> Result<bool> {
        Ok(current == self.managed.as_deref()
            || self.pending()?.is_some_and(|pending| current == pending)
            || restoring && current == self.original.as_deref())
    }
}

#[derive(Deserialize, Serialize)]
struct Journal {
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    entries: BTreeMap<String, Entry>,
    #[serde(rename = "fileExisted")]
    file_existed: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct Legacy {
    path: String,
    existed: bool,
    managed: Option<String>,
}

struct Paths {
    file: PathBuf,
    journal: PathBuf,
    directory: PathBuf,
}

fn paths() -> Result<Paths> {
    let directory = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation");
    check_state_path(&directory, true)?;
    Ok(Paths {
        file: state::xdg("XDG_CONFIG_HOME", ".config")?.join("mimeapps.list"),
        journal: directory.join("application-roles.json"),
        directory,
    })
}

fn optional_string(value: &Value) -> Result<Option<&str>> {
    if value.is_null() {
        Ok(None)
    } else {
        value
            .as_str()
            .map(Some)
            .ok_or_else(|| invalid("Invalid MIME journal value"))
    }
}

fn read(path: &Path, limit: u64, owned: bool) -> Result<Option<String>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | if owned { libc::O_NOFOLLOW } else { 0 })
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    // SAFETY: geteuid has no pointer arguments or side effects.
    if !metadata.is_file()
        || metadata.len() > limit
        || owned && metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(invalid(format!(
            "Invalid application metadata: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid(format!(
            "Application metadata is too large: {}",
            path.display()
        )));
    }
    String::from_utf8(bytes).map(Some).map_err(|_| {
        invalid(format!(
            "Application metadata is not UTF-8: {}",
            path.display()
        ))
    })
}

fn mime_valid(key: &str) -> Result<()> {
    if key.is_empty()
        || key.len() > 512
        || key.starts_with('-')
        || key.starts_with('#')
        || key
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "=;[]".contains(c))
    {
        return Err(invalid("Invalid MIME default key"));
    }
    Ok(())
}

pub fn roles(root: &Path, personal: bool) -> Result<Value> {
    let contents = read(
        &root.join("config/application-roles.json"),
        64 * 1024,
        false,
    )?
    .ok_or_else(|| invalid("Application role configuration is missing"))?;
    let mut value: Value = serde_json::from_str(&contents)?;
    let entries = value
        .as_object_mut()
        .ok_or_else(|| invalid("Invalid application role configuration"))?;
    let browser = entries
        .remove("personalBrowser")
        .ok_or_else(|| invalid("Personal browser configuration is missing"))?;
    if personal {
        entries.insert("browser".into(), browser);
    }
    for (name, role) in entries {
        configured_role(name, role)?;
    }
    Ok(value)
}

fn configured_role(name: &str, value: &Value) -> Result<apps::ApplicationRole> {
    let role: apps::ApplicationRole = serde_json::from_value(value.clone())?;
    if name.is_empty()
        || name.chars().any(char::is_control)
        || role.package.is_empty()
        || role.executable.is_empty()
        || role.executable.contains('/')
        || role.executable.chars().any(char::is_control)
        || role.mimes.len() > 256
    {
        return Err(invalid(format!(
            "Invalid configured application role: {name}"
        )));
    }
    desktop_name(&role.desktop)?;
    for mime in &role.mimes {
        mime_valid(mime)?;
    }
    Ok(role)
}

fn application_record(name: &str, value: &Value) -> Result<apps::ApplicationRole> {
    let mut record = value.clone();
    record
        .as_object_mut()
        .ok_or_else(|| invalid("Invalid application metadata record"))?
        .entry("mimes")
        .or_insert_with(|| json!([]));
    configured_role(name, &record)
}

fn desktop_name(name: &str) -> Result<()> {
    if name.contains('/')
        || name.chars().any(char::is_control)
        || !name.ends_with(".desktop")
        || name.len() > 255
    {
        return Err(invalid("Invalid application desktop filename"));
    }
    Ok(())
}

pub fn desktop_path(name: &str) -> Result<Option<PathBuf>> {
    desktop_name(name)?;
    let mut directories = vec![state::xdg("XDG_DATA_HOME", ".local/share")?];
    directories.extend(
        env::split_paths(
            &env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
        )
        .filter(|path| !path.as_os_str().is_empty()),
    );
    Ok(directories
        .into_iter()
        .map(|path| path.join("applications").join(name))
        .find(|path| path.is_file()))
}

pub fn validate(name: &str, value: &Value) -> Result<PathBuf> {
    let role = application_record(name, value)?;
    let desktop = desktop_path(&role.desktop)?;
    if !apps::executable(&role.executable) || desktop.is_none() {
        return Err(invalid(format!(
            "Missing {name}: install {} ({})",
            role.package, role.desktop
        )));
    }
    let desktop = desktop.ok_or_else(|| invalid("Application desktop entry disappeared"))?;
    let text = read(&desktop, 256 * 1024, false)?
        .ok_or_else(|| invalid("Application desktop entry disappeared"))?;
    apps::validate_entry(&text, &role.executable)
        .map_err(|error| invalid(format!("{error}: {}", desktop.display())))?;
    Ok(desktop)
}

fn section_header(line: &str) -> Result<Option<&str>> {
    if !line.trim_start().starts_with('[') {
        return Ok(None);
    }
    let name = line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .ok_or_else(|| invalid("Malformed MIME section header"))?;
    if name.is_empty() || name.chars().any(|c| c.is_control() || "[]".contains(c)) {
        return Err(invalid("Malformed MIME section header"));
    }
    Ok(Some(name))
}

pub fn values(text: &str) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    let mut section = "";
    let mut found = false;
    for line in text.lines() {
        if let Some(name) = section_header(line)? {
            section = name;
            if section == SECTION {
                if found {
                    return Err(invalid("Duplicate MIME defaults section"));
                }
                found = true;
            }
        } else if section == SECTION
            && !line.trim_start().starts_with(['#', ';'])
            && let Some((key, value)) = line.split_once('=')
        {
            let key = key.trim();
            if result.insert(key.into(), value.trim().into()).is_some() {
                return Err(invalid(format!("Duplicate MIME default: {key}")));
            }
        }
    }
    Ok(result)
}

pub fn replace(text: &str, changes: &BTreeMap<String, Option<String>>) -> Result<String> {
    values(text)?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut output = String::new();
    let mut section = "";
    let mut remaining = changes.clone();
    let mut found = false;
    let append = |output: &mut String, changes: &BTreeMap<String, Option<String>>| {
        for (key, value) in changes {
            if let Some(value) = value {
                output.push_str(&format!("{key}={value}{newline}"));
            }
        }
    };
    for raw in text.split_inclusive('\n') {
        let line = raw
            .strip_suffix("\r\n")
            .or_else(|| raw.strip_suffix('\n'))
            .unwrap_or(raw);
        if let Some(name) = section_header(line)? {
            if section == SECTION {
                append(&mut output, &remaining);
                remaining.clear();
            }
            section = name;
            found |= section == SECTION;
        }
        if section == SECTION
            && !line.trim_start().starts_with(['#', ';'])
            && let Some((key, _)) = line.split_once('=')
            && let Some(value) = remaining.remove(key.trim())
        {
            if let Some(value) = value {
                let ending = if raw.ends_with("\r\n") {
                    "\r\n"
                } else if raw.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                output.push_str(&format!("{}={value}{ending}", key.trim()));
            }
            continue;
        }
        output.push_str(raw);
    }
    if remaining.values().any(Option::is_some) {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push_str(newline);
        }
        if !found {
            output.push_str(&format!("{newline}[{SECTION}]{newline}"));
        }
        append(&mut output, &remaining);
    }
    if output.len() as u64 > LIMIT {
        return Err(invalid("MIME defaults file is too large"));
    }
    Ok(output)
}

fn load_journal(path: &Path) -> Result<Option<Journal>> {
    let Some(text) = read(path, LIMIT, true)? else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(&text)?;
    let saved: Journal = serde_json::from_value(value.clone())?;
    for key in saved.entries.keys() {
        let raw = &value["entries"][key];
        if raw.get("original").is_none() || raw.get("managed").is_none() {
            return Err(invalid("Incomplete MIME journal entry"));
        }
    }
    validate_journal(&saved)?;
    Ok(Some(saved))
}

fn validate_journal(saved: &Journal) -> Result<()> {
    if saved.entries.len() > 4096 {
        return Err(invalid("Too many owned MIME defaults"));
    }
    for (key, entry) in &saved.entries {
        mime_valid(key)?;
        for value in [
            entry.original.as_deref(),
            entry.managed.as_deref(),
            entry.pending()?.flatten(),
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 64 * 1024 || value.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
                return Err(invalid("Invalid MIME journal value"));
            }
        }
    }
    Ok(())
}

fn ownership(saved: &Journal, current: &BTreeMap<String, String>, restoring: bool) -> Result<()> {
    for (key, entry) in &saved.entries {
        if !entry.accepts(current.get(key).map(String::as_str), restoring)? {
            return Err(invalid(format!(
                "MIME default changed outside installation: {key}"
            )));
        }
    }
    Ok(())
}

fn publish(path: &Path, saved: &Journal) -> Result<()> {
    validate_journal(saved)?;
    let mut bytes = serde_json::to_vec_pretty(saved)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("MIME journal is too large"));
    }
    state::atomic(path, &bytes, 0o600)
}

fn check_state_path(path: &Path, directory: bool) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            // SAFETY: geteuid has no pointer arguments or side effects.
            if metadata.file_type().is_symlink()
                || metadata.uid() != unsafe { libc::geteuid() }
                || if directory {
                    !metadata.is_dir()
                } else {
                    !metadata.is_file()
                }
            {
                return Err(invalid("Invalid application-role state ownership"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn mutation_lock(paths: &Paths) -> Result<File> {
    check_state_path(&paths.directory, true)?;
    check_state_path(&paths.directory.join("roles.lock"), false)?;
    state::private_directory(&paths.directory)?;
    state::lock(&paths.directory.join("roles.lock"), TIMEOUT)
}

// Loading never writes. Every ownership guard runs before either migration
// evidence or managed defaults are published.
fn transaction(paths: &Paths, text: &str, existed: bool) -> Result<(Journal, Option<String>)> {
    let saved = load_journal(&paths.journal)?;
    let legacy_path = paths.directory.join("mimeapps.json");
    let legacy_text = read(&legacy_path, LIMIT, true)?;
    let Some(legacy_text) = legacy_text else {
        return Ok((
            saved.unwrap_or(Journal {
                profile: None,
                entries: BTreeMap::new(),
                file_existed: existed,
                extra: BTreeMap::new(),
            }),
            None,
        ));
    };
    let legacy: Legacy = serde_json::from_str(&legacy_text)?;
    if legacy.path != paths.file.to_string_lossy() || legacy.managed.is_none() {
        return Err(invalid(
            "Incomplete legacy MIME transaction; review before migration",
        ));
    }
    let archive = paths.directory.join("mimeapps.v011-migrated.json");
    if let Some(archived) = read(&archive, LIMIT, true)?
        && archived != legacy_text
    {
        return Err(invalid("Legacy MIME migration archive already exists"));
    }
    let original = if legacy.existed {
        values(
            &read(&paths.directory.join("mimeapps.original"), LIMIT, true)?
                .ok_or_else(|| invalid("Legacy MIME original backup is missing"))?,
        )?
    } else {
        BTreeMap::new()
    };
    let managed = values(legacy.managed.as_deref().unwrap())?;
    let saved = if let Some(saved) = saved {
        for (key, value) in &managed {
            if original.get(key) != Some(value)
                && !saved
                    .entries
                    .get(key)
                    .is_some_and(|entry| entry.original.as_ref() == original.get(key))
            {
                return Err(invalid(format!(
                    "Existing MIME journal differs from legacy ownership: {key}"
                )));
            }
        }
        if saved.file_existed != legacy.existed {
            return Err(invalid(
                "Existing MIME journal differs from legacy file ownership",
            ));
        }
        saved
    } else {
        let current = values(text)?;
        let mut entries = BTreeMap::new();
        for (key, value) in managed {
            if original.get(&key) != Some(&value) {
                mime_valid(&key)?;
                if current.get(&key) != Some(&value) {
                    return Err(invalid(format!(
                        "Legacy owned MIME default changed externally: {key}"
                    )));
                }
                entries.insert(
                    key.clone(),
                    Entry {
                        original: original.get(&key).cloned(),
                        managed: Some(value),
                        extra: BTreeMap::new(),
                    },
                );
            }
        }
        Journal {
            profile: Some("personal".into()),
            entries,
            file_existed: legacy.existed,
            extra: BTreeMap::new(),
        }
    };
    Ok((saved, Some(legacy_text)))
}

fn archive_legacy(paths: &Paths, legacy: Option<&str>) -> Result<()> {
    let Some(legacy) = legacy else {
        return Ok(());
    };
    let source = paths.directory.join("mimeapps.json");
    let archive = paths.directory.join("mimeapps.v011-migrated.json");
    match fs::hard_link(&source, &archive) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if read(&archive, LIMIT, true)?.as_deref() != Some(legacy) {
                return Err(invalid("Legacy MIME migration archive already exists"));
            }
        }
        Err(error) => return Err(error.into()),
    }
    File::open(&paths.directory)?.sync_all()?;
    fs::remove_file(source)?;
    File::open(&paths.directory)?.sync_all()?;
    Ok(())
}

pub fn migrate_legacy(file: &Path, journal: &Path) -> Result<()> {
    let paths = Paths {
        file: file.into(),
        journal: journal.into(),
        directory: journal
            .parent()
            .ok_or_else(|| invalid("MIME journal requires a parent"))?
            .into(),
    };
    let _lock = mutation_lock(&paths)?;
    let text = read(file, LIMIT, true)?;
    let (saved, legacy) = transaction(&paths, text.as_deref().unwrap_or(""), text.is_some())?;
    if legacy.is_some() {
        ownership(&saved, &values(text.as_deref().unwrap_or(""))?, false)?;
        publish(journal, &saved)?;
        archive_legacy(&paths, legacy.as_deref())?;
    }
    Ok(())
}

pub fn conflicts() -> Result<Vec<String>> {
    let paths = paths()?;
    let Some(saved) = load_journal(&paths.journal)? else {
        return Ok(Vec::new());
    };
    let current = values(&read(&paths.file, LIMIT, true)?.unwrap_or_default())?;
    let mut failures = Vec::new();
    for (key, entry) in saved.entries {
        if !entry.accepts(current.get(&key).map(String::as_str), false)? {
            failures.push(format!("MIME default changed externally: {key}"));
        }
    }
    Ok(failures)
}

fn query(args: &[&str]) -> Result<String> {
    let args = args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let output = process::checked(&args, None, TIMEOUT, true)?;
    Ok(String::from_utf8(output.stdout)
        .map_err(|_| invalid("XDG default query returned invalid UTF-8"))?
        .trim()
        .into())
}

pub fn check(root: &Path, personal: bool) -> Result<Value> {
    let desired = roles(root, personal)?;
    let mut failures = conflicts()?;
    let mut messages = failures
        .iter()
        .map(|failure| format!("FAIL: {failure}"))
        .collect::<Vec<_>>();
    for (name, value) in desired.as_object().unwrap() {
        let role = configured_role(name, value)?;
        let result = (|| {
            validate(name, value)?;
            for mime in &role.mimes {
                let actual = query(&["xdg-mime", "query", "default", mime])?;
                if actual != role.desktop {
                    return Err(invalid(format!(
                        "{mime} resolves to {}, expected {}",
                        if actual.is_empty() {
                            "(unset)"
                        } else {
                            &actual
                        },
                        role.desktop
                    )));
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => messages.push(format!("PASS application role {name}: {}", role.desktop)),
            Err(error) => {
                messages.push(format!("FAIL application role {name}: {error}"));
                failures.push(error.to_string());
            }
        }
    }
    let expected = desired["browser"]["desktop"]
        .as_str()
        .ok_or_else(|| invalid("Browser role is missing"))?;
    match query(&["xdg-settings", "get", "default-web-browser"]) {
        Ok(actual) if actual == expected => (),
        Ok(actual) => failures.push(format!("Default browser differs: {actual}")),
        Err(error) => failures.push(error.to_string()),
    }
    Ok(json!({"failures": failures, "messages": messages}))
}

pub fn apply(root: &Path, personal: bool) -> Result<Value> {
    let desired = roles(root, personal)?;
    let mut changes = BTreeMap::new();
    for (name, value) in desired.as_object().unwrap() {
        validate(name, value)?;
        let role = configured_role(name, value)?;
        for mime in role.mimes {
            changes.insert(mime, Some(format!("{};", role.desktop)));
        }
    }
    let paths = paths()?;
    let _lock = mutation_lock(&paths)?;
    let original = read(&paths.file, LIMIT, true)?;
    let text = original.as_deref().unwrap_or("");
    let current = values(text)?;
    let (mut saved, legacy) = transaction(&paths, text, original.is_some())?;
    ownership(&saved, &current, false)?;
    let replaced = replace(text, &changes)?;
    for (mime, value) in &changes {
        let entry = saved.entries.entry(mime.clone()).or_insert_with(|| Entry {
            original: current.get(mime).cloned(),
            managed: current.get(mime).cloned(),
            extra: BTreeMap::new(),
        });
        entry.extra.insert("pending".into(), json!(value));
    }
    let profile = if personal { "personal" } else { "core" };
    saved.profile = Some(profile.into());
    publish(&paths.journal, &saved)?;
    archive_legacy(&paths, legacy.as_deref())?;
    fs::create_dir_all(paths.file.parent().unwrap())?;
    state::atomic(&paths.file, replaced.as_bytes(), 0o644)?;
    for (mime, value) in &changes {
        let entry = saved.entries.get_mut(mime).unwrap();
        entry.managed = value.clone();
        entry.extra.remove("pending");
    }
    publish(&paths.journal, &saved)?;
    if !check(root, personal)?["failures"]
        .as_array()
        .unwrap()
        .is_empty()
    {
        return Err(invalid(
            "XDG default verification failed; inspect overriding desktop-specific mimeapps.list. Backups retained.",
        ));
    }
    Ok(json!({"profile": profile, "changed": replaced != text}))
}

pub fn restore() -> Result<Value> {
    let paths = paths()?;
    let _lock = mutation_lock(&paths)?;
    if read(&paths.journal, LIMIT, true)?.is_none()
        && read(&paths.directory.join("mimeapps.json"), LIMIT, true)?.is_none()
    {
        return Ok(json!({"restored": 0}));
    }
    let original = read(&paths.file, LIMIT, true)?;
    let text = original.as_deref().unwrap_or("");
    let (mut saved, legacy) = transaction(&paths, text, original.is_some())?;
    ownership(&saved, &values(text)?, true)?;
    let changes = saved
        .entries
        .iter()
        .map(|(key, entry)| (key.clone(), entry.original.clone()))
        .collect();
    let replaced = replace(text, &changes)?;
    for entry in saved.entries.values_mut() {
        entry.extra.insert("pending".into(), json!(entry.original));
    }
    publish(&paths.journal, &saved)?;
    archive_legacy(&paths, legacy.as_deref())?;
    if !saved.file_existed
        && replaced
            .lines()
            .all(|line| line.trim().is_empty() || line.trim() == "[Default Applications]")
    {
        match fs::remove_file(&paths.file) {
            Ok(()) => File::open(paths.file.parent().unwrap())?.sync_all()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    } else {
        fs::create_dir_all(paths.file.parent().unwrap())?;
        state::atomic(&paths.file, replaced.as_bytes(), 0o644)?;
    }
    fs::remove_file(&paths.journal)?;
    File::open(&paths.directory)?.sync_all()?;
    Ok(json!({"restored": saved.entries.len()}))
}

pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let action = args.first().map(String::as_str).unwrap_or("roles");
    let personal = args.get(1).is_some_and(|arg| arg == "--personal");
    match action {
        "roles" | "apply" | "check" if args.len() <= 2 && (args.len() < 2 || personal) => {
            match action {
                "roles" => roles(root, personal),
                "apply" => apply(root, personal),
                _ => check(root, personal),
            }
        }
        "validate" if args.len() == 2 => {
            let mut input = Vec::new();
            std::io::stdin()
                .take(64 * 1024 + 1)
                .read_to_end(&mut input)?;
            if input.len() > 64 * 1024 {
                return Err(invalid("Application role input is too large"));
            }
            Ok(json!(validate(&args[1], &serde_json::from_slice(&input)?)?))
        }
        "desktop" if args.len() == 2 => Ok(json!(desktop_path(&args[1])?)),
        "conflicts" if args.len() == 1 => Ok(json!(conflicts()?)),
        "restore" if args.len() == 1 => restore(),
        _ => Err(invalid(
            "Usage: application-roles roles|apply|check [--personal], validate ROLE, desktop NAME, conflicts, or restore",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_preserves_unrelated_bytes_and_crlf() {
        let original = "# user comment\r\n[Default Applications]\r\ntext/plain=old.desktop;\r\napplication/unrelated=keep.desktop;\r\n\r\n[Added Associations]\r\nimage/png=other.desktop;\r\n";
        let changes = BTreeMap::from([
            ("text/plain".into(), Some("new.desktop;".into())),
            ("application/pdf".into(), Some("papers.desktop;".into())),
        ]);
        let managed = replace(original, &changes).unwrap();
        assert!(managed.contains("application/unrelated=keep.desktop;\r\n\r\n"));
        assert!(managed.contains("[Added Associations]\r\nimage/png=other.desktop;\r\n"));
        let restored = replace(
            &managed,
            &BTreeMap::from([
                ("text/plain".into(), Some("old.desktop;".into())),
                ("application/pdf".into(), None),
            ]),
        )
        .unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn ambiguous_or_malformed_sections_fail_before_replacement() {
        assert!(values("[Default Applications]\ntext/plain=a;\n[Other]\n[Default Applications]\napplication/pdf=b;\n").is_err());
        assert!(values("[Default Applications]\ntext/plain=a;\ntext/plain=b;\n").is_err());
        for original in [
            "[Other]\n[Default Applications]  \ntext/plain=old;\n",
            "[Other]\n[Default Applications]\r\r\ntext/plain=old;\n",
            "[Default Applications]\ntext/plain=old;\n [Added Associations]\nimage/png=user.desktop;\n",
            "[Default Applications]\ntext/plain=old;\n[Added Associations]  \nimage/png=user.desktop;\n",
        ] {
            assert!(values(original).is_err());
            assert!(
                replace(
                    original,
                    &BTreeMap::from([("application/pdf".into(), Some("papers.desktop;".into()))])
                )
                .is_err()
            );
        }
    }

    #[test]
    fn personal_application_records_do_not_require_mime_mappings() {
        let records: Value =
            serde_json::from_str(include_str!("../../../apps/personal.json")).unwrap();
        for (name, value) in records.as_object().unwrap() {
            let record = application_record(name, value).unwrap();
            assert!(record.mimes.is_empty());
            assert_eq!(record.executable, value["executable"].as_str().unwrap());
            assert!(configured_role(name, value).is_err());
            let mut invalid = value.clone();
            invalid["mimes"] = json!(["#invalid/type"]);
            assert!(application_record(name, &invalid).is_err());
            invalid["mimes"] = Value::Null;
            assert!(application_record(name, &invalid).is_err());
        }
    }

    #[test]
    fn adding_a_default_after_an_unterminated_changed_key_inserts_a_delimiter() {
        for newline in ["\n", "\r\n"] {
            let original = format!("[Default Applications]{newline}text/plain=old.desktop;");
            let managed = replace(
                &original,
                &BTreeMap::from([
                    ("text/plain".into(), Some("new.desktop;".into())),
                    ("application/pdf".into(), Some("papers.desktop;".into())),
                ]),
            )
            .unwrap();
            assert!(managed.contains(&format!(
                "text/plain=new.desktop;{newline}application/pdf=papers.desktop;{newline}"
            )));
            assert_eq!(values(&managed).unwrap().len(), 2);
        }
    }

    #[test]
    fn explicit_null_pending_allows_interrupted_removal() {
        let mut entry = Entry {
            original: None,
            managed: Some("new.desktop;".into()),
            extra: BTreeMap::new(),
        };
        assert!(!entry.accepts(None, false).unwrap());
        entry.extra.insert("pending".into(), Value::Null);
        assert!(entry.accepts(None, false).unwrap());
        assert!(entry.accepts(Some("new.desktop;"), false).unwrap());
        assert!(!entry.accepts(Some("foreign.desktop;"), true).unwrap());
    }
}
