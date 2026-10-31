//! Marketplace's owned color layer. No Spotify account or profile data is read.
use crate::{Result, apps, invalid, process, state, theme_render as render};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
const SCHEME: &str = "DesktopFoundation";

fn paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
    let config = state::xdg("XDG_CONFIG_HOME", ".config")?.join("spicetify");
    let state = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation");
    Ok((
        config,
        state.join("spotify/theme-binding.json"),
        state.join("theme/current/spotify/color.ini"),
    ))
}
fn section<'a>(text: &'a str, name: &str) -> Result<Option<(usize, usize, &'a str)>> {
    let mut start = None;
    let mut end = text.len();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if trimmed == format!("[{name}]") {
                if start.is_some() {
                    return Err(invalid("Duplicate INI section"));
                }
                start = Some(offset + line.len());
            } else if start.is_some() && end == text.len() {
                end = offset;
            }
        }
        offset += line.len();
    }
    Ok(start.map(|start| (start, end, &text[start..end])))
}
fn value(text: &str, name: &str, key: &str) -> Result<Option<String>> {
    let mut result = None;
    if let Some((_, _, body)) = section(text, name)? {
        for line in body.lines() {
            let line = line.trim();
            if line.starts_with([';', '#']) {
                continue;
            }
            if let Some((k, v)) = line.split_once('=')
                && k.trim() == key
            {
                if result.is_some() {
                    return Err(invalid("Duplicate INI key"));
                }
                result = Some(v.trim().into());
            }
        }
    }
    Ok(result)
}
fn setting(text: &str, key: &str, value: Option<&str>) -> Result<String> {
    let (start, end, body) =
        section(text, "Setting")?.ok_or_else(|| invalid("Spicetify Setting section missing"))?;
    let mut new = String::new();
    let mut found = false;
    for line in body.split_inclusive('\n') {
        if line.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            if found {
                return Err(invalid("Duplicate Spicetify setting"));
            }
            found = true;
            if let Some(value) = value {
                new.push_str(&format!("{key} = {value}\n"));
            }
        } else {
            new.push_str(line);
        }
    }
    if !found && let Some(value) = value {
        if !new.is_empty() && !new.ends_with('\n') {
            new.push('\n');
        }
        new.push_str(&format!("{key} = {value}\n"));
    }
    Ok(format!("{}{}{}", &text[..start], new, &text[end..]))
}
fn merge(text: &str, generated: &str) -> Result<String> {
    if section(generated, SCHEME)?.is_none() {
        return Err(invalid("Generated Spotify scheme missing"));
    }
    let retained = if let Some((start, end, _)) = section(text, SCHEME)? {
        let header = text[..start]
            .rfind('[')
            .ok_or_else(|| invalid("Invalid INI header"))?;
        format!("{}{}", &text[..header], &text[end..])
    } else {
        text.into()
    };
    Ok(format!(
        "{}{}{}",
        retained,
        if retained.ends_with('\n') || retained.is_empty() {
            ""
        } else {
            "\n"
        },
        generated
    ))
}
fn check_owned(text: &str, saved: &Value) -> Result<()> {
    for (key, owned) in saved["owned"]
        .as_object()
        .ok_or_else(|| invalid("Invalid Spotify theme journal"))?
    {
        let current = value(text, "Setting", key)?;
        let original = saved["original"][key].as_str();
        if current.as_deref() != owned.as_str()
            && !(saved["pending"] == true && current.as_deref() == original)
        {
            return Err(invalid(format!(
                "Spotify theme setting changed outside adapter: {key}"
            )));
        }
    }
    Ok(())
}
fn palette(root: &Path, generated: &Path) -> Result<String> {
    if generated.is_file() {
        return Ok(fs::read_to_string(generated)?);
    }
    let semantic = generated
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid("Missing palette path"))?
        .join("semantic.json");
    let path = if semantic.is_file() {
        semantic
    } else {
        root.join("theme/generated.json")
    };
    render::spotify_colors(&render::read(&path)?)
}
fn refresh_assets(text: &str) -> Result<()> {
    let app = PathBuf::from(value(text, "Setting", "spotify_path")?.unwrap_or_default())
        .join("Apps/xpui/spicetify-config.json");
    if apps::executable("spicetify") && app.is_file() {
        process::checked(
            &["spicetify".into(), "refresh".into(), "--no-restart".into()],
            None,
            Duration::from_secs(60),
            true,
        )?;
    }
    Ok(())
}
pub fn run(root: &Path, action: &str) -> Result<()> {
    if !["install", "refresh", "restore"].contains(&action) {
        return Err(invalid("Expected Spotify theme install|refresh|restore"));
    }
    let (config, journal, generated) = paths()?;
    if action != "install" && !journal.exists() {
        return Ok(());
    }
    let directory = journal.parent().expect("journal parent");
    state::private_directory(directory)?;
    let _lock = state::lock(
        &directory.join("theme-binding.lock"),
        Duration::from_secs(10),
    )?;
    let file = config.join("config-xpui.ini");
    let color = config.join("Themes/marketplace/color.ini");
    if action == "refresh" && !file.is_file() {
        return Ok(());
    }
    let mut text = fs::read_to_string(&file)?;
    if value(&text, "Setting", "current_theme")?.as_deref() != Some("marketplace") {
        return Err(invalid(
            "Spotify theme is not Marketplace; refusing overwrite",
        ));
    }
    let mut saved = if journal.is_file() {
        render::read(&journal)?
    } else {
        if action != "install" {
            return Ok(());
        }
        if !value(&text, "AdditionalOptions", "custom_apps")?
            .unwrap_or_default()
            .split('|')
            .any(|s| s.trim() == "marketplace")
        {
            return Err(invalid("Marketplace is not configured"));
        }
        let old = fs::read_to_string(&color).unwrap_or_default();
        if section(&old, SCHEME)?.is_some() {
            return Err(invalid("Spotify scheme already exists without ownership"));
        }
        let saved = json!({"original":{"color_scheme":value(&text,"Setting","color_scheme")?,"replace_colors":value(&text,"Setting","replace_colors")?},"owned":{"color_scheme":SCHEME,"replace_colors":"1"},"pending":true});
        render::write_json(&journal, &saved)?;
        saved
    };
    check_owned(&text, &saved)?;
    if action == "restore" {
        for (key, value) in saved["original"]
            .as_object()
            .ok_or_else(|| invalid("Invalid Spotify journal"))?
        {
            text = setting(&text, key, value.as_str())?;
        }
        let colors = fs::read_to_string(&color)?;
        let restored = if let Some((start, end, _)) = section(&colors, SCHEME)? {
            let header = colors[..start]
                .rfind('[')
                .ok_or_else(|| invalid("Invalid scheme header"))?;
            format!("{}{}", &colors[..header], &colors[end..])
        } else {
            colors
        };
        render::write(&color, &restored)?;
        render::write(&file, &text)?;
        fs::remove_file(&journal)?;
        refresh_assets(&text)?;
        return Ok(());
    }
    if action == "refresh" && !color.exists() {
        return Ok(());
    }
    let generated = palette(root, &generated)?;
    render::write(
        &color,
        &merge(&fs::read_to_string(&color).unwrap_or_default(), &generated)?,
    )?;
    if action == "install" {
        for (key, value) in saved["owned"]
            .as_object()
            .ok_or_else(|| invalid("Invalid Spotify journal"))?
        {
            text = setting(&text, key, value.as_str())?;
        }
        render::write(&file, &text)?;
        saved
            .as_object_mut()
            .expect("journal object")
            .remove("pending");
        render::write_json(&journal, &saved)?;
    } else {
        refresh_assets(&text)?;
    }
    Ok(())
}
pub fn check(root: &Path) -> Result<bool> {
    // SAFETY: geteuid takes no pointers. Never execute user-owned tools as root.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Run the Spotify check as your normal desktop user"));
    }
    let config = state::xdg("XDG_CONFIG_HOME", ".config")?;
    let apps = state::xdg("XDG_DATA_HOME", ".local/share")?.join("desktop-foundation/apps");
    let pins = render::read(&root.join("apps/spotify/sources.json"))?;
    for name in ["spotify", "spicetify"] {
        let stamp = apps.join(name).join(".foundation-source.json");
        if !stamp.is_file() {
            return Err(invalid(format!("Missing repo-owned application: {name}")));
        }
        if name == "spotify" && render::read(&stamp)? != pins[name] {
            return Err(invalid("Spotify source differs from pinned client"));
        }
    }
    let binary = apps.join("spicetify/spicetify");
    let result = process::checked(
        &[binary.to_string_lossy().into_owned(), "--version".into()],
        None,
        Duration::from_secs(10),
        true,
    )?;
    if Some(String::from_utf8_lossy(&result.stdout).trim()) != pins["spicetify"]["version"].as_str()
    {
        return Err(invalid("Spicetify version differs from pin"));
    }
    let text = fs::read_to_string(config.join("spicetify/config-xpui.ini"))?;
    for (key, expected) in [
        ("spotify_path", apps.join("spotify/usr/share/spotify")),
        ("prefs_path", config.join("spotify/prefs")),
    ] {
        if value(&text, "Setting", key)?.as_deref() != Some(expected.to_string_lossy().as_ref()) {
            return Err(invalid(format!("Spicetify path mismatch: {key}")));
        }
    }
    for name in ["manifest.json", "index.js"] {
        if !config
            .join("spicetify/CustomApps/marketplace")
            .join(name)
            .is_file()
        {
            return Err(invalid(format!("Marketplace missing: {name}")));
        }
    }
    if value(&text, "Setting", "current_theme")?.as_deref() != Some("marketplace")
        || value(&text, "Setting", "color_scheme")?.as_deref() != Some(SCHEME)
    {
        return Err(invalid("Spotify graphite/Marketplace scheme not selected"));
    }
    Ok(config.join("spotify/prefs").exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unowned_ini_lines() {
        let text =
            "; keep me\n[Setting]\ncolor_scheme = Old\ninject_css = 1\n[Other]\nthing = yes\n";
        let changed = setting(text, "color_scheme", Some(SCHEME)).unwrap();
        assert!(changed.contains("; keep me\n"));
        assert!(changed.contains("inject_css = 1\n[Other]\nthing = yes\n"));
        assert_eq!(
            setting(&changed, "color_scheme", Some("Old")).unwrap(),
            text
        );
    }
    #[test]
    fn preserves_other_schemes() {
        assert_eq!(
            merge(
                "[Other]\nx=1\n[DesktopFoundation]\nx=2\n",
                "[DesktopFoundation]\nx=3\n"
            )
            .unwrap(),
            "[Other]\nx=1\n[DesktopFoundation]\nx=3\n"
        );
    }
    #[test]
    fn rejects_duplicate_settings() {
        assert!(value("[Setting]\nx=1\nx=2\n", "Setting", "x").is_err());
    }
}
