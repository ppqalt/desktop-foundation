//! Source-only identity, shared by the build script and read-only diagnostics.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn collect(
    directory: &Path,
    files: &mut Vec<PathBuf>,
    directories: &mut Vec<PathBuf>,
) -> io::Result<()> {
    directories.push(directory.to_path_buf());
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), files, directories)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "rs")
        {
            if !kind.is_file() {
                return Err(invalid("Native sources must be regular files"));
            }
            files.push(entry.path());
        }
    }
    Ok(())
}

pub fn inputs(manifest: &Path) -> io::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = ["Cargo.toml", "Cargo.lock", "build.rs"]
        .map(|name| manifest.join(name))
        .to_vec();
    let mut directories = Vec::new();
    collect(&manifest.join("src"), &mut files, &mut directories)?;
    files.sort();
    directories.sort();
    Ok((files, directories))
}

pub fn digest(manifest: &Path) -> io::Result<String> {
    let (files, _) = inputs(manifest)?;
    let mut hasher = Sha256::new();
    hasher.update(b"desktop-foundation-native-source-v1\0");
    let mut total = 0_u64;
    for path in files {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() > MAX_FILE {
            return Err(invalid("Native source is not a regular file within 16 MiB"));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| invalid("Native sources are too large"))?;
        if total > MAX_TOTAL {
            return Err(invalid("Native sources exceed 64 MiB"));
        }
        let relative = path
            .strip_prefix(manifest)
            .map_err(|_| invalid("Native source escaped its directory"))?;
        let name = relative
            .to_str()
            .ok_or_else(|| invalid("Native source path must be UTF-8"))?;
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(metadata.len().to_le_bytes());
        let mut file = File::open(&path)?;
        let mut read = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > metadata.len() {
                return Err(invalid("Native source changed while reading"));
            }
            hasher.update(&buffer[..count]);
        }
        if read != metadata.len() {
            return Err(invalid("Native source changed while reading"));
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}
