//! Pinned Geode loader files and one app-scoped Proton override, with rollback.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

const FILES: [&str; 5] = [
    "Geode.dll",
    "Geode.lib",
    "Geode.pdb",
    "GeodeUpdater.exe",
    "XInput1_4.dll",
];
const UPPERCASE_PROXY: &str = "XINPUT1_4.dll";
const LIMIT: u64 = 160 * 1024 * 1024;
const REG_KEY: &str = r"HKCU\Software\Wine\AppDefaults\GeometryDash.exe\DllOverrides";
const REG_VALUE: &str = "xinput1_4";
const OVERRIDE: &str = "native,builtin";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    version: String,
    url: String,
    sha256: String,
    #[serde(rename = "gdTimestamp")]
    gd_timestamp: u32,
    files: Vec<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Installing,
    Installed,
    Restoring,
    Restored,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManagedFile {
    name: String,
    original_hash: Option<String>,
    original_mode: Option<u32>,
    installed_hash: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Transaction {
    schema: u32,
    geode_version: String,
    game_dir: PathBuf,
    phase: Phase,
    files: Vec<ManagedFile>,
    registry_original: Option<String>,
}

struct Paths {
    state: PathBuf,
    cache: PathBuf,
}

impl Paths {
    fn environment() -> Result<Self> {
        Ok(Self {
            state: state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/geode"),
            cache: state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/geode"),
        })
    }
    fn journal(&self) -> PathBuf {
        self.state.join("transaction.json")
    }
    fn backup(&self) -> PathBuf {
        self.state.join("backup")
    }
}

/// Install, inspect, or restore only the pinned loader and Geometry Dash's override.
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let (action, explicit_game) = arguments(args)?;
    let source = if action == "restore" {
        None
    } else {
        Some(source(root)?)
    };
    let paths = Paths::environment()?;
    // Status does not create a lock, state directory, cache, or transaction.
    let _lock = if action == "status" {
        None
    } else {
        private_directory(&paths.state)?;
        let lock = paths.state.join("lock");
        if let Ok(info) = fs::symlink_metadata(&lock)
            && (!info.is_file() || info.uid() != uid())
        {
            return Err(invalid("Unsafe Geode transaction lock"));
        }
        Some(state::lock(&lock, Duration::from_secs(3))?)
    };
    let mut previous = transaction(&paths)?;
    let game = match explicit_game {
        Some(path) => game_directory(&path)?,
        None if previous.is_some() => game_directory(&previous.as_ref().unwrap().game_dir)?,
        None => discover_game()?,
    };
    if let Some(journal) = &previous
        && journal.game_dir != game
    {
        return Err(invalid(format!(
            "The Geode transaction belongs to {}; restore that installation first",
            journal.game_dir.display()
        )));
    }
    let version = previous
        .as_ref()
        .map(|journal| journal.geode_version.clone())
        .or_else(|| source.as_ref().map(|source| source.version.clone()));
    // Inspection and rollback remain available after Steam updates the executable.
    let timestamp = validate_game(&game)?;
    match action {
        "status" => status(
            &game,
            previous.as_ref(),
            source.as_ref().unwrap(),
            timestamp,
        ),
        "restore" => {
            let Some(journal) = previous.as_mut() else {
                return Ok(summary(
                    "restore",
                    "not-installed",
                    &game,
                    version.as_deref(),
                    false,
                ));
            };
            if journal.phase == Phase::Restored {
                return Ok(summary(
                    "restore",
                    "restored",
                    &game,
                    version.as_deref(),
                    false,
                ));
            }
            game_not_running()?;
            restore(&paths, journal)?;
            Ok(summary(
                "restore",
                "restored",
                &game,
                version.as_deref(),
                true,
            ))
        }
        "install" => {
            let source = source.as_ref().unwrap();
            if timestamp != source.gd_timestamp {
                return Err(invalid(format!(
                    "Geode {} supports GeometryDash.exe PE timestamp {}; found {timestamp}",
                    source.version, source.gd_timestamp
                )));
            }
            game_not_running()?;
            let conflicts = other_proxy_names(&game)?;
            if !conflicts.is_empty() {
                return Err(invalid(format!(
                    "Geometry Dash has an unmanaged case variant of XInput1_4.dll: {}; move it aside before installing",
                    conflicts.join(", ")
                )));
            }
            if let Some(journal) = &previous {
                match journal.phase {
                    Phase::Installed => {
                        if journal.geode_version != source.version {
                            return Err(invalid(format!(
                                "Geode {} is installed, while the manifest pins {}; run geode restore before installing the new pin",
                                journal.geode_version, source.version
                            )));
                        }
                        require_installed(journal)?;
                        return Ok(summary(
                            "install",
                            "installed",
                            &game,
                            Some(&source.version),
                            false,
                        ));
                    }
                    Phase::Installing | Phase::Restoring => {
                        return Err(invalid(
                            "A Geode transaction is unfinished; run geode restore before installing again",
                        ));
                    }
                    Phase::Restored => {}
                }
            }
            install(&paths, &game, source)?;
            Ok(summary(
                "install",
                "installed",
                &game,
                Some(&source.version),
                true,
            ))
        }
        _ => unreachable!(),
    }
}

