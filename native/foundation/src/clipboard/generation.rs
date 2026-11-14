//! Optional unchanged-generation accelerator. SQLite and bundles remain authoritative.
use super::{Entry, MAX_ITEMS, Result, image_projection, state};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

const NAME: &str = "initialization-cache.json";
const LIMIT: u64 = 256 * 1024;
// Bump whenever projection semantics or bundle validation changes.
const VERSION: u8 = 1;
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    device: u64,
    inode: u64,
    size: u64,
    mode: u32,
    uid: u32,
    modified: i64,
    modified_ns: i64,
    changed: i64,
    changed_ns: i64,
}
fn identity(path: &Path) -> Option<FileIdentity> {
    let m = fs::symlink_metadata(path).ok()?;
    // SAFETY: geteuid has no arguments or side effects.
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.mode() & 0o777 != 0o600
    {
        return None;
    }
    Some(FileIdentity {
        device: m.dev(),
        inode: m.ino(),
        size: m.len(),
        mode: m.mode(),
        uid: m.uid(),
        modified: m.mtime(),
        modified_ns: m.mtime_nsec(),
        changed: m.ctime(),
        changed_ns: m.ctime_nsec(),
    })
}
fn reference_identity(path: &Path) -> Option<FileIdentity> {
    let m = fs::metadata(path).ok()?;
    Some(FileIdentity {
        device: m.dev(),
        inode: m.ino(),
        size: m.len(),
        mode: m.mode(),
        uid: m.uid(),
        modified: m.mtime(),
        modified_ns: m.mtime_nsec(),
        changed: m.ctime(),
        changed_ns: m.ctime_nsec(),
    })
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Generation {
    version: u8,
    files: BTreeMap<String, FileIdentity>,
    references: BTreeMap<std::path::PathBuf, Option<FileIdentity>>,
}
fn capture(directory: &Path, references: &BTreeSet<std::path::PathBuf>) -> Option<Generation> {
    if references.len() > 1024 || references.iter().any(|path| !path.is_absolute()) {
        return None;
    }
    // WAL may be checkpointed after publication/connection close. Never use
    // metadata shortcuts across that boundary; normal SQLite recovery wins.
    if directory.join("history.sqlite-wal").try_exists().ok()? {
        return None;
    }
    let mut files = BTreeMap::new();
    for name in ["history.sqlite", "index.json"] {
        files.insert(name.into(), identity(&directory.join(name))?);
    }
    if files["index.json"].size > 4 * 1024 * 1024 {
        return None;
    }
    let index = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(directory.join("index.json"))
        .ok()?;
    let mut bytes = Vec::new();
    index
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let entries: Vec<Entry> = serde_json::from_slice(&bytes).ok()?;
    if entries.len() > MAX_ITEMS * 2 {
        return None;
    }
    // Referenced files are statted without reading contents. Image
    // replacement/deletion/permission changes also invalidate reuse.
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().into_string().ok()?;
        if image_projection(&name) {
            files.insert(name, identity(&entry.path())?);
            if files.len() > MAX_ITEMS * 2 + 2 {
                return None;
            }
        }
    }
    Some(Generation {
        version: VERSION,
        files,
        references: references
            .iter()
            .map(|path| (path.clone(), reference_identity(path)))
            .collect(),
    })
}
pub(super) fn unchanged(directory: &Path) -> bool {
    let load = || -> Option<Generation> {
        let path = directory.join(NAME);
        let m = identity(&path)?;
        if m.size > LIMIT {
            return None;
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .ok()?;
        let mut bytes = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > LIMIT {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    };
    let Some(saved) = load() else {
        return false;
    };
    capture(directory, &saved.references.keys().cloned().collect())
        .is_some_and(|current| saved == current)
}
pub(super) fn publish(directory: &Path, references: &BTreeSet<std::path::PathBuf>) {
    let result = || -> Result<()> {
        if let Some(generation) = capture(directory, references) {
            state::atomic(
                &directory.join(NAME),
                &serde_json::to_vec(&generation)?,
                0o600,
            )?;
        } else if directory
            .join(NAME)
            .symlink_metadata()
            .is_ok_and(|m| m.is_file())
        {
            fs::remove_file(directory.join(NAME))?;
        }
        Ok(())
    };
    // This cache is optional. Failure cannot invalidate committed clipboard data.
    let _ = result();
}
