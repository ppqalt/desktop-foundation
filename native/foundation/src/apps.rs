//! Typed application roles. MIME ownership remains in its separate recovery tool.
use crate::{Result, invalid, process, state};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct ApplicationRole {
    pub package: String,
    pub executable: String,
    pub desktop: String,
    pub mimes: Vec<String>,
}

#[derive(Deserialize)]
struct Roles {
    #[serde(rename = "personalBrowser")]
    personal_browser: ApplicationRole,
    #[serde(flatten)]
    entries: BTreeMap<String, ApplicationRole>,
}

#[derive(Deserialize)]
struct Profile {
    profile: Option<String>,
}

pub fn executable(name: &str) -> bool {
    let test = |p: PathBuf| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if name.contains('/') {
        return test(PathBuf::from(name));
    }
    env::var_os("PATH").is_some_and(|p| env::split_paths(&p).any(|dir| test(dir.join(name))))
}

fn read(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 {
        return Err(invalid(format!(
            "Configuration is too large: {}",
            path.display()
        )));
    }
    String::from_utf8(bytes)
        .map_err(|_| invalid(format!("Configuration is not UTF-8: {}", path.display())))
}

pub fn role(root: &Path, name: &str) -> Result<ApplicationRole> {
    if !matches!(
        name,
        "terminal" | "browser" | "files" | "pdf" | "image" | "text"
    ) {
        return Err(invalid(format!("Unknown application role: {name}")));
    }
    let mut roles: Roles =
        serde_json::from_str(&read(&root.join("config/application-roles.json"))?)?;
    let journal = state::xdg("XDG_STATE_HOME", ".local/state")?
        .join("desktop-foundation/application-roles.json");
    if journal.exists() {
        let profile: Profile = serde_json::from_str(&read(&journal)?)?;
        if profile.profile.as_deref() == Some("personal") {
            roles
                .entries
                .insert("browser".into(), roles.personal_browser);
        }
    }
    let role = roles
        .entries
        .remove(name)
        .ok_or_else(|| invalid(format!("Missing configured application role: {name}")))?;
    if role.executable.is_empty()
        || role.executable.contains('/')
        || role.desktop.contains('/')
        || !role.desktop.ends_with(".desktop")
    {
        return Err(invalid(format!(
            "Invalid configured application role: {name}"
        )));
    }
    Ok(role)
}

fn desktop_path(name: &str) -> Result<Option<PathBuf>> {
    let mut directories = vec![state::xdg("XDG_DATA_HOME", ".local/share")?];
    directories.extend(
        env::split_paths(
            &env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
        )
        .filter(|p| !p.as_os_str().is_empty()),
    );
    Ok(directories
        .into_iter()
        .map(|p| p.join("applications").join(name))
        .find(|p| p.is_file()))
}

pub fn validate_entry(text: &str, executable: &str) -> Result<()> {
    let mut in_entry = false;
    let mut seen_entry = false;
    let mut values = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_entry = line == "[Desktop Entry]";
            if in_entry && seen_entry {
                return Err(invalid("Duplicate Desktop Entry section"));
            }
            seen_entry |= in_entry;
            continue;
        }
        if in_entry {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| invalid("Invalid desktop entry line"))?;
            if values
                .insert(key.trim().to_ascii_lowercase(), value.trim())
                .is_some()
            {
                return Err(invalid("Duplicate desktop entry key"));
            }
        }
    }
    if values.get("type") != Some(&"Application") || values.get("hidden") == Some(&"true") {
        return Err(invalid("Desktop entry is not a visible application"));
    }
    let command = shell_words::split(
        values
            .get("exec")
            .ok_or_else(|| invalid("Desktop entry has no Exec"))?,
    )
    .map_err(|e| invalid(format!("Invalid desktop entry Exec: {e}")))?;
    if !command
        .first()
        .is_some_and(|s| Path::new(s).file_name().is_some_and(|n| n == executable))
    {
        return Err(invalid("Desktop entry executable differs from role"));
    }
    Ok(())
}

pub fn command(root: &Path, name: &str) -> Result<Vec<String>> {
    let role = role(root, name)?;
    let desktop = desktop_path(&role.desktop)?;
    if !executable(&role.executable) || desktop.is_none() {
        return Err(invalid(format!(
            "Missing {name}: install {} ({})",
            role.package, role.desktop
        )));
    }
    let desktop = desktop.ok_or_else(|| invalid("Desktop entry disappeared"))?;
    validate_entry(&read(&desktop)?, &role.executable)?;
    let mut command = vec![
        root.join("scripts/launch").to_string_lossy().into_owned(),
        role.executable,
    ];
    match name {
        "browser" => command.push("about:blank".into()),
        "files" => command.push(state::home()?.to_string_lossy().into_owned()),
        _ => {}
    }
    Ok(command)
}

pub fn launch(root: &Path, name: &str) -> Result<()> {
    process::replace(&command(root, name)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_validation_preserves_routing_and_rejects_ambiguous_metadata() {
        assert!(validate_entry("[Desktop Entry]\nType=Application\nName[fi]=Selain\nExec=\"/usr/bin/browser\" %U\n", "browser").is_ok());
        for text in [
            "[Desktop Entry]\nType=Application\nHidden=true\nExec=browser\n",
            "[Desktop Entry]\nType=Link\nExec=browser\n",
            "[Desktop Entry]\nType=Application\nExec=sh -c browser\n",
            "[Desktop Entry]\nType=Application\nExec=browser\nExec=other\n",
            "[Desktop Entry]\nType=Application\nExec='unterminated\n",
        ] {
            assert!(validate_entry(text, "browser").is_err());
        }
    }
}
