//! Root-owned, reversible greetd/PAM setup. No running display manager restart.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
const DIRECTORY: &str = "/var/lib/desktop-foundation/system";
const HELPER: &str = "/usr/local/libexec/desktop-foundation-oo7-ready";
const PAM: &str = "/etc/pam.d/greetd";
const CONFIG: &str = "/etc/greetd/config.toml";
const READY_HOOK: &[u8] = b"session    [success=ok ignore=ignore default=1] pam_exec.so quiet type=open_session /usr/local/libexec/desktop-foundation-oo7-ready\n";
const LIMIT: u64 = 16 * 1024 * 1024;
const TARGETS: [(&str, &str); 3] = [
    (
        HELPER,
        "native/foundation/target/release/foundation-oo7-ready",
    ),
    (CONFIG, "session/greetd/config.toml"),
    (PAM, "session/greetd/pam-greetd"),
];
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Entry {
    backup: Option<PathBuf>,
    original: Option<String>,
    managed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
type Journal = BTreeMap<String, Entry>;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bounded(path: &Path, limit: u64) -> Result<(Vec<u8>, fs::Metadata)> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let info = file.metadata()?;
    if !info.is_file() || info.len() > limit {
        return Err(invalid(format!(
            "Expected bounded regular file: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Login file changed beyond its size limit"));
    }
    Ok((bytes, info))
}

/// Production is fixed to / and UID 0. Alternate roots exist only in unit fixtures.
struct Store {
    base: PathBuf,
    uid: u32,
}
impl Store {
    fn path(&self, logical: &str) -> PathBuf {
        self.base.join(logical.trim_start_matches('/'))
    }
    fn parents(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.base) {
            return Err(invalid("Protected path escaped its root"));
        }
        for parent in path.ancestors().skip(1) {
            match fs::symlink_metadata(parent) {
                Ok(info)
                    if !info.is_dir() || info.uid() != self.uid || info.mode() & 0o022 != 0 =>
                {
                    return Err(invalid(format!(
                        "Expected protected owned directory: {}",
                        parent.display()
                    )));
                }
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
            if parent == self.base {
                break;
            }
        }
        Ok(())
    }
    fn read(&self, path: &Path, private: bool, limit: u64) -> Result<(Vec<u8>, u32)> {
        self.parents(path)?;
        let (bytes, info) = bounded(path, limit)?;
        if info.uid() != self.uid
            || info.mode() & 0o6022 != 0
            || private && info.mode() & 0o077 != 0
        {
            return Err(invalid(format!(
                "Unsafe protected login file: {}",
                path.display()
            )));
        }
        Ok((bytes, info.mode() & 0o777))
    }
    fn optional(&self, path: &Path, private: bool, limit: u64) -> Result<Option<(Vec<u8>, u32)>> {
        self.parents(path)?;
        match fs::symlink_metadata(path) {
            Ok(_) => self.read(path, private, limit).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn atomic(&self, path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
        self.parents(path)?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(path.parent().ok_or_else(|| invalid("Missing parent"))?)?;
        self.parents(path)?;
        state::atomic(path, bytes, mode)
    }
    fn lock(&self) -> Result<File> {
        let lock = self.path(&format!("{DIRECTORY}/lock"));
        self.parents(&lock)?;
        if let Some((_, mode)) = self.optional(&lock, true, 4096)?
            && mode != 0o600
        {
            return Err(invalid("Login setup lock must have mode 0600"));
        }
        state::private_directory(lock.parent().expect("lock parent"))?;
        state::lock(&lock, Duration::from_secs(5))
    }
    fn journal_path(&self) -> PathBuf {
        self.path(&format!("{DIRECTORY}/manifest.json"))
    }
    fn save(&self, entries: &Journal) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(entries)?;
        bytes.push(b'\n');
        self.atomic(&self.journal_path(), &bytes, 0o600)
    }
    fn load(&self) -> Result<Journal> {
        let Some((bytes, _)) = self.optional(&self.journal_path(), true, 1024 * 1024)? else {
            return Ok(Journal::new());
        };
        let entries: Journal = serde_json::from_slice(&bytes)?;
        for (name, entry) in &entries {
            if entry.managed.is_none() && entry.pending.is_none() {
                return Err(invalid("Login journal lacks a managed or pending digest"));
            }
            if !TARGETS.iter().any(|(target, _)| target == name) {
                return Err(invalid("Unknown target in login journal"));
            }
            for hash in [&entry.managed, &entry.pending, &entry.original]
                .into_iter()
                .flatten()
            {
                if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(invalid("Invalid login journal digest"));
                }
            }
            if entry.backup.is_some() != entry.original.is_some() {
                return Err(invalid("Incomplete original login backup record"));
            }
            if let Some(backup) = &entry.backup {
                let expected = Path::new(DIRECTORY).join(
                    Path::new(name)
                        .file_name()
                        .ok_or_else(|| invalid("Invalid login target"))?,
                );
                if backup != &expected {
                    return Err(invalid("Login backup escaped its expected path"));
                }
                let (bytes, _) = self.read(
                    &self.path(
                        backup
                            .to_str()
                            .ok_or_else(|| invalid("Invalid backup path"))?,
                    ),
                    false,
                    LIMIT,
                )?;
                if Some(digest(&bytes)) != entry.original {
                    return Err(invalid(format!(
                        "Original login backup changed: {}",
                        backup.display()
                    )));
                }
            }
        }
        Ok(entries)
    }
}
trait Host {
    fn validate(&self, store: &Store) -> Result<()>;
    fn enable(&self) -> Result<()>;
}
struct Native;
impl Host for Native {
    fn validate(&self, store: &Store) -> Result<()> {
        process::checked(
            &[
                "/usr/bin/pacman".into(),
                "-Q".into(),
                "greetd".into(),
                "greetd-tuigreet".into(),
                "oo7".into(),
                "niri".into(),
            ],
            None,
            Duration::from_secs(20),
            true,
        )?;
        if !store.path("/usr/lib/security/pam_oo7.so").is_file()
            || !store
                .path("/usr/share/wayland-sessions/niri.desktop")
                .is_file()
        {
            return Err(invalid(
                "oo7 PAM module or packaged Niri session entry missing; login files unchanged",
            ));
        }
        let output = process::checked(
            &[
                "/usr/bin/tuigreet".into(),
                "--background".into(),
                "matrix".into(),
                "--dump-config".into(),
            ],
            None,
            Duration::from_secs(20),
            true,
        )?;
        if !String::from_utf8_lossy(&output.stdout).contains("kind = \"matrix\"") {
            return Err(invalid(
                "Installed tuigreet lacks Matrix support; login files unchanged",
            ));
        }
        let manager = store.path("/etc/systemd/system/display-manager.service");
        store.parents(&manager)?;
        match fs::symlink_metadata(&manager) {
            Ok(info)
                if info.file_type().is_symlink()
                    && manager
                        .canonicalize()?
                        .file_name()
                        .is_some_and(|name| name == "greetd.service") => {}
            Ok(_) => {
                return Err(invalid(
                    "Another display manager owns login; use --no-greeter. Login files unchanged",
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
    fn enable(&self) -> Result<()> {
        process::checked(
            &[
                "/usr/bin/systemctl".into(),
                "enable".into(),
                "greetd.service".into(),
            ],
            None,
            Duration::from_secs(30),
            true,
        )?;
        Ok(())
    }
}
fn legacy_pam(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut remaining = bytes;
    while let Some(index) = remaining
        .windows(READY_HOOK.len())
        .position(|w| w == READY_HOOK)
    {
        output.extend_from_slice(&remaining[..index]);
        remaining = &remaining[index + READY_HOOK.len()..];
    }
    output.extend_from_slice(remaining);
    output
}
fn accepted(entry: &Entry, bytes: &[u8]) -> bool {
    let hash = digest(bytes);
    [&entry.managed, &entry.pending, &entry.original]
        .into_iter()
        .flatten()
        .any(|v| v == &hash)
}
fn pam_ready(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let lines: Vec<Vec<_>> = text
        .lines()
        .map(|line| {
            line.split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect()
        })
        .collect();
    let auth = lines
        .iter()
        .any(|line| line.first() == Some(&"auth") && line.contains(&"pam_oo7.so"));
    let session: Vec<_> = lines
        .iter()
        .filter(|line| line.first() == Some(&"session"))
        .collect();
    let login = session
        .iter()
        .position(|line| line.contains(&"system-local-login"));
    let ready = session.iter().position(|line| line.contains(&HELPER));
    let unlock = session.iter().position(|line| line.contains(&"pam_oo7.so"));
    match (login, ready, unlock) {
        (Some(login), Some(ready), Some(unlock)) => {
            auth && login < ready
                && unlock == ready + 1
                && session[ready].get(1..4)
                    == Some(&["[success=ok", "ignore=ignore", "default=1]"][..])
                && session[ready].contains(&"pam_exec.so")
                && session[ready].contains(&"type=open_session")
                && !session[ready].contains(&"expose_authtok")
        }
        _ => false,
    }
}
fn sources(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    TARGETS
        .into_iter()
        .map(|(name, file)| {
            let (bytes, _) = bounded(
                &root.join(file),
                if name == HELPER { LIMIT } else { 64 * 1024 },
            )?;
            if name == HELPER && !bytes.starts_with(b"\x7fELF") {
                return Err(invalid(
                    "Build scripts/build-backend: readiness helper must be a native ELF executable",
                ));
            }
            if name == PAM && !pam_ready(&bytes) {
                return Err(invalid(
                    "PAM template is missing the ordered oo7 readiness/unlock hooks",
                ));
            }
            if name == CONFIG {
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| invalid("greetd template is not UTF-8"))?;
                let config: toml::Value = toml::from_str(text)
                    .map_err(|e| invalid(format!("Invalid greetd template: {e}")))?;
                for field in ["command", "user"] {
                    if config
                        .get("default_session")
                        .and_then(|s| s.get(field))
                        .and_then(toml::Value::as_str)
                        .is_none_or(str::is_empty)
                    {
                        return Err(invalid(
                            "greetd template needs a default session command and user",
                        ));
                    }
                }
            }
            Ok((name.into(), bytes))
        })
        .collect()
}
fn perform(root: &Path, action: &str, store: &Store, host: &impl Host) -> Result<()> {
    let mut entries = store.load()?;
    if action == "install" {
        let sources = sources(root)?;
        host.validate(store)?;
        // Preflight every target and backup before publishing any login file.
        for (name, content) in &sources {
            let current = store.optional(&store.path(name), false, LIMIT)?;
            if let Some(entry) = entries.get(name) {
                match current {
                    None if entry.backup.is_none() => (),
                    Some((ref bytes, _))
                        if accepted(entry, bytes)
                            || bytes == content
                            || name == PAM && digest(bytes) == digest(&legacy_pam(content)) => {}
                    _ => return Err(invalid(format!("Modified outside deployment: {name}"))),
                }
            }
        }
        // Install the helper before PAM can begin referring to it.
        let mut ordered: Vec<_> = sources.into_iter().collect();
        ordered.sort_by_key(|(name, _)| name != HELPER);
        for (name, content) in ordered {
            let target = store.path(&name);
            if !entries.contains_key(&name) {
                let original = store.optional(&target, false, LIMIT)?;
                let backup = original.as_ref().map(|_| {
                    Path::new(DIRECTORY).join(target.file_name().expect("target filename"))
                });
                if let Some((bytes, mode)) = &original {
                    store.atomic(
                        &store.path(
                            backup
                                .as_ref()
                                .expect("backup")
                                .to_str()
                                .expect("UTF-8 backup"),
                        ),
                        bytes,
                        *mode,
                    )?;
                }
                entries.insert(
                    name.clone(),
                    Entry {
                        backup,
                        original: original.as_ref().map(|(bytes, _)| digest(bytes)),
                        managed: None,
                        pending: None,
                        extra: BTreeMap::new(),
                    },
                );
            }
            let entry = entries.get_mut(&name).expect("recorded target");
            entry.pending = Some(digest(&content));
            store.save(&entries)?;
            store.atomic(
                &target,
                &content,
                if name == HELPER { 0o755 } else { 0o644 },
            )?;
            let entry = entries.get_mut(&name).expect("recorded target");
            entry.managed = entry.pending.take();
            store.save(&entries)?;
        }
        host.enable()?;
        println!(
            "Configured. Active display manager untouched. Log in with your password on next boot."
        );
    } else {
        for (name, entry) in &entries {
            match store.optional(&store.path(name), false, LIMIT)? {
                None if entry.backup.is_none() => (),
                Some((bytes, _)) if accepted(entry, &bytes) => (),
                _ => {
                    return Err(invalid(format!(
                        "Refusing to overwrite modified file: {name}"
                    )));
                }
            }
        }
        // PAM/config restoration precedes removal of the helper they invoke.
        let mut names: Vec<_> = entries.keys().cloned().collect();
        names.sort_by_key(|name| name == HELPER);
        for name in names {
            let entry = &entries[&name];
            if let Some(backup) = &entry.backup {
                let (bytes, mode) = store.read(
                    &store.path(backup.to_str().expect("validated backup")),
                    false,
                    LIMIT,
                )?;
                store.atomic(&store.path(&name), &bytes, mode)?;
            } else if store.path(&name).try_exists()? {
                fs::remove_file(store.path(&name))?;
                File::open(store.path(&name).parent().expect("target parent"))?.sync_all()?;
            }
            entries.remove(&name);
            store.save(&entries)?;
        }
        if store.journal_path().try_exists()? {
            fs::remove_file(store.journal_path())?;
            File::open(store.path(DIRECTORY))?.sync_all()?;
        }
        println!("Previous greetd/PAM files restored. Display manager service unchanged.");
    }
    Ok(())
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    if args.len() != 1 || !["install", "restore"].contains(&args[0].as_str()) {
        return Err(invalid("Expected system-setup install|restore"));
    }
    // SAFETY: geteuid takes no arguments and retains no pointers.
    if unsafe { libc::geteuid() } != 0 {
        return Err(invalid(
            "Run with sudo; user deployment does not edit system authentication files",
        ));
    }
    let store = Store {
        base: PathBuf::from("/"),
        uid: 0,
    };
    let _lock = store.lock()?;
    perform(root, &args[0], &store, &Native)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        os::unix::fs::PermissionsExt,
        os::unix::fs::symlink,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        store: Store,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "foundation-login-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&base).unwrap();
            fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
            let root = base.join("checkout");
            let system = base.join("system-root");
            fs::create_dir(&system).unwrap();
            // SAFETY: geteuid takes no arguments or pointers.
            let store = Store {
                base: system,
                uid: unsafe { libc::geteuid() },
            };
            for (_, source) in TARGETS {
                fs::create_dir_all(root.join(source).parent().unwrap()).unwrap();
            }
            fs::write(root.join(TARGETS[0].1), b"\x7fELFfixture native helper").unwrap();
            fs::write(root.join(TARGETS[1].1), b"[terminal]\nvt = 1\n[default_session]\ncommand = \"tuigreet\"\nuser = \"greeter\"\n").unwrap();
            let mut pam =
                b"auth optional pam_oo7.so\nsession include system-local-login\n".to_vec();
            pam.extend_from_slice(READY_HOOK);
            pam.extend_from_slice(b"session optional pam_oo7.so\n");
            fs::write(root.join(TARGETS[2].1), pam).unwrap();
            store
                .atomic(&store.path(CONFIG), b"original config", 0o640)
                .unwrap();
            store
                .atomic(&store.path(PAM), b"original PAM", 0o644)
                .unwrap();
            Self { base, root, store }
        }
        fn install(&self) {
            perform(&self.root, "install", &self.store, &Fake::default()).unwrap();
        }
        fn bytes(&self, name: &str) -> Vec<u8> {
            fs::read(self.store.path(name)).unwrap()
        }
        fn entry(&self, name: &str) -> Entry {
            self.store.load().unwrap()[name].clone()
        }
        fn backup(&self, entry: &Entry) -> PathBuf {
            self.store
                .path(entry.backup.as_ref().unwrap().to_str().unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.base).unwrap();
        }
    }
    #[derive(Default)]
    struct Fake {
        reject: bool,
        enable_fails: bool,
        enables: Cell<usize>,
    }
    impl Host for Fake {
        fn validate(&self, _: &Store) -> Result<()> {
            if self.reject {
                Err(invalid("fixture capability missing"))
            } else {
                Ok(())
            }
        }
        fn enable(&self) -> Result<()> {
            self.enables.set(self.enables.get() + 1);
            if self.enable_fails {
                Err(invalid("fixture enable failed"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn install_rerun_restore_preserve_original_bytes_modes_and_unknown_metadata() {
        let f = Fixture::new();
        let _lock = f.store.lock().unwrap();
        f.install();
        let first = f.store.load().unwrap();
        let original = f.entry(PAM);
        let backup = f.bytes(original.backup.as_ref().unwrap().to_str().unwrap());
        let mut journal = first.clone();
        journal
            .get_mut(PAM)
            .unwrap()
            .extra
            .insert("future".into(), Value::Bool(true));
        f.store.save(&journal).unwrap();
        f.install();
        assert_eq!(f.entry(PAM).original, original.original);
        assert_eq!(fs::read(f.backup(&original)).unwrap(), backup);
        assert_eq!(f.entry(PAM).extra["future"], true);
        assert_eq!(
            fs::metadata(f.store.path(HELPER)).unwrap().mode() & 0o777,
            0o755
        );
        perform(&f.root, "restore", &f.store, &Fake::default()).unwrap();
        assert_eq!(f.bytes(PAM), b"original PAM");
        assert_eq!(f.bytes(CONFIG), b"original config");
        assert_eq!(
            fs::metadata(f.store.path(CONFIG)).unwrap().mode() & 0o777,
            0o640
        );
        assert!(!f.store.path(HELPER).exists());
        assert!(!f.store.journal_path().exists());
        f.install();
        assert_eq!(f.entry(PAM).original, original.original);
    }
    #[test]
    fn source_upgrade_does_not_rebase_rollback_backups() {
        let f = Fixture::new();
        f.install();
        let original = f.entry(PAM).original;
        let mut updated = fs::read(f.root.join(TARGETS[2].1)).unwrap();
        updated.extend_from_slice(b"# updated template\n");
        fs::write(f.root.join(TARGETS[2].1), &updated).unwrap();
        f.install();
        assert_eq!(f.bytes(PAM), updated);
        assert_eq!(f.entry(PAM).original, original);
        perform(&f.root, "restore", &f.store, &Fake::default()).unwrap();
        assert_eq!(f.bytes(PAM), b"original PAM");
    }
    #[test]
    fn all_targets_are_preflighted_before_publishing_an_upgrade_or_restore() {
        let f = Fixture::new();
        f.install();
        let config = f.bytes(CONFIG);
        let helper = f.bytes(HELPER);
        f.store
            .atomic(&f.store.path(PAM), b"external user PAM edit", 0o644)
            .unwrap();
        fs::write(f.root.join(TARGETS[0].1), b"\x7fELFchanged helper").unwrap();
        for action in ["install", "restore"] {
            assert!(perform(&f.root, action, &f.store, &Fake::default()).is_err());
        }
        assert_eq!(f.bytes(CONFIG), config);
        assert_eq!(f.bytes(HELPER), helper);
        assert!(f.store.journal_path().exists());
    }
    #[test]
    fn missing_or_changed_backups_stop_before_targets_change() {
        for missing in [false, true] {
            let f = Fixture::new();
            f.install();
            let backup = f.backup(&f.entry(PAM));
            let current = f.bytes(PAM);
            if missing {
                fs::remove_file(backup).unwrap();
            } else {
                fs::write(backup, b"tampered").unwrap();
            }
            assert!(perform(&f.root, "restore", &f.store, &Fake::default()).is_err());
            assert_eq!(f.bytes(PAM), current);
            assert!(f.store.journal_path().exists());
        }
    }
    #[test]
    fn legacy_python_journal_pending_publication_and_partial_restore_resume() {
        for published in [false, true] {
            let f = Fixture::new();
            let original = f.bytes(PAM);
            let template = fs::read(f.root.join(TARGETS[2].1)).unwrap();
            let backup = Path::new(DIRECTORY).join("greetd");
            f.store
                .atomic(&f.store.path(backup.to_str().unwrap()), &original, 0o644)
                .unwrap();
            let entry = Entry {
                backup: Some(backup),
                original: Some(digest(&original)),
                managed: None,
                pending: Some(digest(&template)),
                extra: BTreeMap::new(),
            };
            f.store
                .save(&BTreeMap::from([(PAM.into(), entry)]))
                .unwrap();
            if published {
                f.store
                    .atomic(&f.store.path(PAM), &template, 0o644)
                    .unwrap();
            }
            f.install();
            assert_eq!(f.entry(PAM).original, Some(digest(&original)));
            // Crash after original publication but before journal removal.
            f.store
                .atomic(&f.store.path(PAM), &original, 0o644)
                .unwrap();
            perform(&f.root, "restore", &f.store, &Fake::default()).unwrap();
            assert_eq!(f.bytes(PAM), original);
        }
    }
    #[test]
    fn previous_exact_oo7_template_can_be_adopted_without_losing_backups() {
        let f = Fixture::new();
        f.install();
        let original = f.entry(PAM).original;
        let template = fs::read(f.root.join(TARGETS[2].1)).unwrap();
        f.store
            .atomic(&f.store.path(PAM), &legacy_pam(&template), 0o644)
            .unwrap();
        f.install();
        assert_eq!(f.bytes(PAM), template);
        assert_eq!(f.entry(PAM).original, original);
    }
    #[test]
    fn unsafe_journal_targets_and_escaping_backup_paths_are_rejected() {
        for escaped in [false, true] {
            let f = Fixture::new();
            f.install();
            let mut journal = f.store.load().unwrap();
            if escaped {
                journal.get_mut(PAM).unwrap().backup = Some(PathBuf::from("/etc/shadow"));
            } else {
                journal.insert("/etc/shadow".into(), f.entry(PAM));
            }
            f.store.save(&journal).unwrap();
            assert!(f.store.load().is_err());
        }
    }
    #[test]
    fn symlink_sources_targets_journals_and_writable_parents_are_refused() {
        for kind in 0..4 {
            let f = Fixture::new();
            f.install();
            match kind {
                0 => {
                    fs::remove_file(f.root.join(TARGETS[2].1)).unwrap();
                    symlink(f.store.path(PAM), f.root.join(TARGETS[2].1)).unwrap();
                }
                1 => {
                    fs::remove_file(f.store.path(PAM)).unwrap();
                    symlink(f.store.path(CONFIG), f.store.path(PAM)).unwrap();
                }
                2 => {
                    fs::remove_file(f.store.journal_path()).unwrap();
                    symlink(f.store.path(CONFIG), f.store.journal_path()).unwrap();
                }
                _ => {
                    fs::set_permissions(
                        f.store.path("/etc/pam.d"),
                        fs::Permissions::from_mode(0o777),
                    )
                    .unwrap();
                }
            }
            assert!(perform(&f.root, "install", &f.store, &Fake::default()).is_err());
        }
    }
    #[test]
    fn missing_capability_or_non_native_helper_cannot_publish_login_files() {
        let f = Fixture::new();
        let original = f.bytes(PAM);
        let host = Fake {
            reject: true,
            ..Fake::default()
        };
        assert!(perform(&f.root, "install", &f.store, &host).is_err());
        assert_eq!(f.bytes(PAM), original);
        assert!(!f.store.journal_path().exists());
        assert_eq!(host.enables.get(), 0);
        fs::write(f.root.join(TARGETS[0].1), b"#!/bin/sh\n").unwrap();
        assert!(perform(&f.root, "install", &f.store, &Fake::default()).is_err());
        assert_eq!(f.bytes(PAM), original);
    }
    #[test]
    fn enable_failure_retains_recoverable_journal_and_retry_keeps_backups() {
        let f = Fixture::new();
        let host = Fake {
            enable_fails: true,
            ..Fake::default()
        };
        assert!(perform(&f.root, "install", &f.store, &host).is_err());
        assert!(f.store.journal_path().exists());
        let original = f.entry(PAM).original;
        f.install();
        assert_eq!(f.entry(PAM).original, original);
    }
    #[test]
    fn restore_after_interrupted_first_publication_removes_only_owned_new_files() {
        let f = Fixture::new();
        let entry = Entry {
            backup: None,
            original: None,
            managed: None,
            pending: Some(digest(b"new")),
            extra: BTreeMap::new(),
        };
        f.store
            .save(&BTreeMap::from([(HELPER.into(), entry)]))
            .unwrap();
        perform(&f.root, "restore", &f.store, &Fake::default()).unwrap();
        assert_eq!(f.bytes(PAM), b"original PAM");
        assert!(!f.store.journal_path().exists());
    }
    #[test]
    fn source_size_limits_and_unsafe_existing_lock_are_enforced() {
        let f = Fixture::new();
        fs::write(f.root.join(TARGETS[2].1), vec![b'x'; 64 * 1024 + 1]).unwrap();
        assert!(sources(&f.root).is_err());
        let lock = f.store.path(&format!("{DIRECTORY}/lock"));
        f.store.atomic(&lock, b"", 0o644).unwrap();
        assert!(f.store.lock().is_err());
    }
    #[test]
    fn malformed_templates_or_empty_ownership_records_cannot_publish() {
        let sample = Fixture::new();
        let template = fs::read(sample.root.join(TARGETS[2].1)).unwrap();
        for content in [b"invalid PAM".to_vec(), legacy_pam(&template)] {
            let f = Fixture::new();
            fs::write(f.root.join(TARGETS[2].1), content).unwrap();
            assert!(perform(&f.root, "install", &f.store, &Fake::default()).is_err());
            assert_eq!(f.bytes(PAM), b"original PAM");
        }
        let f = Fixture::new();
        fs::write(f.root.join(TARGETS[1].1), b"not valid TOML").unwrap();
        assert!(perform(&f.root, "install", &f.store, &Fake::default()).is_err());
        f.store
            .save(&BTreeMap::from([(
                HELPER.into(),
                Entry {
                    backup: None,
                    original: None,
                    managed: None,
                    pending: None,
                    extra: BTreeMap::new(),
                },
            )]))
            .unwrap();
        assert!(f.store.load().is_err());
    }
    #[test]
    fn transaction_lock_interoperates_with_legacy_flock_without_python() {
        use std::os::fd::AsRawFd;
        let f = Fixture::new();
        let _guard = f.store.lock().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.store.path(&format!("{DIRECTORY}/lock")))
            .unwrap();
        // SAFETY: file owns a valid descriptor; flock retains no pointers.
        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn unexpected_owner_and_privileged_path_overrides_are_refused() {
        let f = Fixture::new();
        let foreign = Store {
            base: f.store.base.clone(),
            uid: f.store.uid + 1,
        };
        assert!(foreign.load().is_err());
        assert!(
            run(
                &f.root,
                &[
                    "install".into(),
                    "--test-root".into(),
                    f.store.base.to_string_lossy().into_owned()
                ]
            )
            .is_err()
        );
    }
}