fn arguments(args: &[String]) -> Result<(&str, Option<PathBuf>)> {
    let mut action = None;
    let mut game = None;
    let mut options = args.iter();
    while let Some(option) = options.next() {
        if option == "--game-dir" {
            let path = options
                .next()
                .ok_or_else(|| invalid("--game-dir requires an absolute directory"))?;
            if game.replace(PathBuf::from(path)).is_some() {
                return Err(invalid("--game-dir may be supplied only once"));
            }
        } else if matches!(option.as_str(), "install" | "status" | "restore") && action.is_none() {
            action = Some(option.as_str());
        } else {
            return Err(invalid(
                "Expected geode install|status|restore [--game-dir ABSOLUTE_DIRECTORY]",
            ));
        }
    }
    Ok((
        action.ok_or_else(|| invalid("Expected geode install, status, or restore"))?,
        game,
    ))
}

fn valid_version(version: &str) -> bool {
    let Some(version) = version.strip_prefix('v') else {
        return false;
    };
    let parts = version.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 10 && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn source(root: &Path) -> Result<Source> {
    let source: Source = serde_json::from_slice(&small_file(
        &root.join("apps/geode/sources.json"),
        16 * 1024,
        false,
    )?)?;
    if !valid_version(&source.version)
        || source.url
            != format!(
                "https://github.com/geode-sdk/geode/releases/download/{0}/geode-{0}-win.zip",
                source.version
            )
        || !valid_hash(&source.sha256)
        || source.gd_timestamp == 0
        || source
            .files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != FILES.into_iter().collect()
        || source.files.len() != FILES.len()
    {
        return Err(invalid(
            "Invalid Geode source pin: expected an official tagged Windows ZIP, SHA-256, game timestamp, and the five loader files",
        ));
    }
    Ok(source)
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .filter(|text| !text.contains(['\n', '\r', '\0']))
        .ok_or_else(|| invalid("Geode paths must be UTF-8 without line breaks"))
}

fn game_directory(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(invalid("The Geometry Dash directory must be absolute"));
    }
    path_text(path)?;
    let path = fs::canonicalize(path)?;
    path_text(&path)?;
    if !fs::metadata(&path)?.is_dir() {
        return Err(invalid("The Geometry Dash path must be a host directory"));
    }
    Ok(path)
}

