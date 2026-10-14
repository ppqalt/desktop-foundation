//! Optional image-projection verification metadata. Stored bundles stay authoritative.
use super::{MAX_BYTES, MAX_ITEMS, Result, image_projection, invalid, state, valid_id};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub(super) const CACHE_NAME: &str = "projection-cache.json";
const CACHE_VERSION: u8 = 1;
const CACHE_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fingerprint {
    device: u64,
    inode: u64,
    size: u64,
    mode: u32,
    modified: i64,
    modified_ns: i64,
    changed: i64,
    changed_ns: i64,
}

impl Fingerprint {
    fn from(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            mode: metadata.mode(),
            modified: metadata.mtime(),
            modified_ns: metadata.mtime_nsec(),
            changed: metadata.ctime(),
            changed_ns: metadata.ctime_nsec(),
        }
    }

    fn valid(&self) -> bool {
        self.size > 0
            && self.size <= MAX_BYTES as u64
            && self.mode & libc::S_IFMT == libc::S_IFREG
            && (0..1_000_000_000).contains(&self.modified_ns)
            && (0..1_000_000_000).contains(&self.changed_ns)
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verified {
    sha256: String,
    file: Fingerprint,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u8,
    images: BTreeMap<String, Verified>,
}

pub(super) struct Cache {
    images: BTreeMap<String, Verified>,
    dirty: bool,
}

impl Cache {
    pub(super) fn load(directory: &Path) -> Self {
        let loaded = (|| -> Option<Saved> {
            let file = private_read(&directory.join(CACHE_NAME)).ok()?;
            let metadata = file.metadata().ok()?;
            if !metadata.is_file()
                || metadata.len() > CACHE_BYTES
                || metadata.mode() & 0o777 != 0o600
            {
                return None;
            }
            let mut bytes = Vec::new();
            file.take(CACHE_BYTES + 1).read_to_end(&mut bytes).ok()?;
            if bytes.len() as u64 > CACHE_BYTES {
                return None;
            }
            let saved: Saved = serde_json::from_slice(&bytes).ok()?;
            (saved.version == CACHE_VERSION
                && saved.images.len() <= MAX_ITEMS * 2
                && saved.images.iter().all(|(name, verified)| {
                    image_projection(name) && valid_id(&verified.sha256) && verified.file.valid()
                }))
            .then_some(saved)
        })();
        Self {
            images: loaded.map_or_else(BTreeMap::new, |saved| saved.images),
            dirty: false,
        }
    }

    pub(super) fn verify(&mut self, name: &str, path: &Path, data: &[u8]) -> Result<bool> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => return Err(invalid("Invalid image projection path")),
            Err(_) => {
                self.forget(name);
                return Ok(false);
            }
        };
        let fingerprint = Fingerprint::from(&metadata);
        let sha256 = format!("{:x}", Sha256::digest(data));
        if self
            .images
            .get(name)
            .is_some_and(|verified| verified.sha256 == sha256 && verified.file == fingerprint)
        {
            return Ok(true);
        }
        self.forget(name);
        let Some(file) = byte_verified(path, data, &fingerprint)? else {
            return Ok(false);
        };
        self.images.insert(name.into(), Verified { sha256, file });
        self.dirty = true;
        Ok(true)
    }

    fn forget(&mut self, name: &str) {
        self.dirty |= self.images.remove(name).is_some();
    }

    pub(super) fn publish(&mut self, directory: &Path, retained: &HashSet<String>) {
        let before = self.images.len();
        self.images.retain(|name, _| retained.contains(name));
        self.dirty |= self.images.len() != before;
        let path = directory.join(CACHE_NAME);
        if self.images.is_empty() {
            // This contains only fingerprints and hashes, never clipboard data.
            // Clear stale metadata even when loading a malformed cache failed.
            let _ = fs::remove_file(&path);
            return;
        }
        if !self.dirty {
            return;
        }
        let saved = Saved {
            version: CACHE_VERSION,
            images: self.images.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec(&saved)
            && bytes.len() as u64 <= CACHE_BYTES
        {
            // A failed optional cache write cannot turn a completed history
            // publication into a failure or replace its authoritative payloads.
            let _ = state::atomic(&path, &bytes, 0o600);
        }
    }
}

fn private_read(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

fn byte_verified(path: &Path, data: &[u8], expected: &Fingerprint) -> Result<Option<Fingerprint>> {
    let mut file = match private_read(path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };
    let metadata = file.metadata()?;
    let before = Fingerprint::from(&metadata);
    if !metadata.is_file() {
        return Err(invalid("Invalid image projection path"));
    }
    if before != *expected || before.size != data.len() as u64 {
        return Ok(None);
    }
    // Compare in bounded chunks; a replaced/growing file cannot cause an
    // unbounded allocation, even if its earlier stat matched the bundle size.
    let mut chunk = [0u8; 8192];
    for expected in data.chunks(chunk.len()) {
        match file.read_exact(&mut chunk[..expected.len()]) {
            Ok(()) if chunk[..expected.len()] == *expected => {}
            Ok(()) => return Ok(None),
            Err(_) => return Ok(None),
        }
    }
    let mut extra = [0u8; 1];
    if file.read(&mut extra).ok() != Some(0) {
        return Ok(None);
    }
    let after = Fingerprint::from(&file.metadata()?);
    let current = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if !current.is_file() {
        return Err(invalid("Invalid image projection path"));
    }
    Ok((before == after && after == Fingerprint::from(&current)).then_some(after))
}
