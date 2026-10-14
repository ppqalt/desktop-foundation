//! Narrow, root-owned publication of one browser theme policy.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

pub const HELPER: &str = "/usr/local/libexec/desktop-foundation-browser-policy";
pub const POLICY: &str = "/etc/brave/policies/managed/70-desktop-foundation-theme.json";
pub const OWNER: &str = "/etc/desktop-foundation/browser-policy.json";
const DIRECTORY: &str = "/var/lib/desktop-foundation/browser-policy";
const LIMIT: u64 = 128 * 1024;
const WRAPPER: &str = "/usr/bin/brave-origin-nightly";
const BROWSER: &str = "/opt/brave.com/brave-origin-nightly/brave";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    version: u32,
    uid: u32,
    user: String,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
enum Change {
    Apply { bytes: Vec<u8> },
    Remove,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    original: Option<Vec<u8>>,
    original_mode: Option<u32>,
    managed: Option<Vec<u8>>,
    pending: Option<Change>,
    created_directories: Vec<String>,
}

fn secure_parents(path: &Path) -> Result<()> {
    for parent in path.ancestors().skip(1) {
        let info = fs::symlink_metadata(parent)?;
        if !info.is_dir() || info.uid() != 0 || info.mode() & 0o022 != 0 {
            return Err(invalid(format!(
                "Unsafe browser policy directory: {}",
                parent.display()
            )));
        }
    }
    Ok(())
}

fn read_root(path: &Path, private: bool) -> Result<Vec<u8>> {
    secure_parents(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let info = file.metadata()?;
    if !info.is_file()
        || info.uid() != 0
        || info.mode() & 0o022 != 0
        || private && info.mode() & 0o077 != 0
        || info.len() > LIMIT
    {
        return Err(invalid(format!(
            "Unsafe browser policy file: {}",
            path.display()
        )));
    }
    let mut contents = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut contents)?;
    if contents.len() as u64 > LIMIT {
        return Err(invalid("Browser policy file grew beyond its limit"));
    }
    Ok(contents)
}

fn optional_policy() -> Result<Option<Vec<u8>>> {
    secure_parents(Path::new(POLICY))?;
    match fs::symlink_metadata(POLICY) {
        Ok(_) => Ok(Some(read_root(Path::new(POLICY), false)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn owner() -> Result<Owner> {
    let owner: Owner = serde_json::from_slice(&read_root(Path::new(OWNER), false)?)?;
    if owner.version != 1
        || owner.uid == 0
        || owner.user.is_empty()
        || owner.user.len() > 64
        || owner.user.contains(['\n', '\r', '\0'])
    {
        return Err(invalid("Invalid browser theme owner marker"));
    }
    let name = std::ffi::CString::new(owner.user.as_bytes())
        .map_err(|_| invalid("Invalid browser theme owner name"))?;
    let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0_u8; 64 * 1024];
    // SAFETY: CString, passwd output and bounded scratch buffer remain alive
    // for getpwnam_r; a nonnull success result initializes the output record.
    let code = unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code != 0 || result.is_null() || unsafe { record.assume_init() }.pw_uid != owner.uid {
        return Err(invalid(
            "Browser theme owner no longer matches the local account",
        ));
    }
    Ok(owner)
}

fn color(text: &str) -> Result<String> {
    if text.len() != 7
        || !text.starts_with('#')
        || !text.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(invalid("Browser policy color must be #RRGGBB"));
    }
    Ok(text.to_ascii_lowercase())
}

fn policy_color(bytes: &[u8]) -> Result<String> {
    if bytes.len() > 4096 {
        return Err(invalid("The owned browser color policy exceeds 4 KiB"));
    }
    let parsed: Value = serde_json::from_slice(bytes)?;
    let object = parsed
        .as_object()
        .ok_or_else(|| invalid("Browser theme policy must be an object"))?;
    if object.len() != 1 {
        return Err(invalid(
            "The owned policy file may contain only BrowserThemeColor",
        ));
    }
    color(
        object
            .get("BrowserThemeColor")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("BrowserThemeColor must be a color string"))?,
    )
}

fn journal_path() -> PathBuf {
    Path::new(DIRECTORY).join("policy.json")
}

fn journal() -> Result<Journal> {
    let journal: Journal = serde_json::from_slice(&read_root(&journal_path(), true)?)?;
    if journal.version != 1
        || journal.created_directories.len() > 3
        || journal.created_directories.iter().any(|path| {
            !matches!(
                path.as_str(),
                "/etc/brave" | "/etc/brave/policies" | "/etc/brave/policies/managed"
            )
        })
        || journal.original.is_some() != journal.original_mode.is_some()
        || journal
            .original_mode
            .is_some_and(|mode| mode & !0o777 != 0 || mode & 0o022 != 0)
    {
        return Err(invalid("Invalid browser policy rollback journal"));
    }
    for bytes in [&journal.original, &journal.managed].into_iter().flatten() {
        policy_color(bytes)?;
    }
    if let Some(Change::Apply { bytes }) = &journal.pending {
        policy_color(bytes)?;
    }
    let current = optional_policy()?;
    let desired = match &journal.pending {
        Some(Change::Apply { bytes }) => Some(Some(bytes.clone())),
        Some(Change::Remove) => Some(journal.original.clone()),
        None => None,
    };
    if current != journal.managed
        && current != journal.original
        && desired.as_ref() != Some(&current)
    {
        return Err(invalid(
            "Browser theme policy changed outside Desktop Foundation; refusing overwrite",
        ));
    }
    Ok(journal)
}

fn save(journal: &Journal) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(journal)?;
    bytes.push(b'\n');
    state::atomic(&journal_path(), &bytes, 0o600)
}

