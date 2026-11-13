//! Adopt identified legacy shell configuration through the deployment journal.
use super::{Manifest, Paths};
use crate::{Result, deploy_units::Target, invalid, state};
use std::{fs, path::Path};

fn text(path: &Path) -> Result<Option<String>> {
    state::optional_text(path, 65536)
}
fn legacy_css(value: &str) -> bool {
    let first = value.lines().next().unwrap_or("").to_ascii_lowercase();
    first.starts_with("/* ryoku palette") || first.starts_with("/* inir palette")
}
fn clean_environment(value: &str) -> String {
    let mut output =
        String::from("# Desktop Foundation session environment; migrated preferences preserved.\n");
    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') && trimmed.to_ascii_lowercase().contains("ryoku") {
            continue;
        }
        if trimmed.starts_with("RYOKU_") || trimmed.starts_with("INIR_") {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=')
            && ["QML_IMPORT_PATH", "QML2_IMPORT_PATH"].contains(&key)
            && ["$HOME/.local/lib/qt6/qml", "${HOME}/.local/lib/qt6/qml"].contains(&value)
        {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}
pub(super) fn targets(
    root: &Path,
    paths: &Paths,
    manifest: &Manifest,
    write: bool,
) -> Result<Vec<Target>> {
    let mut targets = Vec::new();
    for version in ["gtk-3.0", "gtk-4.0"] {
        let path = paths.config.join(version).join("gtk.css");
        let recorded = manifest.entries.iter().any(|entry| entry.path == path);
        let missing = match fs::symlink_metadata(&path) {
            Ok(_) => false,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => return Err(e.into()),
        };
        if recorded || missing || text(&path)?.is_some_and(|value| legacy_css(&value)) {
            targets.push((
                path,
                paths
                    .state
                    .join("theme/current")
                    .join(version)
                    .join("gtk.css"),
            ));
        }
    }
    // This is the legacy file found on Lucky38, not a broad name-based cleanup.
    let old = paths.config.join("environment.d/ryoku-session.conf");
    let recorded = manifest.entries.iter().find(|entry| entry.path == old);
    let original = if let Some(entry) = recorded {
        entry.backup.as_deref().map(text).transpose()?.flatten()
    } else {
        text(&old)?
    };
    let identified = original
        .as_ref()
        .is_some_and(|value| value.starts_with("# Ryoku session environment"));
    if identified {
        let destination = paths
            .config
            .join("environment.d/session-desktop-foundation.conf");
        let source = paths.state.join("legacy/session-environment.conf");
        let destination_recorded = manifest
            .entries
            .iter()
            .any(|entry| entry.path == destination);
        if write && (!destination_recorded || !source.try_exists()?) {
            state::private_directory(&paths.state.join("legacy"))?;
            let mut migrated = clean_environment(original.as_deref().unwrap());
            // A pre-existing Foundation override keeps precedence and is also
            // backed up by install_link, just like the old environment file.
            if let Some(custom) = text(&destination)? {
                migrated.push_str(&custom);
            } else if fs::symlink_metadata(&destination).is_ok() {
                return Err(invalid(
                    "Refusing to replace a non-regular session environment",
                ));
            }
            state::atomic(&source, migrated.as_bytes(), 0o600)?;
        }
        targets.push((old, root.join("session/legacy-environment.conf")));
        targets.push((destination, source));
    }
    Ok(targets)
}