fn other_proxy_names(game: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(game)? {
        let entry = entry?;
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes.eq_ignore_ascii_case(b"xinput1_4.dll")
            && bytes != b"XInput1_4.dll"
            && bytes != b"XINPUT1_4.dll"
        {
            names.push(name.to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn checked_cli(args: &[String], timeout: Duration) -> Result<process::Output> {
    process::checked(args, None, timeout, true).map_err(|error| match error {
        crate::Error::Process { command, detail }
            if detail.contains("No such file or directory") =>
        {
            invalid(format!(
                "Required Geode tool {command} is missing; install {command} and retry"
            ))
        }
        error => error,
    })
}

fn prefix(command: &str) -> Result<process::Output> {
    process::run(
        &[
            "env".into(),
            "-u".into(),
            "DISPLAY".into(),
            "-u".into(),
            "WAYLAND_DISPLAY".into(),
            "-u".into(),
            "STEAM_COMPAT_DATA_PATH".into(),
            "-u".into(),
            "WINEPREFIX".into(),
            "-u".into(),
            "WINE".into(),
            "-u".into(),
            "WINESERVER".into(),
            "-u".into(),
            "WINELOADER".into(),
            "-u".into(),
            "PROTON_VERSION".into(),
            "WINEDEBUG=-all".into(),
            "LC_ALL=C".into(),
            "protontricks".into(),
            "--no-background-wineserver".into(),
            "-c".into(),
            command.into(),
            "322170".into(),
        ],
        None,
        Duration::from_secs(60),
        true,
    )
}

fn prefix_success(command: &str) -> Result<process::Output> {
    let output = prefix(command)?;
    if !output.status.success() {
        return Err(prefix_error(&output));
    }
    Ok(output)
}

fn prefix_error(output: &process::Output) -> crate::Error {
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let detail = detail.trim().chars().take(4096).collect::<String>();
    invalid(format!(
        "Protontricks could not access Steam Geometry Dash (app 322170): {}{}",
        if detail.is_empty() {
            output.status.to_string()
        } else {
            detail
        },
        if output.status.code() == Some(127) {
            "; install protontricks and its Wine/Steam runtime dependencies"
        } else {
            ""
        }
    ))
}

fn discover_game() -> Result<PathBuf> {
    let output = prefix_success(r#"printf '%s\n' "$STEAM_APP_PATH""#)?;
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Protontricks returned a non-UTF-8 game directory"))?;
    let lines = text
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(invalid(
            "Protontricks did not return one host game directory",
        ));
    }
    game_directory(Path::new(lines[0]))
}

fn validate_game(game: &Path) -> Result<u32> {
    let mut exe = regular_file(&game.join("GeometryDash.exe"), false)?;
    let length = exe.metadata()?.len();
    let mut dos = [0_u8; 64];
    exe.read_exact(&mut dos)?;
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if &dos[..2] != b"MZ" || offset < 64 || offset + 26 > length {
        return Err(invalid("GeometryDash.exe has an invalid PE header"));
    }
    exe.seek(SeekFrom::Start(offset))?;
    let mut pe = [0_u8; 26];
    exe.read_exact(&mut pe)?;
    if &pe[..4] != b"PE\0\0"
        || u16::from_le_bytes(pe[4..6].try_into().unwrap()) != 0x8664
        || u16::from_le_bytes(pe[24..26].try_into().unwrap()) != 0x020b
    {
        return Err(invalid(
            "Geode requires the 64-bit Windows Geometry Dash executable",
        ));
    }
    let timestamp = u32::from_le_bytes(pe[8..12].try_into().unwrap());
    Ok(timestamp)
}

fn game_name(bytes: &[u8]) -> bool {
    let name = bytes
        .rsplit(|byte| matches!(byte, b'/' | b'\\'))
        .next()
        .unwrap_or(bytes);
    name.eq_ignore_ascii_case(b"GeometryDash.exe") || name.eq_ignore_ascii_case(b"GeometryDash.ex")
}

fn game_not_running() -> Result<()> {
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        let directory = entry.path();
        let info = match fs::metadata(&directory) {
            Ok(info) => info,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if info.uid() != uid() {
            continue;
        }
        let comm = match fs::read(directory.join("comm")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if game_name(comm.strip_suffix(b"\n").unwrap_or(&comm)) {
            return Err(invalid(
                "Close Geometry Dash before changing its Geode installation",
            ));
        }
        let mut bytes = Vec::new();
        match File::open(directory.join("cmdline")) {
            Ok(file) => {
                file.take(4096).read_to_end(&mut bytes)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        if bytes.split(|byte| *byte == 0).take(2).any(game_name) {
            return Err(invalid(
                "Close Geometry Dash before changing its Geode installation",
            ));
        }
    }
    Ok(())
}

fn uid() -> u32 {
    // SAFETY: geteuid takes no arguments and has no failure mode.
    unsafe { libc::geteuid() }
}

fn private_directory(path: &Path) -> Result<()> {
    if let Ok(info) = fs::symlink_metadata(path)
        && (!info.is_dir() || info.uid() != uid())
    {
        return Err(invalid(format!(
            "Unsafe Geode private directory: {}",
            path.display()
        )));
    }
    let mut missing = Vec::new();
    for directory in path.ancestors() {
        match fs::symlink_metadata(directory) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing.push(directory),
            Err(error) => return Err(error.into()),
        }
    }
    state::private_directory(path)?;
    // A durable journal also needs the newly created directory entries leading
    // to it, including the separate backup directory, to survive a power loss.
    sync_directory(path)?;
    for directory in missing {
        sync_directory(directory)?;
        if let Some(parent) = directory.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

fn regular_file(path: &Path, private: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let info = file.metadata()?;
    if !info.is_file()
        || private && (info.uid() != uid() || info.mode() & 0o077 != 0 || info.nlink() != 1)
    {
        return Err(invalid(format!("Unsafe Geode file: {}", path.display())));
    }
    Ok(file)
}

fn small_file(path: &Path, limit: u64, private: bool) -> Result<Vec<u8>> {
    let file = regular_file(path, private)?;
    if file.metadata()?.len() > limit {
        return Err(invalid("Geode metadata exceeds its size limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Geode metadata grew beyond its size limit"));
    }
    Ok(bytes)
}

struct Fingerprint {
    hash: String,
    mode: u32,
    length: u64,
}

fn fingerprint(path: &Path, private: bool) -> Result<Option<Fingerprint>> {
    let file = match regular_file(path, private) {
        Ok(file) => file,
        Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let info = file.metadata()?;
    if info.len() > LIMIT {
        return Err(invalid(format!(
            "Geode file exceeds 160 MiB: {}",
            path.display()
        )));
    }
    let mut hash = Sha256::new();
    let mut length = 0;
    let mut buffer = [0_u8; 64 * 1024];
    let mut file = file;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length += count as u64;
        if length > LIMIT {
            return Err(invalid("Geode file grew beyond 160 MiB"));
        }
        hash.update(&buffer[..count]);
    }
    Ok(Some(Fingerprint {
        hash: format!("{:x}", hash.finalize()),
        mode: info.mode() & 0o777,
        length,
    }))
}

fn current_hash(path: &Path) -> Result<Option<String>> {
    Ok(fingerprint(path, false)?.map(|file| file.hash))
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn transaction(paths: &Paths) -> Result<Option<Transaction>> {
    if !paths.state.exists() {
        return Ok(None);
    }
    let info = fs::symlink_metadata(&paths.state)?;
    if !info.is_dir() || info.uid() != uid() || info.mode() & 0o077 != 0 {
        return Err(invalid("Unsafe Geode transaction directory"));
    }
    let bytes = match small_file(&paths.journal(), 64 * 1024, true) {
        Ok(bytes) => bytes,
        Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let journal: Transaction = serde_json::from_slice(&bytes)?;
    let names = journal
        .files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = FILES.into_iter().collect::<BTreeSet<_>>();
    if journal.schema != 1
        || !valid_version(&journal.geode_version)
        || !journal.game_dir.is_absolute()
        || names.len() != journal.files.len()
        || !expected.is_subset(&names)
        || names
            .iter()
            .any(|name| !expected.contains(name) && *name != UPPERCASE_PROXY)
        || journal.files.iter().any(|file| {
            file.original_hash.is_some() != file.original_mode.is_some()
                || file.original_mode.is_some_and(|mode| mode > 0o777)
                || file
                    .original_hash
                    .as_deref()
                    .is_some_and(|hash| !valid_hash(hash))
                || file
                    .installed_hash
                    .as_deref()
                    .is_some_and(|hash| !valid_hash(hash))
                || (file.name == UPPERCASE_PROXY) != file.installed_hash.is_none()
                || file.name == UPPERCASE_PROXY && file.original_hash.is_none()
        })
        || journal
            .registry_original
            .as_deref()
            .is_some_and(|value| value.len() > 4096 || value.contains(['\0', '\n', '\r']))
    {
        return Err(invalid("Invalid Geode transaction journal"));
    }
    path_text(&journal.game_dir)?;
    Ok(Some(journal))
}

fn save(paths: &Paths, journal: &Transaction) -> Result<()> {
    state::atomic(
        &paths.journal(),
        &serde_json::to_vec_pretty(journal)?,
        0o600,
    )
}

struct Temporary(PathBuf, bool);
impl Drop for Temporary {
    fn drop(&mut self) {
        if self.1 {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}

fn temporary(parent: &Path, directory: bool) -> Result<Temporary> {
    loop {
        let path = parent.join(format!(
            ".geode-{}-{}{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
            if directory { ".stage" } else { ".tmp" }
        ));
        let created = if directory {
            fs::DirBuilder::new().mode(0o700).create(&path)
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map(|_| ())
        };
        match created {
            Ok(()) => return Ok(Temporary(path, directory)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn sync_directory(directory: &Path) -> Result<()> {
    File::open(directory)?.sync_all()?;
    Ok(())
}

/// Stream a checked file to a fresh sibling and publish only after the hash matches.
fn copy_publish(
    source: &Path,
    target: &Path,
    expected_hash: &str,
    mode: u32,
    expected_target: Option<&Option<String>>,
) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Geode copy requires a parent directory"))?;
    let temporary = temporary(parent, false)?;
    let mut input = regular_file(source, false)?;
    let mut output = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary.0)?;
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length += count as u64;
        if length > LIMIT {
            return Err(invalid("Geode copy grew beyond 160 MiB"));
        }
        output.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected_hash {
        return Err(invalid(
            "A Geode source file changed while copying; transaction remains recoverable",
        ));
    }
    output.set_permissions(fs::Permissions::from_mode(mode))?;
    output.sync_all()?;
    if let Some(expected) = expected_target
        && &current_hash(target)? != expected
    {
        return Err(changed(target));
    }
    fs::rename(&temporary.0, target)?;
    sync_directory(parent)
}

fn archive(paths: &Paths, source: &Source) -> Result<PathBuf> {
    private_directory(&paths.cache)?;
    let archive = paths
        .cache
        .join(format!("geode-{}-win.zip", source.version));
    // A previously fetched public release may predate this private cache.
    match regular_file(&archive, false) {
        Ok(file) => {
            let info = file.metadata()?;
            if info.uid() != uid() || info.nlink() != 1 {
                return Err(invalid("Unsafe cached Geode ZIP"));
            }
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if fingerprint(&archive, true)?.is_some_and(|file| file.hash == source.sha256) {
        return Ok(archive);
    }
    let download = temporary(&paths.cache, false)?;
    checked_cli(
        &[
            "curl".into(),
            "--fail".into(),
            "--location".into(),
            "--silent".into(),
            "--show-error".into(),
            "--proto".into(),
            "=https".into(),
            "--proto-redir".into(),
            "=https".into(),
            "--connect-timeout".into(),
            "15".into(),
            "--max-time".into(),
            "115".into(),
            "--max-filesize".into(),
            LIMIT.to_string(),
            "--output".into(),
            path_text(&download.0)?.into(),
            source.url.clone(),
        ],
        Duration::from_secs(120),
    )?;
    if !fingerprint(&download.0, true)?.is_some_and(|file| file.hash == source.sha256) {
        return Err(invalid("Downloaded Geode ZIP failed SHA-256 verification"));
    }
    File::open(&download.0)?.sync_all()?;
    fs::rename(&download.0, &archive)?;
    sync_directory(&paths.cache)?;
    Ok(archive)
}

fn extract(paths: &Paths, source: &Source) -> Result<Temporary> {
    let archive = archive(paths, source)?;
    let output = checked_cli(
        &["bsdtar".into(), "-tf".into(), path_text(&archive)?.into()],
        Duration::from_secs(60),
    )?;
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Invalid Geode ZIP member names"))?;
    let names = text.lines().collect::<Vec<_>>();
    if names.len() != FILES.len()
        || names.iter().copied().collect::<BTreeSet<_>>() != FILES.into_iter().collect()
    {
        return Err(invalid(
            "Geode ZIP must contain exactly the five expected flat files",
        ));
    }
    let stage = temporary(&paths.cache, true)?;
    checked_cli(
        &[
            "bsdtar".into(),
            "-xf".into(),
            path_text(&archive)?.into(),
            "-C".into(),
            path_text(&stage.0)?.into(),
            "--no-same-owner".into(),
            "--no-same-permissions".into(),
        ],
        Duration::from_secs(60),
    )?;
    let entries = fs::read_dir(&stage.0)?.collect::<std::io::Result<Vec<_>>>()?;
    if entries.len() != FILES.len() {
        return Err(invalid("Unexpected extracted Geode files"));
    }
    let mut length = 0_u64;
    for name in FILES {
        let file = regular_file(&stage.0.join(name), false)?;
        let info = file.metadata()?;
        if info.nlink() != 1 || info.uid() != uid() {
            return Err(invalid("Geode ZIP members must be ordinary owned files"));
        }
        length = length
            .checked_add(info.len())
            .ok_or_else(|| invalid("Geode ZIP size overflow"))?;
    }
    if length > LIMIT {
        return Err(invalid("Extracted Geode files exceed 160 MiB"));
    }
    Ok(stage)
}

fn registry_query() -> Result<Option<String>> {
    let output = prefix(&format!(
        "wine reg query {} /v {REG_VALUE}",
        shell_words::quote(REG_KEY)
    ))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        if output.status.code() == Some(1)
            && text
                .to_ascii_lowercase()
                .contains("unable to find the specified registry key")
        {
            return Ok(None);
        }
        return Err(prefix_error(&output));
    }
    let text =
        std::str::from_utf8(&output.stdout).map_err(|_| invalid("Invalid Wine registry output"))?;
    let mut value = None;
    for line in text.lines() {
        let line = line.trim_start();
        let Some((name, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if !name.eq_ignore_ascii_case(REG_VALUE) {
            continue;
        }
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix("REG_SZ") else {
            return Err(invalid(
                "The existing Geometry Dash xinput1_4 override is not REG_SZ",
            ));
        };
        if value.is_some() {
            return Err(invalid("Ambiguous Wine registry override output"));
        }
        // Wine's four-space field separator precedes the string itself. Keep
        // whitespace inside an existing value intact for an exact rollback.
        let found = rest
            .strip_prefix("    ")
            .ok_or_else(|| invalid("Unrecognized Wine registry field separator"))?;
        if found.len() > 4096 || found.contains(['\0', '\n', '\r']) {
            return Err(invalid("Invalid prior Geometry Dash DLL override"));
        }
        value = Some(found.to_owned());
    }
    value
        .map(Some)
        .ok_or_else(|| invalid("Wine did not return the requested registry value"))
}

fn registry_set(value: Option<&str>) -> Result<()> {
    let command = match value {
        Some(value) => format!(
            "wine reg add {} /v {REG_VALUE} /t REG_SZ /d {} /f",
            shell_words::quote(REG_KEY),
            shell_words::quote(value)
        ),
        None => format!(
            "wine reg delete {} /v {REG_VALUE} /f",
            shell_words::quote(REG_KEY)
        ),
    };
    prefix_success(&command)?;
    if registry_query()?.as_deref() != value {
        return Err(invalid(
            "The app-scoped Geode registry override did not verify",
        ));
    }
    Ok(())
}

fn changed(path: &Path) -> crate::Error {
    invalid(format!(
        "Geode loader file changed outside this transaction (possibly a Geode update): {}; refusing to overwrite it",
        path.display()
    ))
}

fn install(paths: &Paths, game: &Path, source: &Source) -> Result<()> {
    if let (Ok(mixed), Ok(uppercase)) = (
        fs::symlink_metadata(game.join("XInput1_4.dll")),
        fs::symlink_metadata(game.join(UPPERCASE_PROXY)),
    ) && mixed.dev() == uppercase.dev()
        && mixed.ino() == uppercase.ino()
    {
        return Err(invalid(
            "The two XInput1_4.dll spellings alias the same file; use a case-sensitive game directory",
        ));
    }
    let stage = extract(paths, source)?;
    private_directory(&paths.backup())?;
    let registry_original = registry_query()?;
    let mut files = Vec::new();
    let mut original_length = 0_u64;
    for name in FILES.into_iter().chain(std::iter::once(UPPERCASE_PROXY)) {
        let original = fingerprint(&game.join(name), false)?;
        if name == UPPERCASE_PROXY && original.is_none() {
            continue;
        }
        if let Some(original) = &original {
            original_length = original_length
                .checked_add(original.length)
                .ok_or_else(|| invalid("Existing Geode file size overflow"))?;
            if original_length > LIMIT {
                return Err(invalid(
                    "Existing loader files exceed the 160 MiB backup limit",
                ));
            }
            copy_publish(
                &game.join(name),
                &paths.backup().join(name),
                &original.hash,
                0o600,
                None,
            )?;
        }
        files.push(ManagedFile {
            name: name.into(),
            original_hash: original.as_ref().map(|file| file.hash.clone()),
            original_mode: original.as_ref().map(|file| file.mode),
            installed_hash: if name == UPPERCASE_PROXY {
                None
            } else {
                Some(
                    fingerprint(&stage.0.join(name), false)?
                        .ok_or_else(|| invalid("Missing staged Geode file"))?
                        .hash,
                )
            },
        });
    }
    let mut journal = Transaction {
        schema: 1,
        geode_version: source.version.clone(),
        game_dir: game.to_path_buf(),
        phase: Phase::Installing,
        files,
        registry_original,
    };
    // Every backup and the complete ownership plan are durable before game/prefix writes.
    save(paths, &journal)?;
    let result = (|| {
        game_not_running()?;
        for file in &journal.files {
            let target = game.join(&file.name);
            if current_hash(&target)? != file.original_hash {
                return Err(changed(&target));
            }
            if let Some(hash) = &file.installed_hash {
                copy_publish(
                    &stage.0.join(&file.name),
                    &target,
                    hash,
                    0o644,
                    Some(&file.original_hash),
                )?;
            } else {
                fs::remove_file(&target)?;
                sync_directory(game)?;
            }
        }
        let current = registry_query()?;
        if current != journal.registry_original {
            return Err(invalid(
                "Geometry Dash's DLL override changed while installing; refusing to overwrite it",
            ));
        }
        if current.as_deref() != Some(OVERRIDE) {
            registry_set(Some(OVERRIDE))?;
        }
        journal.phase = Phase::Installed;
        save(paths, &journal)
    })();
    result.map_err(|error| {
        invalid(format!(
            "{error}; run geode restore to recover the recorded transaction"
        ))
    })
}

fn require_installed(journal: &Transaction) -> Result<()> {
    for file in &journal.files {
        let path = journal.game_dir.join(&file.name);
        if current_hash(&path)? != file.installed_hash {
            return Err(changed(&path));
        }
    }
    if registry_query()?.as_deref() != Some(OVERRIDE) {
        return Err(invalid(
            "Geometry Dash's app-scoped Geode DLL override changed; refusing to overwrite it",
        ));
    }
    Ok(())
}

fn restore(paths: &Paths, journal: &mut Transaction) -> Result<()> {
    let registry = registry_query()?;
    if registry != journal.registry_original && registry.as_deref() != Some(OVERRIDE) {
        return Err(invalid(
            "Geometry Dash's DLL override changed outside this transaction; refusing to restore it",
        ));
    }
    // Check the complete rollback before changing any file or the registry.
    for file in &journal.files {
        let path = journal.game_dir.join(&file.name);
        let current = current_hash(&path)?;
        if current != file.original_hash && current != file.installed_hash {
            return Err(changed(&path));
        }
        if let Some(original) = &file.original_hash
            && !fingerprint(&paths.backup().join(&file.name), true)?
                .is_some_and(|backup| backup.hash == *original)
        {
            return Err(invalid(format!(
                "Geode rollback backup failed verification: {}",
                file.name
            )));
        }
    }
    journal.phase = Phase::Restoring;
    save(paths, journal)?;
    game_not_running()?;
    for file in &journal.files {
        let target = journal.game_dir.join(&file.name);
        let current_file = fingerprint(&target, false)?;
        let current = current_file.as_ref().map(|file| file.hash.clone());
        if current == file.original_hash
            && current_file.as_ref().map(|file| file.mode) == file.original_mode
        {
            continue;
        }
        if current != file.installed_hash && current != file.original_hash {
            return Err(changed(&target));
        }
        if let Some(original) = &file.original_hash {
            copy_publish(
                &paths.backup().join(&file.name),
                &target,
                original,
                file.original_mode.unwrap(),
                Some(&current),
            )?;
        } else {
            // Only an exact installed hash reaches this deletion.
            fs::remove_file(&target)?;
            sync_directory(&journal.game_dir)?;
        }
    }
    let registry = registry_query()?;
    if registry != journal.registry_original {
        if registry.as_deref() != Some(OVERRIDE) {
            return Err(invalid(
                "Geometry Dash's DLL override changed while restoring; refusing to overwrite it",
            ));
        }
        registry_set(journal.registry_original.as_deref())?;
    }
    journal.phase = Phase::Restored;
    save(paths, journal)
}

fn summary(action: &str, status: &str, game: &Path, version: Option<&str>, changed: bool) -> Value {
    json!({"action": action, "status": status, "version": version, "gameDir": game, "changed": changed})
}

fn status(
    game: &Path,
    journal: Option<&Transaction>,
    source: &Source,
    timestamp: u32,
) -> Result<Value> {
    let Some(journal) = journal else {
        let mut result = summary(
            "status",
            "not-installed",
            game,
            Some(&source.version),
            false,
        );
        result["supportedGameBuild"] = json!(timestamp == source.gd_timestamp);
        result["gameTimestamp"] = json!(timestamp);
        return Ok(result);
    };
    if journal.phase == Phase::Restored {
        let mut result = summary(
            "status",
            "restored",
            game,
            Some(&journal.geode_version),
            false,
        );
        result["supportedGameBuild"] = json!(timestamp == source.gd_timestamp);
        result["gameTimestamp"] = json!(timestamp);
        return Ok(result);
    }
    let registry = registry_query()?;
    let proxy_conflicts = other_proxy_names(game)?;
    let mut matches = proxy_conflicts.is_empty();
    let mut files = Vec::new();
    for file in &journal.files {
        let current = current_hash(&game.join(&file.name))?;
        let installed = current == file.installed_hash;
        matches &= installed;
        files.push(json!({"name": file.name, "matchesInstalled": installed, "sha256": current}));
    }
    let registry_matches = registry.as_deref() == Some(OVERRIDE);
    let status = match journal.phase {
        Phase::Installed if matches && registry_matches => "installed",
        Phase::Installed => "modified",
        Phase::Installing | Phase::Restoring => "recovery-required",
        Phase::Restored => unreachable!(),
    };
    Ok(json!({
        "action": "status", "status": status, "version": journal.geode_version, "gameDir": game,
        "pinnedVersion": source.version, "matchesPinnedVersion": journal.geode_version == source.version,
        "supportedGameBuild": timestamp == source.gd_timestamp, "gameTimestamp": timestamp,
        "proxyConflicts": proxy_conflicts,
        "changed": false, "transactionPhase": journal.phase,
        "registryOverride": registry, "registryMatchesInstalled": registry_matches, "files": files
    }))
}