fn lock() -> Result<File> {
    let path = Path::new(DIRECTORY).join("lock");
    secure_parents(&path)?;
    if fs::symlink_metadata(DIRECTORY)?.mode() & 0o777 != 0o700 {
        return Err(invalid("Browser policy state must remain private"));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let info = file.metadata()?;
    if !info.is_file() || info.uid() != 0 || info.mode() & 0o777 != 0o600 {
        return Err(invalid("Unsafe browser policy transaction lock"));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(invalid("Browser policy transaction busy; retry shortly"));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

fn authorize(owner: &Owner) -> Result<()> {
    // Root can install/restore directly. pkexec must identify the exact desktop owner.
    if unsafe { libc::geteuid() } != 0 {
        return Err(invalid(
            "Browser policy updates require the installed privileged helper",
        ));
    }
    if let Some(value) = env::var_os("PKEXEC_UID")
        && value.to_str().and_then(|s| s.parse::<u32>().ok()) != Some(owner.uid)
    {
        return Err(invalid(
            "This user does not own Desktop Foundation's browser theme",
        ));
    }
    Ok(())
}

/// Status is public and read-only; it contains no browser profile information.
pub fn status() -> Result<Value> {
    match fs::symlink_metadata(OWNER) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"enabled":false}));
        }
        Err(error) => return Err(error.into()),
        Ok(_) => (),
    }
    verify_installed_helper()?;
    let owner = owner()?;
    let current = optional_policy()?;
    let current_color = current.as_deref().map(policy_color).transpose()?;
    if unsafe { libc::geteuid() } == 0 {
        let _lock = lock()?;
        journal()?;
    }
    Ok(
        json!({"enabled":true,"ownerUid":owner.uid,"owner":owner.user,
        "policy":POLICY,"color":current_color}),
    )
}

fn verify_installed_helper() -> Result<()> {
    secure_parents(Path::new(HELPER))?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(HELPER)?;
    let info = file.metadata()?;
    if !info.is_file()
        || info.uid() != 0
        || info.mode() & 0o6022 != 0
        || info.mode() & 0o111 == 0
        || info.len() > 32 * 1024 * 1024
    {
        return Err(invalid("Unsafe installed browser theme helper"));
    }
    let mut header = [0_u8; 4];
    file.read_exact(&mut header)?;
    if header != *b"\x7fELF" {
        return Err(invalid("The browser theme helper must be native"));
    }
    Ok(())
}

pub fn enabled() -> Result<bool> {
    let status = status()?;
    if status["enabled"] != true {
        return Ok(false);
    }
    if status["ownerUid"].as_u64() != Some(u64::from(unsafe { libc::getuid() })) {
        return Err(invalid("This desktop user does not own browser theming"));
    }
    Ok(true)
}

fn bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(invalid("Process metadata exceeded its bound"));
    }
    Ok(bytes)
}

fn browser_root() -> Result<PathBuf> {
    Ok(state::xdg("XDG_CONFIG_HOME", ".config")?.join("BraveSoftware/Brave-Origin-Nightly"))
}

