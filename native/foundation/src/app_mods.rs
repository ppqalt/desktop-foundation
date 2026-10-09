//! One-shot configured application add-ons; each adapter retains its own rollback.
use crate::{Result, apps, geode, invalid, process, state};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::Read, path::Path, time::Duration};

const IDS: [&str; 2] = ["spicetify", "geode"];

fn settings(root: &Path) -> Result<BTreeMap<String, bool>> {
    let mut selected = BTreeMap::new();
    let override_file =
        state::xdg("XDG_CONFIG_HOME", ".config")?.join("desktop-foundation/app-mods.json");
    for file in [root.join("config/app-mods.json"), override_file] {
        let stream = match fs::File::open(&file) {
            Ok(stream) => stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && file != root.join("config/app-mods.json") =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        stream.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 16 * 1024 {
            return Err(invalid(
                "Application modification configuration exceeds 16 KiB",
            ));
        }
        let entries: BTreeMap<String, Value> = serde_json::from_slice(&bytes)?;
        for (id, entry) in entries {
            if !IDS.contains(&id.as_str()) || entry.as_object().is_none_or(|entry| entry.len() != 1)
            {
                return Err(invalid(
                    "Unknown application modification or configuration field",
                ));
            }
            selected.insert(
                id,
                entry["enabled"]
                    .as_bool()
                    .ok_or_else(|| invalid("Application modification enabled must be a boolean"))?,
            );
        }
    }
    Ok(selected)
}

fn dependencies(ids: &[&str]) -> Vec<&'static str> {
    if ids.contains(&"geode") {
        vec!["protontricks", "libarchive"]
    } else {
        Vec::new()
    }
}

pub fn run(root: &Path, arguments: &[String]) -> Result<Value> {
    let action = arguments.first().map(String::as_str).unwrap_or("install");
    if !["install", "check", "restore", "plan"].contains(&action) {
        return Err(invalid(
            "Expected app-mods install|check|restore|plan [spicetify|geode] [--skip-unavailable]",
        ));
    }
    // SAFETY: geteuid takes no arguments and retains no pointers.
    if action != "plan" && unsafe { libc::geteuid() } == 0 {
        return Err(invalid(
            "Run application modifications as your normal desktop user",
        ));
    }
    let config = settings(root)?;
    let mut ids = Vec::new();
    let mut skip = arguments.is_empty();
    for argument in arguments.iter().skip(1) {
        if argument == "--skip-unavailable" {
            skip = true;
        } else if IDS.contains(&argument.as_str()) && !ids.contains(&argument.as_str()) {
            ids.push(argument.as_str());
        } else {
            return Err(invalid("Unknown or duplicate application modification"));
        }
    }
    if ids.is_empty() {
        ids.extend(
            IDS.into_iter()
                .filter(|id| config.get(*id).copied().unwrap_or(false)),
        );
    }
    if action == "plan" {
        return Ok(json!({"modifications":ids,"packages":dependencies(&ids)}));
    }
    if action == "restore" {
        ids.reverse();
    }
    let mut results = Vec::new();
    for id in ids {
        eprintln!("Application modification: {id} {action}");
        if id == "spicetify" {
            process::checked(
                &[
                    root.join("scripts/spotify-setup")
                        .to_string_lossy()
                        .into_owned(),
                    action.into(),
                ],
                None,
                Duration::from_secs(900),
                true,
            )?;
            results
                .push(json!({"id":id,"status":if action=="restore" {"restored"} else {"ready"}}));
            continue;
        }
        if !apps::executable("protontricks") {
            if skip {
                results.push(json!({"id":id,"status":"pending","reason":"Install protontricks and libarchive, then run the add-on installer after Geometry Dash's first Steam launch"}));
                continue;
            }
            return Err(invalid(
                "Install protontricks and libarchive before configuring Geode",
            ));
        }
        if action == "restore" {
            results.push(json!({"id":id,"result":geode::run(root,&["restore".into()])?}));
            continue;
        }
        let status = match geode::run(root, &["status".into()]) {
            Ok(status) => status,
            Err(error)
                if skip
                    && error
                        .to_string()
                        .contains("Protontricks could not access Steam Geometry Dash") =>
            {
                results.push(json!({"id":id,"status":"pending","reason":"Install Geometry Dash in Steam and run it once, then rerun install-app-mods"}));
                continue;
            }
            Err(error) => return Err(error),
        };
        if status["supportedGameBuild"] != true && skip {
            results.push(json!({"id":id,"status":"pending","reason":"The Geode pin does not support this Geometry Dash build"}));
            continue;
        }
        if status["status"] == "installed"
            && status["matchesPinnedVersion"] == true
            && status["supportedGameBuild"] == true
        {
            results.push(json!({"id":id,"status":"ready"}));
        } else if action == "check" {
            if skip
                && ["not-installed", "restored"].contains(&status["status"].as_str().unwrap_or(""))
            {
                results.push(
                    json!({"id":id,"status":"pending","reason":"Geode has not been applied yet"}),
                );
            } else {
                return Err(invalid(
                    "Geode needs installation or transaction recovery; use geode-setup status",
                ));
            }
        } else {
            results.push(json!({"id":id,"result":geode::run(root,&["install".into()])?}));
        }
    }
    Ok(json!({"action":action,"modifications":results}))
}
