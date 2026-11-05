//! Declarative DNS selection and read-only system capability inspection.
use crate::{Result, invalid, process, state};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    provider: String,
    transport: String,
    servers: Vec<IpAddr>,
    tls_name: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "adguard".into(),
            transport: "tls".into(),
            servers: Vec::new(),
            tls_name: String::new(),
        }
    }
}
#[derive(Debug, Serialize)]
struct Plan {
    provider: String,
    transport: String,
    servers: Vec<IpAddr>,
    tls_name: String,
    sources: Vec<PathBuf>,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn overlay(config: &mut Config, path: &Path, sources: &mut Vec<PathBuf>) -> Result<()> {
    let contents = match fs::read(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if contents.len() > 65536 {
        return Err(invalid(format!(
            "DNS settings too large: {}",
            path.display()
        )));
    }
    let contents = std::str::from_utf8(&contents)
        .map_err(|e| invalid(format!("Invalid DNS settings {}: {e}", path.display())))?;
    let overlay: toml::Value = toml::from_str(contents)
        .map_err(|e| invalid(format!("Invalid DNS settings {}: {e}", path.display())))?;
    let mut base = toml::Value::try_from(&*config)
        .map_err(|e| invalid(format!("Cannot merge DNS settings: {e}")))?;
    let target = base
        .as_table_mut()
        .ok_or_else(|| invalid("Invalid DNS defaults"))?;
    let additions = overlay
        .as_table()
        .ok_or_else(|| invalid("DNS configuration must be a table"))?;
    for (key, value) in additions {
        target.insert(key.clone(), value.clone());
    }
    *config = base
        .try_into()
        .map_err(|e| invalid(format!("Invalid DNS settings {}: {e}", path.display())))?;
    sources.push(path.to_owned());
    Ok(())
}
fn plan(root: &Path, user: &Path) -> Result<Plan> {
    let mut config = Config::default();
    let mut sources = Vec::new();
    overlay(&mut config, &root.join("config/dns.toml"), &mut sources)?;
    let fragments = user.join("desktop-foundation/dns.d");
    match fs::read_dir(&fragments) {
        Ok(entries) => {
            let mut paths = entries
                .map(|e| e.map(|v| v.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            paths.retain(|p| p.extension().is_some_and(|e| e == "toml"));
            paths.sort();
            for path in paths {
                overlay(&mut config, &path, &mut sources)?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    overlay(
        &mut config,
        &user.join("desktop-foundation/dns.toml"),
        &mut sources,
    )?;
    let (servers, tls_name) = match config.provider.as_str() {
        "adguard" => (
            vec![
                "94.140.14.14".parse().unwrap(),
                "94.140.15.15".parse().unwrap(),
            ],
            "dns.adguard-dns.com".to_owned(),
        ),
        "quad9" => (
            vec![
                "9.9.9.9".parse().unwrap(),
                "149.112.112.112".parse().unwrap(),
            ],
            "dns.quad9.net".to_owned(),
        ),
        "custom" => (config.servers.clone(), config.tls_name.clone()),
        _ => return Err(invalid("DNS provider must be adguard, quad9, or custom")),
    };
    if !matches!(config.transport.as_str(), "tls" | "https" | "plaintext") {
        return Err(invalid("DNS transport must be tls, https, or plaintext"));
    }
    if servers.is_empty()
        || servers
            .iter()
            .any(|s| s.is_unspecified() || s.is_multicast())
    {
        return Err(invalid("DNS requires usable unicast server addresses"));
    }
    if config.transport == "tls" && !valid_name(&tls_name) {
        return Err(invalid(
            "DNS-over-TLS requires a valid authentication hostname",
        ));
    }
    Ok(Plan {
        provider: config.provider,
        transport: config.transport,
        servers,
        tls_name,
        sources,
    })
}
fn command(args: &[&str]) -> Option<String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let out = process::run(&args, None, Duration::from_secs(5), true).ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
fn inspect() -> Value {
    let nm = command(&["systemctl", "is-active", "NetworkManager.service"])
        .is_some_and(|s| s == "active");
    let resolved = command(&["systemctl", "is-active", "systemd-resolved.service"])
        .is_some_and(|s| s == "active");
    let nm_config = command(&["NetworkManager", "--print-config"]);
    let nm_dns = nm_config
        .as_deref()
        .and_then(|s| {
            s.lines()
                .map(str::trim)
                .filter_map(|line| line.strip_prefix("dns="))
                .next_back()
        })
        .map(str::to_owned);
    let resolver = fs::read_to_string("/etc/resolv.conf").ok();
    let uses_stub = resolver.as_deref().is_some_and(|s| {
        s.lines().any(|line| {
            let mut fields = line.split_whitespace();
            fields.next() == Some("nameserver")
                && matches!(fields.next(), Some("127.0.0.53" | "127.0.0.54"))
        })
    });
    json!({"network_manager_active":nm,"resolved_active":resolved,"network_manager_dns":nm_dns,"resolver_uses_systemd_stub":uses_stub,"resolved_status":if resolved { command(&["resolvectl", "status"]) } else { None }})
}
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    match args {
        [action] if action == "plan" => Ok(serde_json::to_value(plan(
            root,
            &state::xdg("XDG_CONFIG_HOME", ".config")?,
        )?)?),
        [action] if action == "status" => Ok(inspect()),
        [action] if action == "verify" => {
            let result = command(&["resolvectl", "query", "example.com"]);
            if let Some(output) = result {
                Ok(
                    json!({"lookup_succeeded":true,"result":output,"note":"A successful lookup does not prove encryption or provider selection."}),
                )
            } else {
                Err(invalid(
                    "DNS lookup failed or systemd-resolved is unavailable",
                ))
            }
        }
        [action] if action == "apply" || action == "restore" => Err(invalid(
            "DNS changes are not available yet: safe transactional application and restore are not implemented",
        )),
        _ => Err(invalid("usage: dns plan|status|verify|apply|restore")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_tls_names() {
        assert!(valid_name("dns.quad9.net"));
        assert!(!valid_name("-bad.example"));
        assert!(!valid_name("bad..example"));
    }
    #[test]
    fn presets_and_invalid_settings() {
        let dir = std::env::temp_dir().join(format!("foundation-dns-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("user/desktop-foundation")).unwrap();
        let p = plan(&dir, &dir.join("user")).unwrap();
        assert_eq!(p.tls_name, "dns.adguard-dns.com");
        fs::write(
            dir.join("user/desktop-foundation/dns.toml"),
            "provider = 'quad9'\n",
        )
        .unwrap();
        assert_eq!(
            plan(&dir, &dir.join("user")).unwrap().tls_name,
            "dns.quad9.net"
        );
        fs::write(
            dir.join("user/desktop-foundation/dns.toml"),
            "provider = 'custom'\nservers = ['9.9.9.9']\ntls_name = ''\n",
        )
        .unwrap();
        assert!(plan(&dir, &dir.join("user")).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