fn main_process(root: &Path) -> Result<Option<u32>> {
    let expected = fs::canonicalize(root)?;
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let path = entry.path();
        let probe = || -> Result<bool> {
            if fs::metadata(&path)?.uid() != unsafe { libc::getuid() }
                || fs::read_link(path.join("exe"))? != Path::new(BROWSER)
            {
                return Ok(false);
            }
            let bytes = bounded(&path.join("cmdline"), LIMIT)?;
            let args: Vec<_> = bytes
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect();
            // Chromium may rewrite its entire process title into argv[0].
            if args.iter().any(|s| s.contains("--type=")) {
                return Ok(false);
            }
            let mut selected = None;
            for (index, argument) in args.iter().enumerate() {
                if let Some(value) = argument.strip_prefix("--user-data-dir=") {
                    selected = Some(PathBuf::from(value));
                } else if argument == "--user-data-dir" {
                    selected = args.get(index + 1).map(PathBuf::from);
                    if selected.is_none() {
                        return Ok(false);
                    }
                } else if argument.contains("--user-data-dir") {
                    return Ok(false); // Ambiguous rewritten path: do not guess.
                }
            }
            if selected.is_none() {
                let environment = bounded(&path.join("environ"), 128 * 1024)?;
                selected = environment.split(|b| *b == 0).find_map(|entry| {
                    entry
                        .strip_prefix(b"CHROME_USER_DATA_DIR=")
                        .and_then(|value| std::str::from_utf8(value).ok())
                        .map(PathBuf::from)
                });
            }
            Ok(fs::canonicalize(selected.as_deref().unwrap_or(root))
                .ok()
                .as_ref()
                == Some(&expected))
        };
        if probe().unwrap_or(false) {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

fn refresh_running() -> Result<bool> {
    let root = browser_root()?;
    if !root.is_dir() || main_process(&root)?.is_none() {
        return Ok(false);
    }
    // The native singleton client returns without opening a window. A bounded
    // child process group is cleaned up if the existing browser exits during
    // handoff and the request accidentally becomes a cold headless start.
    process::checked(
        &[
            WRAPPER.into(),
            format!("--user-data-dir={}", root.display()),
            "--refresh-platform-policy".into(),
            "--no-startup-window".into(),
        ],
        None,
        Duration::from_secs(8),
        true,
    )?;
    Ok(true)
}

/// Unprivileged runtime entry: policy publication is delegated to the installed helper.
pub fn refresh(_root: &Path, value: &str) -> Result<Value> {
    let value = color(value)?;
    if !enabled()? {
        return Ok(json!({"enabled":false,"refreshed":false}));
    }
    process::checked(
        &[
            "/usr/bin/pkexec".into(),
            HELPER.into(),
            "apply".into(),
            value.clone(),
        ],
        None,
        Duration::from_secs(12),
        true,
    )?;
    let refreshed = refresh_running()?;
    Ok(json!({"enabled":true,"color":value,"refreshed":refreshed,"policy":POLICY}))
}

pub fn remove(_root: &Path) -> Result<Value> {
    if !enabled()? {
        return Ok(json!({"enabled":false,"refreshed":false}));
    }
    process::checked(
        &["/usr/bin/pkexec".into(), HELPER.into(), "remove".into()],
        None,
        Duration::from_secs(12),
        true,
    )?;
    let refreshed = refresh_running()?;
    Ok(json!({"enabled":true,"removed":true,"refreshed":refreshed}))
}

pub fn helper_main(args: &[String]) -> Result<()> {
    if args == ["status"] {
        println!("{}", serde_json::to_string(&status()?)?);
        return Ok(());
    }
    let change = match args {
        [action, value] if action == "apply" => {
            let value = color(value)?;
            let mut bytes = serde_json::to_vec(&json!({"BrowserThemeColor":value}))?;
            bytes.push(b'\n');
            Change::Apply { bytes }
        }
        [action] if action == "remove" => Change::Remove,
        _ => {
            return Err(invalid(
                "Usage: browser-policy apply '#RRGGBB' | remove | status",
            ));
        }
    };
    let _lock = lock()?;
    // Ownership and authorization belong to this locked transaction. A request
    // delayed across restore/reinstall must not retain the previous owner's rights.
    let owner = owner()?;
    authorize(&owner)?;
    let mut journal = journal()?;
    let desired = match &change {
        Change::Apply { bytes } => Some(bytes.clone()),
        Change::Remove => journal.original.clone(),
    };
    // Save intent first so an interrupted atomic publication remains recoverable.
    journal.pending = Some(change);
    save(&journal)?;
    secure_parents(Path::new(POLICY))?;
    if let Some(bytes) = &desired {
        let mode = if matches!(journal.pending, Some(Change::Remove)) {
            journal.original_mode.unwrap_or(0o644)
        } else {
            0o644
        };
        state::atomic(Path::new(POLICY), bytes, mode)?;
    } else {
        match fs::remove_file(POLICY) {
            Ok(()) => File::open(Path::new(POLICY).parent().unwrap())?.sync_all()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    journal.managed = desired;
    journal.pending = None;
    save(&journal)?;
    println!("Browser theme policy updated");
    Ok(())
}
