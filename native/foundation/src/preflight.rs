//! Read-only dependency/configuration validation before deployment.
use crate::{Result, apps, invalid, process, theme_render};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut profile = "default";
    let mut compositor = "niri";
    let mut update = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--update" => update = true,
            flag @ ("--profile" | "--compositor") => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| invalid("Missing preflight option value"))?;
                if flag == "--profile" {
                    profile = value;
                } else {
                    compositor = value;
                }
            }
            _ => {
                return Err(invalid(
                    "Expected preflight [--profile NAME] [--compositor niri|hyprland] [--update]",
                ));
            }
        }
        index += 1;
    }
    if !["niri", "hyprland"].contains(&compositor) {
        return Err(invalid("Unknown compositor"));
    }
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid profile"));
    }
    let binary = root.join("native/foundation/target/release/desktop-foundationctl");
    if !binary
        .metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    {
        return Err(invalid(
            "Shared Rust backend is not built. Run scripts/build-backend; nothing deployed.",
        ));
    }
    let wm = if compositor == "niri" {
        "niri"
    } else {
        "Hyprland"
    };
    let required = [
        "pactl",
        "busctl",
        "blueman-manager",
        wm,
        "quickshell",
        "kitty",
        "slangc",
        "fish",
        "fastfetch",
        "lua",
        "wl-copy",
        "wl-paste",
        "wl-clip-persist",
        "swaybg",
        "mako",
        "grim",
        "slurp",
        "wpctl",
        "playerctl",
        "notify-send",
        "dbus-update-activation-environment",
        "gsettings",
        "xdg-mime",
        "fc-cache",
        "pacman-conf",
        "xwayland-satellite",
    ];
    let missing: Vec<_> = required
        .into_iter()
        .filter(|name| !apps::executable(name))
        .collect();
    if update {
        let mandatory = [
            wm,
            "quickshell",
            "kitty",
            "slangc",
            "fish",
            "lua",
            "fc-cache",
        ];
        let blocking: Vec<_> = missing
            .iter()
            .copied()
            .filter(|name| mandatory.contains(name))
            .collect();
        if !blocking.is_empty() {
            return Err(invalid(format!(
                "Missing deployment tools: {}. Install these separately; df-update does not run package managers.",
                blocking.join(", ")
            )));
        }
        if !missing.is_empty() {
            eprintln!("Optional runtime tools unavailable: {}", missing.join(", "));
        }
    } else if !missing.is_empty() {
        return Err(invalid(format!(
            "Missing runtime tools: {}. Run scripts/bootstrap.",
            missing.join(", ")
        )));
    }
    let directory = std::env::temp_dir().join(format!(
        "foundation-preflight-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| invalid(e.to_string()))?
            .as_nanos()
    ));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let scratch = Scratch(directory);
    let file = scratch.0.join(if compositor == "niri" {
        "candidate.kdl"
    } else {
        "candidate.lua"
    });
    let text = if compositor == "niri" {
        theme_render::niri(root, profile, None)?
    } else {
        format!(
            "FOUNDATION_ROOT = {}\nFOUNDATION_PROFILE = {}\ndofile(FOUNDATION_ROOT .. \"/compositor/hyprland/hyprland.lua\")\n",
            serde_json::to_string(&root)?,
            serde_json::to_string(profile)?
        )
    };
    fs::write(&file, text)?;
    let args = if compositor == "niri" {
        vec![
            wm.into(),
            "validate".into(),
            "-c".into(),
            file.to_string_lossy().into_owned(),
        ]
    } else {
        vec![
            wm.into(),
            "--verify-config".into(),
            "-c".into(),
            file.to_string_lossy().into_owned(),
        ]
    };
    process::checked(&args,None,Duration::from_secs(30),true).map_err(|e|invalid(format!("Installed {compositor} cannot validate this configuration; nothing deployed. See docs/installation.md.\n{e}")))?;
    println!("PASS runtime tools and native {compositor} configuration.");
    Ok(())
}
