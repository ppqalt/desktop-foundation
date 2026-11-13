//! Native Qt/KDE colour adapters; preserve unrelated style, icon and font choices.
use crate::{
    Result,
    deploy::{Manifest, Paths},
    deploy_units::Target,
    invalid, state, theme_render as render,
};
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::symlink,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const LIMIT: u64 = 256 * 1024;

fn group(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}
fn entry(text: &str, section: &str, key: &str) -> Option<String> {
    let mut current = "";
    let mut result = None;
    for line in text.lines() {
        if let Some(name) = group(line) {
            current = name;
        } else if current == section
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            result = Some(value.trim().trim_matches('"').to_string());
        }
    }
    result
}
fn rewrite(text: &str, section: &str, changes: &[(&str, String)], remove_colors: bool) -> String {
    let mut output = String::new();
    let mut current = "";
    let mut inserted = false;
    for line in text.lines() {
        if let Some(name) = group(line) {
            current = name;
            if name == section && !inserted {
                output.push_str(line);
                output.push('\n');
                for (key, value) in changes {
                    output.push_str(&format!("{key}={value}\n"));
                }
                inserted = true;
                continue;
            }
        }
        if remove_colors && (current.starts_with("Colors:") || current.starts_with("ColorEffects:"))
        {
            continue;
        }
        if current == section
            && line
                .split_once('=')
                .is_some_and(|(key, _)| changes.iter().any(|(name, _)| *name == key.trim()))
        {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    if !inserted {
        output.push_str(&format!("\n[{section}]\n"));
        for (key, value) in changes {
            output.push_str(&format!("{key}={value}\n"));
        }
    }
    output
}
fn qt_path(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .filter(|value| !value.chars().any(char::is_control))
        .ok_or_else(|| invalid("Invalid Qt palette path"))?;
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
fn legacy_kde(text: &str) -> bool {
    entry(text, "General", "ColorScheme")
        .is_some_and(|name| ["ryoku", "inir"].contains(&name.to_ascii_lowercase().as_str()))
}
fn read_current(path: &Path, managed: &Path) -> Result<String> {
    let owned = fs::read_link(path).is_ok_and(|value| value == managed);
    let actual = if owned {
        path.canonicalize()?
    } else {
        path.to_path_buf()
    };
    let value = state::optional_text(&actual, LIMIT)?;
    if owned && value.is_none() {
        return Err(invalid(
            "Managed Qt preferences exceed the safe read limit or are not a regular file; nothing replaced",
        ));
    }
    Ok(value.unwrap_or_default())
}
pub fn palette(p: &Value) -> Result<String> {
    // QPalette ColorRole order, including Accent (Qt 6.6+). BrightText is light
    // foreground, never the dark on-accent colour used for HighlightedText.
    let roles = [
        "foreground",
        "elevated",
        "hover",
        "elevated",
        "background",
        "border",
        "foreground",
        "foreground",
        "foreground",
        "background",
        "background",
        "background",
        "accent",
        "background",
        "accent",
        "accentStrong",
        "elevated",
        "background",
        "elevated",
        "foreground",
        "muted",
        "accent",
    ];
    let mut result = String::from("[ColorScheme]\n");
    for name in ["active", "inactive", "disabled"] {
        let colors: Result<Vec<_>> = roles
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let role =
                    if name == "disabled" && [0, 6, 7, 8, 14, 15, 19, 20, 21].contains(&index) {
                        "muted"
                    } else if name != "active" && index == 12 {
                        "selected"
                    } else if name != "active" && index == 13 {
                        "foreground"
                    } else {
                        key
                    };
                Ok(format!("#ff{}", &render::role(p, role)?[1..]))
            })
            .collect();
        result.push_str(&format!("{name}_colors={}\n", colors?.join(", ")));
    }
    Ok(result)
}
pub fn kde_colors(p: &Value) -> Result<String> {
    let mut result = String::new();
    for (name, bg, fg) in [
        ("Window", "background", "foreground"),
        ("View", "background", "foreground"),
        ("Button", "elevated", "foreground"),
        ("Selection", "accent", "background"),
        ("Tooltip", "elevated", "foreground"),
        ("Complementary", "elevated", "foreground"),
        ("Header", "elevated", "foreground"),
    ] {
        result.push_str(&format!("\n[Colors:{name}]\n"));
        for (key, role) in [
            ("BackgroundNormal", bg),
            (
                "BackgroundAlternate",
                if name == "Selection" {
                    "accentStrong"
                } else {
                    "elevated"
                },
            ),
            ("ForegroundNormal", fg),
            (
                "ForegroundInactive",
                if name == "Selection" {
                    "background"
                } else {
                    "muted"
                },
            ),
            (
                "ForegroundActive",
                if name == "Selection" {
                    "background"
                } else {
                    "accent"
                },
            ),
            (
                "ForegroundLink",
                if name == "Selection" {
                    "background"
                } else {
                    "accent"
                },
            ),
            (
                "ForegroundVisited",
                if name == "Selection" {
                    "background"
                } else {
                    "accentStrong"
                },
            ),
            (
                "ForegroundNegative",
                if name == "Selection" {
                    "background"
                } else {
                    "error"
                },
            ),
            ("DecorationFocus", "accent"),
            ("DecorationHover", "hover"),
        ] {
            result.push_str(&format!("{key}={}\n", render::role(p, role)?));
        }
    }
    result.push_str("\n[ColorEffects:Inactive]\nEnable=false\n\n[ColorEffects:Disabled]\nEnable=true\nIntensityEffect=0\nColorEffect=0\nContrastEffect=1\nContrastAmount=0.65\n");
    Ok(result)
}
fn scheme(p: &Value) -> Result<String> {
    Ok(palette(p)? + "\n[General]\nName=Desktop Foundation\n" + &kde_colors(p)?)
}
pub fn render_palette(target: &Path, p: &Value) -> Result<()> {
    render::write(&target.join("qt/DesktopFoundation.colors"), &scheme(p)?)
}
pub fn ready(paths: &Paths, p: &Value) -> bool {
    let home = paths.state.join("qt");
    saved_link(&home, "current").is_ok_and(|link| link.is_some())
        && state::optional_text(&home.join("current/DesktopFoundation.colors"), LIMIT)
            .ok()
            .flatten()
            .is_some_and(|value| scheme(p).is_ok_and(|expected| value == expected))
        && home.join("current/kdeglobals").is_file()
        && home.join("current/qt6ct.conf").is_file()
}
fn link(home: &Path, name: &str, target: &Path) -> Result<()> {
    let temporary = home.join(format!(
        ".{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    symlink(target, &temporary)?;
    if let Err(error) = fs::rename(&temporary, home.join(name)) {
        let _ = fs::remove_file(temporary);
        return Err(error.into());
    }
    Ok(())
}
fn saved_link(home: &Path, name: &str) -> Result<Option<std::path::PathBuf>> {
    match fs::symlink_metadata(home.join(name)) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let value = fs::read_link(home.join(name))?;
            if !value.is_absolute() || value.parent() != Some(home.join("revisions").as_path()) {
                return Err(invalid("Refusing an unknown Qt palette link"));
            }
            Ok(Some(value))
        }
        Ok(_) => Err(invalid("Refusing a non-link Qt palette pointer")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
/// The palette pointer is a single atomic swap. Rollback creates new colour
/// assets from the requested palette and CURRENT preferences, not old fonts.
pub fn with_assets<T>(
    paths: &Paths,
    p: &Value,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let home = paths.state.join("qt");
    state::private_directory(&home)?;
    let revisions = home.join("revisions");
    state::private_directory(&revisions)?;
    let old = saved_link(&home, "current")?;
    let previous = saved_link(&home, "previous")?;
    let qt_pathname = paths.config.join("qt6ct/qt6ct.conf");
    let kde_pathname = paths.config.join("kdeglobals");
    let qt_managed = home.join("current/qt6ct.conf");
    let kde_managed = home.join("current/kdeglobals");
    let qt = read_current(&qt_pathname, &qt_managed)?;
    let kde = read_current(&kde_pathname, &kde_managed)?;
    let nonce = format!(
        "{}-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| invalid(e.to_string()))?
            .as_nanos(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let target = revisions.join(nonce);
    state::private_directory(&target)?;
    let prepare = (|| {
        let scheme_path = paths.config.join("qt6ct/colors/DesktopFoundation.conf");
        let qt_config = rewrite(
            &qt,
            "Appearance",
            &[
                ("color_scheme_path", qt_path(&scheme_path)?),
                ("custom_palette", "true".into()),
            ],
            false,
        );
        let kde_config = rewrite(
            &kde,
            "General",
            &[("ColorScheme", "DesktopFoundation".into())],
            true,
        )
        .trim_end()
        .to_owned()
            + "\n"
            + &kde_colors(p)?;
        render::write(&target.join("qt6ct.conf"), &qt_config)?;
        render::write(&target.join("kdeglobals"), &kde_config)?;
        render::write(&target.join("DesktopFoundation.colors"), &scheme(p)?)?;
        render::write_json(
            &target.join("owner.json"),
            &serde_json::json!({"kind":"foundation-qt-palette-v1"}),
        )?;
        if read_current(&qt_pathname, &qt_managed)? != qt
            || read_current(&kde_pathname, &kde_managed)? != kde
        {
            return Err(invalid(
                "Qt preferences changed during theme preparation; retry",
            ));
        }
        link(&home, "current", &target)
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_dir_all(&target);
        return Err(error);
    }
    match operation() {
        Ok(value) => {
            if let Some(old) = &old
                && let Err(error) = link(&home, "previous", old)
            {
                eprintln!("Qt palette history deferred: {error}");
            }
            if let Err(error) = prune(&revisions, &target, old.as_deref(), previous.as_deref()) {
                eprintln!("Qt palette cleanup deferred: {error}");
            }
            Ok(value)
        }
        Err(error) => {
            if let Some(old) = old {
                link(&home, "current", &old)?;
            } else {
                fs::remove_file(home.join("current"))?;
            }
            let _ = fs::remove_dir_all(target);
            Err(error)
        }
    }
}
fn prune(
    revisions: &Path,
    current: &Path,
    old: Option<&Path>,
    previous: Option<&Path>,
) -> Result<()> {
    let mut bundles: Vec<_> = fs::read_dir(revisions)?
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|t| t.is_dir())
                && render::read(&entry.path().join("owner.json"))
                    .is_ok_and(|v| v["kind"] == "foundation-qt-palette-v1")
        })
        .map(|entry| entry.path())
        .collect();
    bundles.sort();
    bundles.reverse();
    for bundle in bundles.into_iter().skip(6) {
        if bundle != current && old != Some(bundle.as_path()) && previous != Some(bundle.as_path())
        {
            fs::remove_dir_all(bundle)?;
        }
    }
    Ok(())
}

pub fn targets(paths: &Paths, manifest: &Manifest) -> Result<Vec<Target>> {
    let kde = paths.config.join("kdeglobals");
    let qt = paths.config.join("qt6ct/qt6ct.conf");
    let recorded = |path: &Path| manifest.entries.iter().any(|entry| entry.path == path);
    let kde_legacy = state::optional_text(&kde, LIMIT)?.is_some_and(|value| legacy_kde(&value));
    let qt_legacy = if let Some(value) = state::optional_text(&qt, LIMIT)? {
        entry(&value, "Appearance", "color_scheme_path").is_some_and(|value| {
            let file = Path::new(&value)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            ["ryoku.colors", "inir.colors"].contains(&file.as_str())
                || (kde_legacy && file == "darkly.colors")
        })
    } else {
        false
    };
    let mut targets = Vec::new();
    let kde_text = state::optional_text(&kde, LIMIT)?;
    let kde_unconfigured = kde_text.as_ref().is_some_and(|text| {
        entry(text, "General", "ColorScheme").is_none()
            && !text
                .lines()
                .any(|line| group(line).is_some_and(|name| name.starts_with("Colors:")))
    });
    let missing = |path: &Path| {
        fs::symlink_metadata(path).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    };
    if kde_legacy || kde_unconfigured || missing(&kde) || recorded(&kde) {
        targets.push((kde, paths.state.join("qt/current/kdeglobals")));
    }
    if qt_legacy || missing(&qt) || recorded(&qt) {
        targets.push((qt, paths.state.join("qt/current/qt6ct.conf")));
    }
    let consumers = targets;
    let mut targets = Vec::new();
    for path in [
        paths.data.join("color-schemes/DesktopFoundation.colors"),
        paths.config.join("qt6ct/colors/DesktopFoundation.conf"),
    ] {
        let missing = match fs::symlink_metadata(&path) {
            Ok(_) => false,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => return Err(e.into()),
        };
        if !consumers.is_empty() && !recorded(&path) && !missing {
            return Err(invalid(format!(
                "A custom palette already exists outside deployment ownership: {}",
                path.display()
            )));
        }
        if recorded(&path) || missing {
            targets.push((
                path,
                paths.state.join("qt/current/DesktopFoundation.colors"),
            ));
        }
    }
    // Palette catalogues exist before consumers are adopted, including on fresh installs.
    targets.extend(consumers);
    Ok(targets)
}
