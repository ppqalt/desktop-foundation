//! Password-free encrypted-root login gate and sleep authorization.
//! All persistent privileged assets are installed with an owned recovery journal.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
const DIR: &str = "/var/lib/desktop-foundation/authentication";
const HELPER: &str = "/usr/local/libexec/desktop-foundation-auth";
const POLICY: &str = "/etc/desktop-foundation/authentication.toml";
const RUNTIME: &str = "/run/desktop-foundation-auth";
const TARGETS: &[(&str, &str)] = &[
    (HELPER, "native/foundation/target/release/foundation-auth"),
    (
        "/etc/pam.d/desktop-foundation-lock",
        "session/authentication/pam-lock",
    ),
    (
        "/etc/systemd/system/greetd.service.d/30-desktop-foundation-auth.conf",
        "session/authentication/greetd.conf",
    ),
    (
        "/etc/systemd/system/desktop-foundation-lock-before-sleep.service",
        "session/authentication/before-sleep.service",
    ),
    (
        "/etc/systemd/system/sleep.target.d/30-desktop-foundation-lock.conf",
        "session/authentication/sleep.conf",
    ),
];
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    encrypted_login: bool,
    #[serde(default)]
    user: String,
}
fn checked_text(args: &[&str]) -> Result<String> {
    let out = process::checked(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        None,
        Duration::from_secs(3),
        true,
    )?;
    String::from_utf8(out.stdout).map_err(|_| invalid("Invalid system inspection output"))
}
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn protected(path: &Path, optional: bool) -> Result<Option<Vec<u8>>> {
    for parent in path.ancestors().skip(1) {
        let info = fs::symlink_metadata(parent)?;
        if !info.is_dir() || info.uid() != 0 || info.mode() & 0o022 != 0 {
            return Err(invalid("Unprotected authentication parent"));
        }
    }
    match fs::symlink_metadata(path) {
        Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(info)
            if !info.is_file()
                || info.uid() != 0
                || info.mode() & 0o7022 != 0
                || info.len() > 16 * 1024 * 1024 =>
        {
            Err(invalid("Unprotected authentication file"))
        }
        Ok(_) => Ok(Some(fs::read(path)?)),
    }
}
fn public_parents(path: &Path) -> Result<()> {
    let mut absent = Vec::new();
    for parent in path.ancestors().skip(1) {
        match fs::symlink_metadata(parent) {
            Ok(info) if !info.is_dir() || info.uid() != 0 || info.mode() & 0o022 != 0 => {
                return Err(invalid("Unprotected authentication directory"));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => absent.push(parent.to_path_buf()),
            Err(e) => return Err(e.into()),
        }
    }
    for parent in absent.iter().rev() {
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}
fn user_record(name: &str) -> Result<(u32, u32)> {
    if name.is_empty()
        || name.starts_with('-')
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid login account"));
    }
    let record = checked_text(&["/usr/bin/getent", "passwd", name])?;
    let fields: Vec<_> = record.trim_end().split(':').collect();
    if fields.len() != 7
        || fields[0] != name
        || fields[6].ends_with("nologin")
        || fields[6].ends_with("/false")
    {
        return Err(invalid("No interactive account"));
    }
    let uid: u32 = fields[2]
        .parse()
        .map_err(|_| invalid("Invalid account UID"))?;
    let gid = fields[3]
        .parse()
        .map_err(|_| invalid("Invalid account GID"))?;
    if !(1000..65534).contains(&uid) {
        return Err(invalid("Automatic boot login requires a regular user"));
    }
    Ok((uid, gid))
}
/// Accept only a direct single encrypted root mapping. Layered/multi-device,
/// keyfile and token boot paths retain conventional authentication.
fn root_luks(tree: &Value) -> Result<(String, String)> {
    let rows = tree["blockdevices"]
        .as_array()
        .ok_or_else(|| invalid("Missing root device"))?;
    if rows.len() != 1 || rows[0]["type"] != "crypt" {
        return Err(invalid("Root is not a direct encrypted mapping"));
    }
    let parents = rows[0]["children"]
        .as_array()
        .ok_or_else(|| invalid("Missing encrypted root backing device"))?;
    if parents.len() != 1 || parents[0]["fstype"] != "crypto_LUKS" {
        return Err(invalid("Ambiguous root encryption"));
    }
    let uuid = parents[0]["uuid"]
        .as_str()
        .ok_or_else(|| invalid("Missing LUKS UUID"))?;
    let path = parents[0]["path"]
        .as_str()
        .ok_or_else(|| invalid("Missing LUKS path"))?;
    if uuid.len() != 36
        || !uuid.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
        || !path.starts_with("/dev/")
    {
        return Err(invalid("Invalid encrypted root identity"));
    }
    Ok((path.into(), uuid.into()))
}
fn interactive_policy(cmdline: &str, crypttab: &str, uuid: &str, metadata: &Value) -> bool {
    // LUKS metadata version 2 is enforced by the JSON dump command itself.
    if metadata
        .get("tokens")
        .and_then(Value::as_object)
        .is_none_or(|t| !t.is_empty())
    {
        return false;
    }
    if cmdline.split_whitespace().any(|a| {
        a.starts_with("rd.luks.key=")
            || a.starts_with("luks.key=")
            || a.contains("tpm2-device=")
            || a.contains("fido2-device=")
            || a.contains("pkcs11-uri=")
    }) {
        return false;
    }
    let mut matched = false;
    for line in crypttab
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[1] == format!("UUID={uuid}") {
            if matched {
                return false;
            }
            matched = true;
            if fields
                .get(2)
                .is_some_and(|key| !["-", "none"].contains(key))
            {
                return false;
            }
            if fields.get(3).is_some_and(|options| {
                options.split(',').any(|o| {
                    o.starts_with("tpm2-device=")
                        || o.starts_with("fido2-device=")
                        || o.starts_with("pkcs11-uri=")
                        || o == "headless"
                })
            }) {
                return false;
            }
            // Credential caching must already be explicitly disabled in the
            // boot policy before passwordless desktop login is permitted.
            if !fields
                .get(3)
                .is_some_and(|opts| opts.split(',').any(|o| o == "password-cache=no"))
            {
                return false;
            }
        }
    }
    matched
}
fn encrypted_root() -> Result<(String, Value, String, String)> {
    let source = checked_text(&["/usr/bin/findmnt", "-n", "-o", "SOURCE", "/"])?;
    let source = source
        .trim()
        .split('[')
        .next()
        .unwrap_or_default()
        .to_string();
    let tree: Value = serde_json::from_str(&checked_text(&[
        "/usr/bin/lsblk",
        "--json",
        "--tree",
        "--inverse",
        "--output",
        "PATH,TYPE,FSTYPE,UUID",
        &source,
    ])?)?;
    let (device, uuid) = root_luks(&tree)?;
    Ok((source, tree, device, uuid))
}
fn boot_user() -> Result<Option<String>> {
    let bytes = protected(Path::new(POLICY), false)?
        .ok_or_else(|| invalid("Missing authentication policy"))?;
    let policy: Policy =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| invalid("Invalid policy"))?)
            .map_err(|_| invalid("Invalid authentication policy"))?;
    if !policy.encrypted_login || Path::new(DIR).join("rebuild-required").exists() {
        return Ok(None);
    }
    user_record(&policy.user)?;
    let status = checked_text(&["/usr/bin/passwd", "--status", &policy.user])?;
    if status.split_whitespace().nth(1) != Some("P") {
        return Ok(None);
    }
    let (source, tree, device, uuid) = encrypted_root()?;
    if tree["blockdevices"][0]["fstype"] == "btrfs" {
        let info = checked_text(&["/usr/bin/btrfs", "filesystem", "show", "--raw", "/"])?;
        if !info.lines().any(|l| {
            l.split_whitespace()
                .collect::<Vec<_>>()
                .starts_with(&["Total", "devices", "1"])
        }) {
            return Ok(None);
        }
    }
    let metadata: Value = serde_json::from_str(&checked_text(&[
        "/usr/bin/cryptsetup",
        "luksDump",
        "--dump-json-metadata",
        &device,
    ])?)?;
    let crypttab = fs::read_to_string("/etc/crypttab")?;
    let cmdline = fs::read_to_string("/proc/cmdline")?;
    if !interactive_policy(&cmdline, &crypttab, &uuid, &metadata) {
        return Ok(None);
    }
    // Exclude automatic keyfiles that cryptsetup discovers without crypttab.
    let mapper = Path::new(&source)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("Missing root mapper"))?;
    if ["/etc/cryptsetup-keys.d", "/run/cryptsetup-keys.d"]
        .iter()
        .any(|dir| Path::new(dir).join(format!("{mapper}.key")).exists())
    {
        return Ok(None);
    }
    Ok(Some(policy.user))
}
fn prepare_login() -> Result<()> {
    let base = protected(Path::new("/etc/greetd/config.toml"), false)?
        .ok_or_else(|| invalid("Missing greetd configuration"))?;
    let mut config: toml::Value =
        toml::from_str(std::str::from_utf8(&base).map_err(|_| invalid("Invalid greetd template"))?)
            .map_err(|_| invalid("Invalid greetd configuration"))?;
    // Existing custom automatic sessions are not reinterpreted as DF policy.
    if config.get("initial_session").is_some() {
        return Err(invalid(
            "Custom initial_session requires explicit reconciliation",
        ));
    }
    let runtime = Path::new(RUNTIME);
    if !runtime.exists() {
        fs::create_dir(runtime)?;
        fs::set_permissions(runtime, fs::Permissions::from_mode(0o700))?;
    }
    protected(&runtime.join("greetd.toml"), true)?;
    // Publish conventional login first; timeout or inspection failure must not
    // strand the machine without a greeter.
    state::atomic(&runtime.join("greetd.toml"), &base, 0o600)?;
    let first = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(runtime.join("attempted"));
    if let Err(error) = &first
        && error.kind() != std::io::ErrorKind::AlreadyExists
    {
        return Err(invalid("Cannot establish boot login attempt marker"));
    }
    if first.is_ok() {
        match boot_user() {
            Ok(Some(user)) => {
                state::atomic(
                    &runtime.join("passwordless-keyring"),
                    user.as_bytes(),
                    0o600,
                )?;
                config["initial_session"] = toml::Value::try_from(
                    json!({"user": user, "command": "/usr/bin/niri-session"}),
                )
                .map_err(|_| invalid("Invalid initial session"))?;
            }
            Ok(None) => (),
            Err(_) => eprintln!(
                "Desktop Foundation: encrypted boot authorization unavailable; using normal login"
            ),
        }
    }
    let text = toml::to_string(&config)
        .map_err(|_| invalid("Cannot render ephemeral login configuration"))?;
    state::atomic(&runtime.join("greetd.toml"), text.as_bytes(), 0o600)
}
/// A non-secret, root-only one-shot marker avoids waiting for a keyring password
/// that does not exist on a passwordless boot. Normal logins keep the PAM handoff.
pub fn skip_initial_keyring() -> Result<bool> {
    if unsafe { libc::geteuid() } != 0
        || std::env::var("PAM_SERVICE").as_deref() != Ok("greetd")
        || std::env::var("PAM_TYPE").as_deref() != Ok("open_session")
    {
        return Ok(false);
    }
    let marker = Path::new(RUNTIME).join("passwordless-keyring");
    if !Path::new(RUNTIME).exists() {
        return Ok(false);
    }
    let Some(user) = protected(&marker, true)? else {
        return Ok(false);
    };
    if std::env::var("PAM_USER").ok().as_deref().map(str::as_bytes) != Some(user.as_slice()) {
        return Ok(false);
    }
    fs::remove_file(marker)?;
    Ok(true)
}
fn before_sleep() -> Result<()> {
    let sessions = checked_text(&[
        "/usr/bin/loginctl",
        "list-sessions",
        "--no-legend",
        "--no-pager",
    ])?;
    let mut users = std::collections::BTreeSet::new();
    for line in sessions.lines() {
        let Some(id) = line.split_whitespace().next() else {
            continue;
        };
        let props = checked_text(&[
            "/usr/bin/loginctl",
            "show-session",
            id,
            "--property=Name",
            "--property=User",
            "--property=Type",
            "--property=Remote",
            "--property=Class",
            "--property=Desktop",
        ])?;
        if props
            .lines()
            .any(|l| l == "Remote=yes" || l == "Class=greeter")
            || !props
                .lines()
                .any(|l| l == "Class=user" || l == "Class=user-early")
        {
            continue;
        }
        if props
            .lines()
            .find_map(|l| l.strip_prefix("User="))
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|uid| !(1000..65534).contains(&uid))
        {
            continue;
        }
        let Some(name) = props.lines().find_map(|l| l.strip_prefix("Name=")) else {
            return Err(invalid("Local Wayland session has no account"));
        };
        let (uid, gid) = user_record(name)?;
        if !users.insert(uid) {
            continue;
        }
        // Only the managed lock service is called. No source paths or user
        // shell commands are evaluated by this root process.
        let runtime = format!("/run/user/{uid}");
        let mut command = std::process::Command::new("/usr/bin/systemctl");
        command
            .args(["--user", "restart", "desktop-foundation-lock.service"])
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("XDG_RUNTIME_DIR", &runtime)
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={runtime}/bus"),
            );
        use std::os::unix::process::CommandExt;

        // SAFETY: this child only clears supplementary groups before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(gid, gid, gid) != 0
                    || libc::setresuid(uid, uid, uid) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        // greetd initial sessions can be classified as tty before Niri starts;
        // inspect the actual managed compositor, not just logind's initial type.
        let mut probe = std::process::Command::new("/usr/bin/systemctl");
        probe
            .args(["--user", "is-active", "--quiet", "niri.service"])
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("XDG_RUNTIME_DIR", &runtime)
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={runtime}/bus"),
            );
        unsafe {
            probe.pre_exec(move || {
                if libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(gid, gid, gid) != 0
                    || libc::setresuid(uid, uid, uid) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        if !probe.status()?.success() {
            if props.lines().any(|l| l == "Desktop=niri") {
                return Err(invalid("Cannot inspect the Niri session before sleep"));
            }
            continue;
        }
        if !command.status()?.success() {
            return Err(invalid(
                "Session lock could not be confirmed; refusing sleep",
            ));
        }
    }
    Ok(())
}
#[derive(Deserialize, Serialize)]
struct Entry {
    original: Option<String>,
    managed: String,
    backup: Option<PathBuf>,
    original_mode: u32,
    #[serde(default)]
    pending: Option<String>,
}
fn validate_entry(name: &str, entry: &Entry) -> Result<()> {
    if entry.original.is_some() != entry.backup.is_some()
        || entry.original_mode & !0o777 != 0
        || entry.original_mode & 0o022 != 0
        || [&entry.managed]
            .into_iter()
            .chain(entry.original.iter())
            .chain(entry.pending.iter())
            .any(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        || entry.backup.as_ref().is_some_and(|p| {
            *p != Path::new(DIR).join(format!("original-{}", hash(name.as_bytes())))
        })
    {
        return Err(invalid("Invalid authentication recovery entry"));
    }
    Ok(())
}
fn console_crypttab(text: &str, uuid: &str, mapper: &str, add: bool) -> Result<String> {
    let mut found = false;
    let mut output = String::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if !line.trim_start().starts_with('#')
            && fields.len() >= 2
            && fields[1] == format!("UUID={uuid}")
        {
            if found || fields[0] != mapper {
                return Err(invalid(
                    "Ambiguous encrypted root mapping; retain conventional login",
                ));
            }
            found = true;
            let key = fields.get(2).copied().unwrap_or("-");
            let mut options: Vec<_> = fields
                .get(3)
                .copied()
                .unwrap_or("luks")
                .split(',')
                .filter(|o| !o.starts_with("password-cache=") && (!add || *o != "x-initrd.attach"))
                .collect();
            options.push("password-cache=no");
            if add && !options.contains(&"x-initrd.attach") {
                options.push("x-initrd.attach");
            }
            let comment = line
                .find('#')
                .map(|i| format!(" {}", &line[i..]))
                .unwrap_or_default();
            output.push_str(&format!(
                "{mapper} UUID={uuid} {key} {}{comment}\n",
                options.join(",")
            ));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !found && add {
        output.push_str(&format!(
            "{mapper} UUID={uuid} - luks,password-cache=no,x-initrd.attach\n"
        ));
    }
    Ok(output)
}
fn setup(root: &Path, restore: bool, locker_only: bool) -> Result<()> {
    public_parents(&Path::new(DIR).join("lock"))?;
    fs::create_dir_all(DIR)?;
    fs::set_permissions(DIR, fs::Permissions::from_mode(0o700))?;
    let _lock = state::lock(&Path::new(DIR).join("lock"), Duration::from_secs(3))?;
    let journal = Path::new(DIR).join("manifest.json");
    let mut entries: BTreeMap<String, Entry> = match protected(&journal, true)? {
        Some(data) => serde_json::from_slice(&data)?,
        None => BTreeMap::new(),
    };
    let allowed: Vec<&str> = TARGETS
        .iter()
        .map(|(name, _)| *name)
        .chain([POLICY, "/etc/crypttab", "/etc/crypttab.initramfs"])
        .collect();
    if entries.keys().any(|name| !allowed.contains(&name.as_str())) {
        return Err(invalid("Unexpected authentication journal target"));
    }
    for (name, entry) in &entries {
        validate_entry(name, entry)?;
        if let Some(backup) = &entry.backup
            && (backup.parent() != Some(Path::new(DIR))
                || protected(backup, false)?.map(|v| hash(&v)) != entry.original)
        {
            return Err(invalid("Authentication backup changed"));
        }
        let current = protected(Path::new(name), true)?.map(|v| hash(&v));
        if name == POLICY && current.as_ref() != Some(&entry.managed) {
            let bytes = protected(Path::new(POLICY), false)?
                .ok_or_else(|| invalid("Missing administrator authentication policy"))?;
            let _: Policy = toml::from_str(
                std::str::from_utf8(&bytes)
                    .map_err(|_| invalid("Invalid authentication policy"))?,
            )
            .map_err(|_| invalid("Invalid authentication policy"))?;
            continue;
        }
        if current.as_ref() != Some(&entry.managed)
            && current != entry.original
            && current != entry.pending
        {
            return Err(invalid(format!(
                "Authentication file modified outside DF: {name}"
            )));
        }
    }
    if restore {
        // Stop referencing the helper before restoring/removing that helper.
        let mut names: Vec<_> = entries.keys().cloned().collect();
        names.sort_by_key(|n| n == HELPER);
        for name in names {
            let entry = &entries[&name];
            if name == POLICY
                && protected(Path::new(&name), true)?
                    .map(|v| hash(&v))
                    .as_ref()
                    != Some(&entry.managed)
            {
                entries.remove(&name);
                state::atomic(&journal, &serde_json::to_vec_pretty(&entries)?, 0o600)?;
                continue;
            }
            if name.starts_with("/etc/crypttab") {
                state::atomic(&Path::new(DIR).join("rebuild-required"), b"1\n", 0o600)?;
            }
            if let Some(backup) = &entry.backup {
                state::atomic(Path::new(&name), &fs::read(backup)?, entry.original_mode)?;
            } else {
                fs::remove_file(&name)?;
            }
            entries.remove(&name);
            state::atomic(&journal, &serde_json::to_vec_pretty(&entries)?, 0o600)?;
        }
    } else {
        let mut sources = Vec::new();
        for (name, source) in TARGETS {
            if name.contains("greetd.service.d")
                && (locker_only
                    || crate::install::system_display_manager()?.as_deref()
                        != Some("greetd.service"))
            {
                continue;
            }
            let bytes = fs::read(root.join(source))?;
            if *name == HELPER && !bytes.starts_with(b"\x7fELF") {
                return Err(invalid("Build the native authentication helper first"));
            }
            sources.push((
                (*name).to_string(),
                bytes,
                if *name == HELPER { 0o755 } else { 0o644 },
            ));
        }
        if !Path::new(POLICY).exists() {
            let user = std::env::var("SUDO_USER").unwrap_or_default();
            let policy = Policy {
                encrypted_login: !locker_only && user_record(&user).is_ok(),
                user,
            };
            sources.push((
                POLICY.into(),
                toml::to_string(&policy)
                    .map_err(|_| invalid("Invalid initial authentication policy"))?
                    .into_bytes(),
                0o644,
            ));
        }
        // crypttab is the initramfs source of truth. No passphrase is read,
        // reused or written. If a root policy is unsupported, normal login wins.
        if !locker_only && let Ok((source, _, _, uuid)) = encrypted_root() {
            let mapper = Path::new(&source)
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| invalid("Invalid root mapper"))?;
            for name in ["/etc/crypttab", "/etc/crypttab.initramfs"] {
                if let Some(bytes) = protected(Path::new(name), true)? {
                    let text = std::str::from_utf8(&bytes)
                        .map_err(|_| invalid("Invalid crypttab encoding"))?;
                    let changed = console_crypttab(text, &uuid, mapper, name == "/etc/crypttab")?;
                    sources.push((name.into(), changed.into_bytes(), 0o600));
                }
            }
        }
        for (name, _, _) in &sources {
            public_parents(Path::new(name))?;
            protected(Path::new(name), true)?;
        }
        for (name, bytes, mode) in sources {
            if !entries.contains_key(&name) {
                let original = protected(Path::new(&name), true)?;
                let backup = original
                    .as_ref()
                    .map(|_| Path::new(DIR).join(format!("original-{}", hash(name.as_bytes()))));
                if let (Some(data), Some(path)) = (&original, &backup) {
                    state::atomic(path, data, 0o600)?;
                }
                entries.insert(
                    name.clone(),
                    Entry {
                        original_mode: fs::metadata(&name)
                            .map(|m| m.mode() & 0o777)
                            .unwrap_or(mode),
                        original: original.map(|v| hash(&v)),
                        backup,
                        managed: hash(&bytes),
                        pending: None,
                    },
                );
                state::atomic(&journal, &serde_json::to_vec_pretty(&entries)?, 0o600)?;
            }
            entries.get_mut(&name).expect("inserted").pending = Some(hash(&bytes));
            state::atomic(&journal, &serde_json::to_vec_pretty(&entries)?, 0o600)?;
            let changed = protected(Path::new(&name), true)?.as_deref() != Some(bytes.as_slice());
            if name.starts_with("/etc/crypttab") && changed {
                state::atomic(&Path::new(DIR).join("rebuild-required"), b"1\n", 0o600)?;
            }
            state::atomic(Path::new(&name), &bytes, mode)?;
            entries.get_mut(&name).expect("inserted").managed = hash(&bytes);
            entries.get_mut(&name).expect("inserted").pending = None;
            state::atomic(&journal, &serde_json::to_vec_pretty(&entries)?, 0o600)?;
        }
    }
    // Rebuild through the existing reviewed image workflow; never regenerate a
    // vendor-signed EFI executable. The install wrapper handles this step.
    checked_text(&["/usr/bin/systemctl", "daemon-reload"])?;
    println!("Authentication policy installed/restored. Current login session unchanged.");
    Ok(())
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    // SAFETY: UID query has no side effects.
    if unsafe { libc::geteuid() } != 0 {
        return Err(invalid(
            "Authentication setup requires administrator authentication",
        ));
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["install"] => setup(root, false, false),
        ["install", "--locker-only"] => setup(root, false, true),
        ["restore"] => setup(root, true, false),
        ["prepare-login"] => prepare_login(),
        ["greetd"] => {
            let runtime = Path::new(RUNTIME).join("greetd.toml");
            let config = if protected(&runtime, true).ok().flatten().is_some() {
                runtime
            } else {
                PathBuf::from("/etc/greetd/config.toml")
            };
            process::replace(&[
                "/usr/bin/greetd".into(),
                "--config".into(),
                config.to_string_lossy().into_owned(),
            ])
        }
        ["before-sleep"] => before_sleep(),
        _ => Err(invalid(
            "Expected authentication install|restore|prepare-login|before-sleep",
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_gate_rejects_unencrypted_and_ambiguous_roots() {
        assert!(root_luks(&json!({"blockdevices":[{"type":"part"}]})).is_err());
        let tree = json!({"blockdevices":[{"type":"crypt","children":[{"path":"/dev/test","fstype":"crypto_LUKS","uuid":"5e5a1509-008b-4e07-8cb1-1415df854fbf"}]}]});
        assert!(root_luks(&tree).is_ok());
    }
    #[test]
    fn unattended_and_cached_unlocks_never_authorize_login() {
        let uuid = "5e5a1509-008b-4e07-8cb1-1415df854fbf";
        let tab = format!("root UUID={uuid} - luks,password-cache=no\n");
        assert!(interactive_policy("rw", &tab, uuid, &json!({"tokens":{}})));
        for cmdline in [
            "rd.luks.key=/key",
            "rd.luks.options=tpm2-device=auto",
            "luks.key=/key",
        ] {
            assert!(!interactive_policy(
                cmdline,
                &tab,
                uuid,
                &json!({"tokens":{}})
            ));
        }
        assert!(!interactive_policy(
            "",
            &tab.replace(" - ", " /key "),
            uuid,
            &json!({"tokens":{}})
        ));
        assert!(!interactive_policy(
            "",
            &tab.replace(",password-cache=no", ""),
            uuid,
            &json!({"tokens":{}})
        ));
        assert!(!interactive_policy(
            "",
            &tab,
            uuid,
            &json!({"tokens":{"0":{}}})
        ));
    }
    #[test]
    fn crypttab_changes_only_the_root_options_and_is_idempotent() {
        let uuid = "5e5a1509-008b-4e07-8cb1-1415df854fbf";
        let before = format!(
            "# preserve comments\nroot UUID={uuid} - luks,discard,password-cache=yes\ndata UUID=another /secure/key luks\n"
        );
        let after = console_crypttab(&before, uuid, "root", true).unwrap();
        assert!(after.contains("luks,discard,password-cache=no,x-initrd.attach"));
        assert!(after.contains("data UUID=another /secure/key luks"));
        assert_eq!(console_crypttab(&after, uuid, "root", true).unwrap(), after);
        assert!(console_crypttab(&before, uuid, "different", true).is_err());
        assert!(console_crypttab(&(before.clone() + &before), uuid, "root", true).is_err());
        assert!(
            console_crypttab("", uuid, "root", true)
                .unwrap()
                .contains("password-cache=no,x-initrd.attach")
        );
    }
    #[test]
    fn recovery_entries_reject_missing_originals_and_unsafe_modes() {
        let mut entry = Entry {
            original: None,
            managed: hash(b"managed"),
            backup: None,
            original_mode: 0o644,
            pending: None,
        };
        assert!(validate_entry(POLICY, &entry).is_ok());
        entry.original_mode = 0o4777;
        assert!(validate_entry(POLICY, &entry).is_err());
        entry.original_mode = 0o644;
        entry.backup = Some(PathBuf::from("/etc/shadow"));
        assert!(validate_entry(POLICY, &entry).is_err());
    }
}
