//! Reversible, explicitly targeted NetworkManager DNS changes.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

const PROPERTIES: [&str; 5] = [
    "connection.dns-over-tls",
    "ipv4.dns",
    "ipv4.ignore-auto-dns",
    "ipv6.dns",
    "ipv6.ignore-auto-dns",
];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Settings {
    tls: String,
    dns4: String,
    auto4: String,
    dns6: String,
    auto6: String,
}
impl Settings {
    fn values(&self) -> [&str; 5] {
        [&self.tls, &self.dns4, &self.auto4, &self.dns6, &self.auto6]
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    version: u8,
    uuid: String,
    original: Settings,
    desired: Settings,
    device: String,
}
trait Backend {
    fn settings(&self, uuid: &str) -> Result<Settings>;
    fn change(&self, uuid: &str, s: &Settings) -> Result<()>;
    fn reapply(&self, uuid: &str, device: &str) -> Result<()>;
    fn verify(&self, device: &str, desired: &Settings) -> Result<()>;
}
struct NetworkManager;
fn command(args: &[String]) -> Result<String> {
    let output = process::run(args, None, Duration::from_secs(20), true)?;
    if !output.status.success() {
        return Err(invalid(format!(
            "{} failed: {}",
            args[0],
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn nm(args: &[&str]) -> Result<String> {
    let mut cmd = vec!["nmcli".to_owned(), "--wait".into(), "10".into()];
    cmd.extend(args.iter().map(|s| (*s).to_owned()));
    command(&cmd)
}
impl Backend for NetworkManager {
    fn settings(&self, uuid: &str) -> Result<Settings> {
        let get = |field| {
            nm(&[
                "--escape",
                "no",
                "--get-values",
                field,
                "connection",
                "show",
                "uuid",
                uuid,
            ])
        };
        Ok(Settings {
            tls: get(PROPERTIES[0])?,
            dns4: get(PROPERTIES[1])?,
            auto4: get(PROPERTIES[2])?,
            dns6: get(PROPERTIES[3])?,
            auto6: get(PROPERTIES[4])?,
        })
    }
    fn change(&self, uuid: &str, s: &Settings) -> Result<()> {
        let mut args = vec!["connection", "modify", "uuid", uuid];
        for (key, value) in PROPERTIES.iter().zip(s.values()) {
            args.extend([*key, value]);
        }
        nm(&args)?;
        Ok(())
    }
    fn reapply(&self, uuid: &str, _device: &str) -> Result<()> {
        let device = active_device(uuid)?;
        nm(&["device", "reapply", &device])?;
        Ok(())
    }
    fn verify(&self, device: &str, desired: &Settings) -> Result<()> {
        let status = command(&["resolvectl".into(), "status".into(), device.into()])?;
        if desired.tls == "2" && !status.split_whitespace().any(|s| s == "+DNSOverTLS") {
            return Err(invalid("The active connection did not report DNS-over-TLS"));
        }
        command(&[
            "resolvectl".into(),
            "query".into(),
            "--interface".into(),
            device.into(),
            "example.com".into(),
        ])?;
        Ok(())
    }
}
fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}
fn journal_path(dir: &Path, uuid: &str) -> PathBuf {
    dir.join(format!("{uuid}.json"))
}
pub fn journals() -> Result<Vec<String>> {
    let dir = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/dns");
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut uuids = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && let Some(uuid) = path
                .file_stem()
                .and_then(|name| name.to_str())
                .filter(|s| valid_uuid(s))
        {
            uuids.push(uuid.to_owned());
        }
    }
    uuids.sort();
    Ok(uuids)
}
fn save(dir: &Path, j: &Journal) -> Result<()> {
    state::atomic(&journal_path(dir, &j.uuid), &serde_json::to_vec(j)?, 0o600)
}
fn load(dir: &Path, uuid: &str) -> Result<Journal> {
    let data = fs::read(journal_path(dir, uuid))?;
    if data.len() > 65536 {
        return Err(invalid("DNS journal is too large"));
    }
    let j: Journal = serde_json::from_slice(&data)?;
    if j.version != 1 || j.uuid != uuid {
        return Err(invalid("Invalid DNS journal"));
    }
    Ok(j)
}
fn equal(a: &Settings, b: &Settings) -> bool {
    fn tls(s: &str) -> &str {
        match s {
            "yes" | "2" => "2",
            "no" | "0" => "0",
            "opportunistic" | "1" => "1",
            "default" | "-1" => "-1",
            _ => s,
        }
    }
    fn list(s: &str) -> Vec<&str> {
        s.split([',', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }
    tls(&a.tls) == tls(&b.tls)
        && list(&a.dns4) == list(&b.dns4)
        && a.auto4 == b.auto4
        && list(&a.dns6) == list(&b.dns6)
        && a.auto6 == b.auto6
}
fn restore<B: Backend>(backend: &B, dir: &Path, uuid: &str) -> Result<()> {
    let journal = load(dir, uuid)?;
    let current = backend.settings(uuid)?;
    if equal(&current, &journal.original) {
        if !journal.device.is_empty() {
            backend.reapply(uuid, &journal.device)?;
        }
        fs::remove_file(journal_path(dir, uuid))?;
        return Ok(());
    }
    if !equal(&current, &journal.desired) {
        return Err(invalid(
            "DNS settings changed outside Foundation; refusing to overwrite them. The original settings remain in the journal.",
        ));
    }
    backend.change(uuid, &journal.original)?;
    if !journal.device.is_empty() {
        backend.reapply(uuid, &journal.device)?;
    }
    if !equal(&backend.settings(uuid)?, &journal.original) {
        return Err(invalid(
            "DNS restoration could not be verified; journal retained",
        ));
    }
    fs::remove_file(journal_path(dir, uuid))?;
    Ok(())
}
fn apply<B: Backend>(
    backend: &B,
    dir: &Path,
    uuid: &str,
    device: &str,
    desired: Settings,
) -> Result<()> {
    let path = journal_path(dir, uuid);
    if path.exists() {
        let existing = load(dir, uuid)?;
        let current = backend.settings(uuid)?;
        if equal(&current, &existing.desired) && equal(&desired, &existing.desired) {
            backend.verify(device, &existing.desired)?;
            return Ok(());
        }
        return Err(invalid(
            "This connection already has a DNS journal; restore it before applying another configuration",
        ));
    }
    let original = backend.settings(uuid)?;
    let journal = Journal {
        version: 1,
        uuid: uuid.into(),
        original,
        desired,
        device: device.into(),
    };
    save(dir, &journal)?;
    let operation = (|| {
        backend.change(uuid, &journal.desired)?;
        if !equal(&backend.settings(uuid)?, &journal.desired) {
            return Err(invalid(
                "NetworkManager did not retain the requested DNS settings",
            ));
        }
        backend.reapply(uuid, device)?;
        backend.verify(device, &journal.desired)?;
        Ok(())
    })();
    if let Err(error) = operation {
        return match restore(backend, dir, uuid) {
            Ok(()) => Err(invalid(format!(
                "DNS application failed and settings were restored: {error}"
            ))),
            Err(restore_error) => Err(invalid(format!(
                "DNS application failed: {error}; recovery required: {restore_error}"
            ))),
        };
    }
    Ok(())
}
fn desired(servers: &[IpAddr], tls_name: &str, transport: &str) -> Result<Settings> {
    if transport == "https" {
        return Err(invalid(
            "DNS-over-HTTPS is not supported by the NetworkManager/systemd-resolved backend; settings were not changed",
        ));
    }
    let encode = |v: &IpAddr| {
        if transport == "tls" {
            format!("{v}#{tls_name}")
        } else {
            v.to_string()
        }
    };
    Ok(Settings {
        tls: if transport == "tls" { "2" } else { "0" }.into(),
        dns4: servers
            .iter()
            .filter(|v| v.is_ipv4())
            .map(encode)
            .collect::<Vec<_>>()
            .join(","),
        auto4: "yes".into(),
        dns6: servers
            .iter()
            .filter(|v| v.is_ipv6())
            .map(encode)
            .collect::<Vec<_>>()
            .join(","),
        auto6: "yes".into(),
    })
}
fn ensure_backend() -> Result<()> {
    let active = |service: &str| {
        command(&["systemctl".into(), "is-active".into(), service.into()])
            .is_ok_and(|v| v == "active")
    };
    if !active("NetworkManager.service") || !active("systemd-resolved.service") {
        return Err(invalid(
            "DNS management requires active NetworkManager and systemd-resolved",
        ));
    }
    let config = command(&["NetworkManager".into(), "--print-config".into()])?;
    let mut in_main = false;
    let mut resolved_plugin = false;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_main = line == "[main]";
        } else if in_main && line.starts_with("dns=") {
            resolved_plugin = line == "dns=systemd-resolved";
        }
    }
    if !resolved_plugin {
        return Err(invalid(
            "NetworkManager is not explicitly using systemd-resolved; refusing to apply DNS",
        ));
    }
    let resolv = fs::read_to_string("/etc/resolv.conf")?;
    let servers: Vec<_> = resolv
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next() == Some("nameserver") {
                fields.next()
            } else {
                None
            }
        })
        .collect();
    if servers.is_empty()
        || servers
            .iter()
            .any(|s| !matches!(*s, "127.0.0.53" | "127.0.0.54"))
    {
        return Err(invalid(
            "System resolver is not exclusively using the systemd-resolved stub",
        ));
    }
    Ok(())
}
fn active_device(uuid: &str) -> Result<String> {
    let kind = nm(&["-g", "connection.type", "connection", "show", "uuid", uuid])?;
    if kind == "wireguard" || kind == "vpn" {
        return Err(invalid("Refusing to modify a VPN connection"));
    }
    let vpn = nm(&[
        "-g",
        "GENERAL.VPN",
        "connection",
        "show",
        "--active",
        "uuid",
        uuid,
    ])?;
    if vpn != "no" {
        return Err(invalid("Refusing to modify a VPN connection"));
    }
    let device = nm(&[
        "-g",
        "GENERAL.DEVICES",
        "connection",
        "show",
        "--active",
        "uuid",
        uuid,
    ])?;
    if device.is_empty() || device.contains([',', '\n']) {
        return Err(invalid("Connection must have exactly one active device"));
    }
    Ok(device)
}
pub fn default_connection() -> Result<String> {
    let active = nm(&["-t", "-f", "UUID,TYPE", "connection", "show", "--active"])?;
    let mut candidates = Vec::new();
    for line in active.lines().take(256) {
        let Some((uuid, kind)) = line.split_once(':') else {
            continue;
        };
        if !valid_uuid(uuid) || kind == "vpn" || kind == "wireguard" {
            continue;
        }
        if active_device(uuid).is_err() {
            continue;
        }
        let default4 = nm(&[
            "-g",
            "GENERAL.DEFAULT",
            "connection",
            "show",
            "--active",
            "uuid",
            uuid,
        ])?;
        let default6 = nm(&[
            "-g",
            "GENERAL.DEFAULT6",
            "connection",
            "show",
            "--active",
            "uuid",
            uuid,
        ])?;
        if default4 == "yes" || default6 == "yes" {
            candidates.push(uuid.to_owned());
        }
    }
    match candidates.as_slice() {
        [uuid] => Ok(uuid.clone()),
        [] => Err(invalid(
            "No active non-VPN default connection found; select a connection UUID explicitly",
        )),
        _ => Err(invalid(
            "Multiple default connections found; select a connection UUID explicitly",
        )),
    }
}
pub fn run(
    action: &str,
    uuid: &str,
    servers: &[IpAddr],
    tls_name: &str,
    transport: &str,
) -> Result<Value> {
    if !valid_uuid(uuid) {
        return Err(invalid("A NetworkManager connection UUID is required"));
    }
    let dir = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/dns");
    state::private_directory(&dir)?;
    let _lock = state::lock(&dir.join("lock"), Duration::from_secs(5))?;
    let backend = NetworkManager;
    match action {
        "apply" => {
            let target = desired(servers, tls_name, transport)?;
            ensure_backend()?;
            let device = active_device(uuid)?;
            apply(&backend, &dir, uuid, &device, target)?;
            Ok(
                json!({"applied":true,"connection":uuid,"device":device,"scope":"selected connection only; VPN and other connections retain their own DNS"}),
            )
        }
        "restore" => {
            restore(&backend, &dir, uuid)?;
            Ok(json!({"restored":true,"connection":uuid}))
        }
        _ => Err(invalid("Unknown DNS operation")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        sync::atomic::{AtomicU64, Ordering},
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    const UUID: &str = "00000000-0000-0000-0000-000000000001";
    struct Fake {
        current: RefCell<Settings>,
        fail_verify: Cell<bool>,
        writes: Cell<u32>,
    }
    impl Backend for Fake {
        fn settings(&self, _: &str) -> Result<Settings> {
            Ok(self.current.borrow().clone())
        }
        fn change(&self, _: &str, s: &Settings) -> Result<()> {
            *self.current.borrow_mut() = s.clone();
            self.writes.set(self.writes.get() + 1);
            Ok(())
        }
        fn reapply(&self, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn verify(&self, _: &str, _: &Settings) -> Result<()> {
            if self.fail_verify.get() {
                Err(invalid("offline"))
            } else {
                Ok(())
            }
        }
    }
    fn fixture() -> (PathBuf, Fake, Settings) {
        let dir = std::env::temp_dir().join(format!(
            "df-dns-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let original = Settings {
            tls: "-1".into(),
            dns4: "192.0.2.1".into(),
            auto4: "no".into(),
            dns6: String::new(),
            auto6: "no".into(),
        };
        let backend = Fake {
            current: RefCell::new(original.clone()),
            fail_verify: Cell::new(false),
            writes: Cell::new(0),
        };
        (dir, backend, original)
    }
    fn target() -> Settings {
        desired(&["9.9.9.9".parse().unwrap()], "dns.quad9.net", "tls").unwrap()
    }
    #[test]
    fn applies_idempotently_and_restores_original() {
        let (dir, backend, original) = fixture();
        apply(&backend, &dir, UUID, "test0", target()).unwrap();
        apply(&backend, &dir, UUID, "test0", target()).unwrap();
        assert_eq!(backend.writes.get(), 1);
        assert!(journal_path(&dir, UUID).exists());
        restore(&backend, &dir, UUID).unwrap();
        assert_eq!(*backend.current.borrow(), original);
        assert!(!journal_path(&dir, UUID).exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failure_rolls_back_and_external_edits_are_preserved() {
        let (dir, backend, original) = fixture();
        backend.fail_verify.set(true);
        assert!(apply(&backend, &dir, UUID, "test0", target()).is_err());
        assert_eq!(*backend.current.borrow(), original);
        assert!(!journal_path(&dir, UUID).exists());
        backend.fail_verify.set(false);
        apply(&backend, &dir, UUID, "test0", target()).unwrap();
        backend.current.borrow_mut().dns4 = "198.51.100.1".into();
        assert!(restore(&backend, &dir, UUID).is_err());
        assert_eq!(backend.current.borrow().dns4, "198.51.100.1");
        assert!(journal_path(&dir, UUID).exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn interrupted_change_can_be_recovered() {
        let (dir, backend, original) = fixture();
        let j = Journal {
            version: 1,
            uuid: UUID.into(),
            original: original.clone(),
            desired: target(),
            device: "test0".into(),
        };
        save(&dir, &j).unwrap();
        backend.change(UUID, &j.desired).unwrap();
        restore(&backend, &dir, UUID).unwrap();
        assert_eq!(*backend.current.borrow(), original);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn https_never_silently_downgrades() {
        assert!(desired(&["9.9.9.9".parse().unwrap()], "dns.quad9.net", "https").is_err());
        assert_eq!(target().tls, "2");
        assert_eq!(target().auto4, "yes");
    }
}
