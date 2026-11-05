//! Finite semantic-palette renderers. No renderer starts or controls an app.
use crate::{Result, input_config, invalid, state, theme};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn write(path: &Path, value: &str) -> Result<()> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("Missing output directory"))?,
    )?;
    state::atomic(path, value.as_bytes(), 0o644)
}
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    write(path, &(serde_json::to_string_pretty(value)? + "\n"))
}
pub fn role<'a>(palette: &'a Value, key: &str) -> Result<&'a str> {
    let color = palette[key]
        .as_str()
        .ok_or_else(|| invalid(format!("Missing palette role: {key}")))?;
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(invalid(format!("Invalid palette role: {key}")));
    }
    Ok(color)
}
pub fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "__pycache__" || path.extension().is_some_and(|e| e == "pyc") {
            continue;
        }
        let target = to.join(name);
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, target)?;
        } else {
            return Err(invalid(format!("Unsupported template: {}", path.display())));
        }
    }
    Ok(())
}
fn toml_config(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)?;
    if text.len() > 65536 {
        return Err(invalid(format!(
            "Configuration too large: {}",
            path.display()
        )));
    }
    let value: toml::Value = toml::from_str(&text)
        .map_err(|error| invalid(format!("Invalid configuration {}: {error}", path.display())))?;
    Ok(serde_json::to_value(value)?)
}
fn guard_legacy(path: &Path, original: &str, replacement: &str) -> Result<()> {
    match fs::read(path) {
        Ok(data) if format!("{:x}", Sha256::digest(&data)) != original => Err(invalid(format!(
            "Customized legacy configuration {} needs migration to {}; preserve the old file and move it aside after migrating",
            path.display(),
            replacement
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn number(value: &Value) -> Result<String> {
    if value.is_number() {
        Ok(value.to_string())
    } else {
        Err(invalid("Shared configuration requires a number"))
    }
}
fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization")
}
pub fn effective_input(root: &Path, profile: &str) -> Result<input_config::EffectiveInput> {
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid profile name"));
    }
    guard_legacy(
        &root.join("config/input.lua"),
        "d237b12d34a6abc5627a0b33a2bfdb7d3b01c635d741ab773cc0a720e2fcad3a",
        "config/input.toml",
    )?;
    let path = root.join("config/input.toml");
    input_config::load(
        toml_config(&path)?,
        &path,
        &root.join("profiles").join(profile),
        &input_config::user_config()?,
    )
}
pub fn niri(root: &Path, profile: &str, semantic: Option<&Value>) -> Result<String> {
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid profile name"));
    }
    guard_legacy(
        &root.join("config/window-appearance.lua"),
        "63c236a4357d46bc0ae3813a03ec7f23e2111139b08dd56a400643390f4317dd",
        "config/window-appearance.toml",
    )?;
    let v = toml_config(&root.join("config/window-appearance.toml"))?;
    let keyboard = effective_input(root, profile)?.settings;
    let monitor: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("config/monitor.toml"))?)
            .map_err(|e| invalid(e.to_string()))?;
    let radius = monitor
        .get("corner_radius")
        .map(|v| {
            v.as_integer()
                .ok_or_else(|| invalid("Invalid monitor radius"))
        })
        .transpose()?
        .unwrap_or(14);
    if !(0..=32).contains(&radius) {
        return Err(invalid("Monitor radius must be from 0 to 32"));
    }
    let mut text = fs::read_to_string(root.join("theme/templates/niri.kdl.in"))?;
    let shadow = |opacity: &Value| -> Result<String> {
        let alpha = opacity
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))
            .ok_or_else(|| invalid("Invalid shadow opacity"))?;
        // Match Python's ties-to-even rounding used by existing bundles.
        Ok(quoted(&format!(
            "{}{:02x}",
            role(&v["shadow"], "color")?,
            (alpha * 255.0).round_ties_even() as u8
        )))
    };
    let offset = (v["blur"]["radius"]
        .as_f64()
        .ok_or_else(|| invalid("Missing blur radius"))?
        * v["blur"]["strength"]
            .as_f64()
            .ok_or_else(|| invalid("Missing blur strength"))?
        / 3.0)
        .max(1.0);
    let saturation = 1.0
        + v["blur"]["saturationBoost"]
            .as_f64()
            .ok_or_else(|| invalid("Missing saturation"))?;
    let shadow_x = v["shadow"]["offset"]["x"]
        .as_f64()
        .ok_or_else(|| invalid("Missing shadow offset"))?;
    let implicit_palette = if semantic.is_none() {
        state::xdg("XDG_STATE_HOME", ".local/state")
            .ok()
            .and_then(|state| {
                read(&state.join("desktop-foundation/theme/current/semantic.json")).ok()
            })
            .or_else(|| read(&root.join("theme/generated.json")).ok())
    } else {
        None
    };
    let border = |key: &str, fallback: &str| -> Result<String> {
        Ok(quoted(
            if let Some(p) = semantic.or(implicit_palette.as_ref()) {
                role(p, key)?
            } else {
                role(&v["focus"], fallback)?
            },
        ))
    };
    let entries = [
        (
            "config",
            quoted(&root.join("compositor/niri/config.kdl").to_string_lossy()),
        ),
        ("root", quoted(&root.to_string_lossy())),
        (
            "session_start",
            quoted(&root.join("scripts/session-start").to_string_lossy()),
        ),
        ("layout", serde_json::to_string(&keyboard.layout)?),
        ("repeat_delay", keyboard.repeat_delay.to_string()),
        ("repeat_rate", keyboard.repeat_rate.to_string()),
        ("gaps", number(&v["spacing"]["betweenWindows"])?),
        ("edge", number(&v["spacing"]["desktopEdge"])?),
        ("border_width", number(&v["focus"]["borderWidth"])?),
        ("active_border", border("windowActive", "activeBorder")?),
        (
            "inactive_border",
            border("windowInactive", "inactiveBorder")?,
        ),
        ("shadow_range", number(&v["shadow"]["range"])?),
        ("shadow_x", number(&v["shadow"]["offset"]["x"])?),
        ("shadow_left_x", (-shadow_x).to_string()),
        ("shadow_y", number(&v["shadow"]["offset"]["y"])?),
        ("shadow_active", shadow(&v["shadow"]["activeOpacity"])?),
        ("shadow_inactive", shadow(&v["shadow"]["inactiveOpacity"])?),
        ("blur_passes", number(&v["blur"]["qualityPasses"])?),
        ("blur_offset", offset.to_string()),
        ("blur_noise", number(&v["blur"]["noise"])?),
        ("blur_saturation", saturation.to_string()),
        ("rounding", number(&v["rounding"])?),
        ("opacity", number(&v["opacity"]["active"])?),
        ("monitor_radius", radius.to_string()),
    ];
    for (key, value) in entries {
        text = text.replace(&format!("@{key}@"), &value);
    }
    if keyboard.overview_ctrl_scroll {
        // This setting requires the scoped compositor capability. Stock Niri
        // rejects it during deployment validation; never replace it with a
        // global wheel binding that would intercept application zoom.
        text.push_str("overview { ctrl-scroll-workspaces true; }\n");
    }
    let host = root.join("profiles").join(profile).join("niri.kdl");
    if host.is_file() {
        text.push_str(&format!("include {}\n", quoted(&host.to_string_lossy())));
    }
    Ok(text.replace("} ", "}; "))
}

