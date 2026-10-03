//! Private, durable publication shared by one-shot backend commands.
use crate::{Result, invalid};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn home() -> Result<PathBuf> {
    env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| invalid("HOME is unavailable"))
}

pub fn xdg(variable: &str, fallback: &str) -> Result<PathBuf> {
    let path = match env::var_os(variable).filter(|s| !s.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home()?.join(fallback),
    };
    if !path.is_absolute() {
        return Err(invalid(format!("{variable} must be an absolute directory")));
    }
    Ok(path)
}

pub fn private_directory(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(invalid(format!(
            "Refusing symlink state directory: {}",
            path.display()
        )));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn lock(path: &Path, timeout: Duration) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                // Only contention on a user-requested operation, never an idle worker.
                thread::sleep(Duration::from_millis(10));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(invalid(format!(
                    "State is busy: {}; retry shortly",
                    path.display()
                )));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn atomic(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Publication requires a parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("Publication requires a filename"))?;
    let (mut file, temporary) = loop {
        let mut candidate = std::ffi::OsString::from(".");
        candidate.push(name);
        candidate.push(format!(
            "-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let candidate = parent.join(candidate);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&candidate)
        {
            Ok(file) => break (file, Temporary(candidate)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    };
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&temporary.0, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn file_uri(path: &Path) -> Result<String> {
    if !path.is_absolute() {
        return Err(invalid("Image URI requires an absolute path"));
    }
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            uri.push(char::from(b));
        } else {
            use std::fmt::Write;
            let _ = write!(uri, "%{b:02X}");
        }
    }
    Ok(uri)
}
