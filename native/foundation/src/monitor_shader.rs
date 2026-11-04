//! Cache Slang's real version response as seen by Kitty's combined stream capture.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const CACHED_COMPILER: &str = "/usr/bin/slangc";
const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_VERSION: usize = 4 * 1024;
const MAX_RECORD: usize = 64 * 1024;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    path: PathBuf,
    realpath: PathBuf,
    device: u64,
    inode: u64,
    size: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

fn file_identity(path: &Path) -> Result<FileIdentity> {
    let realpath = fs::canonicalize(path)?;
    let metadata = fs::metadata(&realpath)?;
    if !metadata.is_file() {
        return Err(invalid("Slang compiler dependencies must be regular files"));
    }
    Ok(FileIdentity {
        path: path.to_owned(),
        realpath,
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
        mtime: (metadata.mtime(), metadata.mtime_nsec()),
        ctime: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

fn identity() -> Result<Vec<FileIdentity>> {
    let mut paths = vec![PathBuf::from(CACHED_COMPILER)];
    for entry in fs::read_dir("/usr/lib")? {
        let entry = entry?;
        let name = entry.file_name();
        if name.as_bytes().starts_with(b"libslang-compiler.so") {
            if paths.len() >= 33 {
                return Err(invalid("Too many Slang compiler libraries to cache"));
            }
            paths.push(entry.path());
        }
    }
    paths.sort();
    paths.iter().map(|path| file_identity(path)).collect()
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    identity: Vec<FileIdentity>,
    version: String,
}

fn uid() -> u32 {
    // SAFETY: geteuid takes no arguments or pointers.
    unsafe { libc::geteuid() }
}

fn cache_path() -> Result<PathBuf> {
    let directory = state::xdg("XDG_CACHE_HOME", ".cache")?
        .join("desktop-foundation")
        .join("monitor");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() && metadata.uid() == uid() => {}
        Ok(_) => return Err(invalid("The monitor compiler cache directory is not owned")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    state::private_directory(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.uid() != uid() || metadata.mode() & 0o077 != 0 {
        return Err(invalid("Monitor compiler cache directory is not private"));
    }
    Ok(directory.join("slang-version.json"))
}

fn version_valid(version: &str) -> bool {
    !version.trim().is_empty()
        && version.len() <= MAX_VERSION
        && version.ends_with('\n')
        && version
            .chars()
            .all(|ch| !ch.is_control() || ch.is_whitespace())
}

fn cached(path: &Path, identity: &[FileIdentity]) -> Result<String> {
    // NONBLOCK prevents a foreign FIFO from blocking before its type is checked.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != uid()
        || metadata.mode() & 0o077 != 0
        || metadata.len() > MAX_RECORD as u64
    {
        return Err(invalid("Invalid monitor compiler cache record"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD {
        return Err(invalid("Monitor compiler cache record is too large"));
    }
    let record: Record = serde_json::from_slice(&bytes)?;
    if record.identity != identity || !version_valid(&record.version) {
        return Err(invalid("Stale or damaged monitor compiler cache record"));
    }
    Ok(record.version)
}

fn emit(version: &[u8]) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(version)?;
    stdout.flush()?;
    Ok(())
}

/// Cache Kitty's combined `-version` response; other invocations remain Slang's.
pub fn compiler(args: &[String]) -> Result<()> {
    let compiler = find_compiler(std::env::var_os("PATH").as_deref())
        .ok_or_else(|| invalid("Slang compiler not found in PATH"))?;
    let compiler = compiler
        .to_str()
        .ok_or_else(|| invalid("Slang compiler path is not UTF-8"))?;
    if args.len() != 1 || args[0] != "-version" {
        let mut command = vec![compiler.to_owned()];
        command.extend_from_slice(args);
        return process::replace(&command);
    }
    // Loader overrides may select dependencies outside the installed package.
    let overridden = std::env::vars_os()
        .any(|(name, _)| name.as_bytes().starts_with(b"LD_") || name == "GLIBC_TUNABLES");
    // The cached dependency identity describes the system compiler and its
    // system libraries only. Other installations always query the real binary.
    let cache = if overridden || Path::new(compiler) != Path::new(CACHED_COMPILER) {
        None
    } else {
        cache_path().ok()
    };
    let before = cache.as_ref().and_then(|_| identity().ok());
    if let (Some(path), Some(before)) = (&cache, &before)
        && let Ok(version) = cached(path, before)
    {
        return emit(version.as_bytes());
    }
    let output = process::checked_capture(
        &[compiler.into(), "-version".into()],
        TIMEOUT,
        MAX_VERSION as u64,
    )?;
    if !output.stdout.is_empty() && !output.stderr.is_empty() {
        // Mixed streams may contain diagnostics: preserve both without caching.
        emit(&output.stdout)?;
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(&output.stderr)?;
        stderr.flush()?;
        return Ok(());
    }
    let version = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    if version.len() > MAX_VERSION {
        return Err(invalid("Slang compiler version exceeds 4 KiB"));
    }
    if let (Some(path), Some(before), Ok(text)) = (cache, before, std::str::from_utf8(&version))
        && version_valid(text)
        && identity().is_ok_and(|after| after == before)
    {
        // An upgrade during the query cannot attach an old response to new stamps.
        let record = Record {
            identity: before,
            version: text.to_owned(),
        };
        if let Ok(bytes) = serde_json::to_vec(&record)
            && bytes.len() <= MAX_RECORD
        {
            let _ = state::atomic(&path, &bytes, 0o600);
        }
    }
    emit(&version)
}

fn find_compiler(path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path?)
        .map(|directory| directory.join("slangc"))
        .find(|candidate| {
            candidate.metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn discovers_executable_compiler_on_supplied_path() {
        let dir =
            std::env::temp_dir().join(format!("foundation-slang-search-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("slangc");
        fs::write(&binary, "fixture").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(find_compiler(Some(dir.as_os_str())).is_none());
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_compiler(Some(dir.as_os_str())), Some(binary));
        assert!(find_compiler(None).is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