pub fn terminal(root: &Path) -> Result<()> {
    let terminal = root.join("terminal");
    let p = read(&terminal.join("palette.json"))?;
    let color = |key: &str| -> Result<&str> {
        let v = p[key]
            .as_str()
            .ok_or_else(|| invalid(format!("Missing terminal color: {key}")))?;
        if v.len() != 6 || !v.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(invalid("Invalid terminal color"));
        }
        Ok(v)
    };
    let mut kitty =
        String::from("# Generated by the Rust theme renderer; edit terminal/palette.json.\n");
    for (name, key) in [
        ("background", "background"),
        ("foreground", "foreground"),
        ("cursor", "accent"),
        ("selection_background", "selection"),
        ("selection_foreground", "foreground"),
        ("url_color", "accent"),
    ] {
        kitty.push_str(&format!("{name} #{}\n", color(key)?));
    }
    let ansi = p["ansi"]
        .as_array()
        .filter(|v| v.len() == 16)
        .ok_or_else(|| invalid("Expected 16 terminal ANSI colors"))?;
    for (index, value) in ansi.iter().enumerate() {
        let value = value
            .as_str()
            .filter(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| invalid("Invalid ANSI color"))?;
        kitty.push_str(&format!("color{index} #{value}\n"));
    }
    write(&terminal.join("kitty/colors.conf"), &kitty)?;
    let mut fish = String::from("# Generated from terminal/palette.json.\n");
    for key in ["foreground", "muted", "accent", "error"] {
        fish.push_str(&format!("set -g foundation_{key} {}\n", color(key)?));
    }
    for (key, value) in [
        ("normal", "foreground"),
        ("command", "foreground"),
        ("param", "foreground"),
        ("comment", "muted"),
        ("autosuggestion", "muted"),
        ("error", "error"),
        ("quote", "accent"),
        ("operator", "accent"),
        ("escape", "accent"),
        ("redirection", "accent"),
        ("end", "accent"),
        ("valid_path", "foreground"),
    ] {
        fish.push_str(&format!("set -g fish_color_{key} {}\n", color(value)?));
    }
    write(&terminal.join("fish/theme.fish"), &fish)?;
    let mut config = read(&terminal.join("fastfetch/config.jsonc"))?;
    let rgb = |key| -> Result<String> {
        let c = color(key)?;
        Ok([0, 2, 4]
            .into_iter()
            .map(|i| {
                u8::from_str_radix(&c[i..i + 2], 16)
                    .expect("validated hex")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(";"))
    };
    config["display"]["color"]["keys"] = json!(format!("38;2;{}", rgb("muted")?));
    config["logo"]["color"] =
        json!({"1":format!("38;2;{}",rgb("accent")?),"2":format!("38;2;{}",rgb("accent")?)});
    write_json(&terminal.join("fastfetch/config.jsonc"), &config)
}

pub fn spotify_colors(p: &Value) -> Result<String> {
    let mut result = String::from("[DesktopFoundation]\n");
    for (key, value) in [
        ("text", "foreground"),
        ("subtext", "muted"),
        ("main", "background"),
        ("sidebar", "background"),
        ("player", "background"),
        ("card", "elevated"),
        ("shadow", ""),
        ("selected-row", "selected"),
        ("button", "accent"),
        ("button-active", "accentStrong"),
        ("button-disabled", "border"),
        ("tab-active", "selected"),
        ("notification", "elevated"),
        ("notification-error", "error"),
        ("misc", "muted"),
        ("highlight", "elevated"),
        ("highlight-elevated", "selected"),
    ] {
        result.push_str(&format!(
            "{key} = {}\n",
            if value.is_empty() {
                "000000"
            } else {
                &role(p, value)?[1..]
            }
        ));
    }
    Ok(result)
}
fn btop(p: &Value) -> Result<String> {
    let mut result = String::from("# Desktop Foundation; generated from semantic palette roles.\n");
    let mut roles: Vec<(String, &str)> = [
        ("main_bg", "background"),
        ("main_fg", "foreground"),
        ("title", "foreground"),
        ("hi_fg", "accent"),
        ("selected_bg", "selected"),
        ("selected_fg", "foreground"),
        ("inactive_fg", "muted"),
        ("graph_text", "muted"),
        ("meter_bg", "border"),
        ("proc_misc", "accent"),
        ("cpu_box", "border"),
        ("mem_box", "border"),
        ("net_box", "border"),
        ("proc_box", "border"),
        ("div_line", "border"),
        ("proc_pause_bg", "selected"),
        ("proc_follow_bg", "selected"),
        ("proc_banner_bg", "elevated"),
        ("proc_banner_fg", "foreground"),
        ("followed_bg", "selected"),
        ("followed_fg", "foreground"),
    ]
    .into_iter()
    .map(|(a, b)| (a.into(), b))
    .collect();
    for graph in [
        "temp",
        "cpu",
        "free",
        "cached",
        "available",
        "used",
        "download",
        "upload",
        "process",
    ] {
        roles.push((format!("{graph}_start"), "border"));
        roles.push((format!("{graph}_mid"), "accent"));
        roles.push((
            format!("{graph}_end"),
            if ["temp", "cpu", "used", "process"].contains(&graph) {
                "error"
            } else {
                "accentStrong"
            },
        ));
    }
    for (key, value) in roles {
        result.push_str(&format!("theme[{key}]=\"{}\"\n", role(p, value)?));
    }
    Ok(result)
}
pub fn brave(p: &Value) -> Result<Value> {
    let mut colors = serde_json::Map::new();
    for (key, value) in [
        ("frame", "background"),
        ("frame_inactive", "background"),
        ("toolbar", "elevated"),
        ("toolbar_text", "foreground"),
        ("tab_text", "foreground"),
        ("tab_background_text", "muted"),
        ("tab_background_text_inactive", "muted"),
        ("background_tab", "background"),
        ("background_tab_inactive", "background"),
        ("bookmark_text", "foreground"),
        ("button_background", "selected"),
        ("toolbar_button_icon", "accent"),
        ("ntp_background", "background"),
        ("ntp_text", "foreground"),
        ("ntp_link", "accent"),
        ("omnibox_background", "background"),
        ("omnibox_text", "foreground"),
    ] {
        let c = role(p, value)?;
        colors.insert(
            key.into(),
            json!([1, 3, 5].map(|i| u8::from_str_radix(&c[i..i + 2], 16).expect("validated hex"))),
        );
    }
    let digest = Sha256::digest(serde_json::to_vec(p)?);
    let version = format!(
        "1.{}.{}.{}",
        u16::from_be_bytes([digest[0], digest[1]]),
        u16::from_be_bytes([digest[2], digest[3]]),
        u16::from_be_bytes([digest[4], digest[5]])
    );
    Ok(
        json!({"manifest_version":3,"name":"Desktop Foundation Graphite","version":version,"description":"Desktop Foundation wallpaper theme","theme":{"colors":colors}}),
    )
}
pub fn render(root: &Path, target: &Path, p: &Value, profile: &str, reset: bool) -> Result<()> {
    for name in ["kitty", "fish", "fastfetch"] {
        copy_tree(
            &root.join("terminal").join(name),
            &target.join("terminal").join(name),
        )?;
    }
    let backend = shell_words::quote(
        &root
            .join("native/foundation/target/release/desktop-foundationctl")
            .to_string_lossy(),
    )
    .into_owned();
    let owner = shell_words::quote(&root.to_string_lossy()).into_owned();
    write(
        &target.join("terminal/fastfetch/packages"),
        &format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nsource_path=${{BASH_SOURCE[0]}}\n[[ $source_path == */* ]] || source_path=./$source_path\nexec {backend} --root {owner} system packages --config \"${{source_path%/*}}/packages.jsonc\" \"$@\"\n"
        ),
    )?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        target.join("terminal/fastfetch/packages"),
        fs::Permissions::from_mode(0o755),
    )?;
    let mut t = read(&root.join("theme/fallback/terminal.json"))?;
    if !reset {
        for key in ["background", "foreground", "muted", "accent", "error"] {
            t[key] = json!(&role(p, key)?[1..]);
        }
        t["selection"] = json!(&role(p, "selected")?[1..]);
    }
    write_json(&target.join("terminal/palette.json"), &t)?;
    fs::copy(
        root.join("theme/fallback/fastfetch.json"),
        target.join("terminal/fastfetch/config.jsonc"),
    )?;
    terminal(target)?;
    let mut notifications = fs::read_to_string(root.join("theme/fallback/notifications.conf"))?;
    if !reset {
        for (old, key) in [
            ("#171b22f5", "background"),
            ("#b7bec9", "foreground"),
            ("#2b323d", "border"),
            ("#dce2ea", "foreground"),
        ] {
            let mut value = role(p, key)?.to_string();
            if old.ends_with("f5") {
                value.push_str("f5");
            }
            notifications = notifications.replace(old, &value);
        }
    }
    write(&target.join("notifications.conf"), &notifications)?;
    write_json(&target.join("semantic.json"), p)?;
    write(&target.join("niri.kdl"), &niri(root, profile, Some(p))?)?;
    write_json(&target.join("brave/manifest.json"), &brave(p)?)?;
    write_json(
        &target.join("brave/policy.json"),
        &json!({"BrowserThemeColor":role(p,"elevated")?}),
    )?;
    write_json(
        &target.join("gtk.json"),
        &json!({"colorScheme":"prefer-dark","accent":theme::gtk_accent(p)?}),
    )?;
    write(&target.join("spotify/color.ini"), &spotify_colors(p)?)?;
    write(&target.join("btop/desktop-foundation.theme"), &btop(p)?)?;
    write_json(
        &target.join("adapters.json"),
        &read(&root.join("theme/adapters.json"))?,
    )
}
