//! Reusable fixed actions for present and future UI consumers. No shell strings.
use crate::{Result, apps, invalid, process, volume};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvocationPolicy {
    Immediate,
    SessionChange,
}

#[derive(Serialize)]
pub struct DesktopAction {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub keywords: &'static [&'static str],
    pub invocation_policy: InvocationPolicy,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Copy)]
struct ActionSpec {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    icon: &'static str,
    category: &'static str,
    keywords: &'static [&'static str],
    invocation_policy: InvocationPolicy,
}
const SPECS: &[ActionSpec] = &[
    ActionSpec {
        id: "apps.terminal",
        label: "Open terminal",
        description: "Open the configured terminal",
        icon: "utilities-terminal",
        category: "Applications",
        keywords: &["terminal", "command"],
        invocation_policy: InvocationPolicy::Immediate,
    },
    ActionSpec {
        id: "apps.browser",
        label: "Open browser",
        description: "Open the configured browser",
        icon: "web-browser",
        category: "Applications",
        keywords: &["browser", "web"],
        invocation_policy: InvocationPolicy::Immediate,
    },
    ActionSpec {
        id: "apps.files",
        label: "Open files",
        description: "Open your home directory",
        icon: "system-file-manager",
        category: "Applications",
        keywords: &["files", "home"],
        invocation_policy: InvocationPolicy::Immediate,
    },
    ActionSpec {
        id: "volume.up",
        label: "Volume up",
        description: "Increase output volume by 3%, capped at 100%",
        icon: "audio-volume-high",
        category: "Audio",
        keywords: &["audio", "volume"],
        invocation_policy: InvocationPolicy::Immediate,
    },
    ActionSpec {
        id: "volume.down",
        label: "Volume down",
        description: "Decrease output volume by 3%",
        icon: "audio-volume-low",
        category: "Audio",
        keywords: &["audio", "volume"],
        invocation_policy: InvocationPolicy::Immediate,
    },
    ActionSpec {
        id: "power.suspend",
        label: "Suspend",
        description: "Suspend using the native session manager",
        icon: "system-suspend",
        category: "Session",
        keywords: &["sleep", "suspend"],
        invocation_policy: InvocationPolicy::SessionChange,
    },
    ActionSpec {
        id: "power.logout",
        label: "Log out",
        description: "End the current compositor session",
        icon: "system-log-out",
        category: "Session",
        keywords: &["logout", "session"],
        invocation_policy: InvocationPolicy::SessionChange,
    },
    ActionSpec {
        id: "power.reboot",
        label: "Reboot",
        description: "Restart the computer",
        icon: "system-reboot",
        category: "Session",
        keywords: &["reboot", "restart"],
        invocation_policy: InvocationPolicy::SessionChange,
    },
    ActionSpec {
        id: "power.poweroff",
        label: "Power off",
        description: "Shut down the computer",
        icon: "system-shutdown",
        category: "Session",
        keywords: &["shutdown", "power"],
        invocation_policy: InvocationPolicy::SessionChange,
    },
];
pub fn power_command(root: &Path, action: &str) -> Result<Vec<String>> {
    match action {
        "suspend" | "reboot" | "poweroff" => Ok(vec!["systemctl".into(), action.into()]),
        "logout" => Ok(vec![
            root.join("scripts/session-exit")
                .to_string_lossy()
                .into_owned(),
        ]),
        _ => Err(invalid(format!("Unknown power action: {action}"))),
    }
}

pub fn plan(root: &Path, id: &str) -> Result<Vec<String>> {
    if !SPECS.iter().any(|s| s.id == id) {
        return Err(invalid(format!("Unknown action: {id}")));
    }
    let (group, action) = id
        .split_once('.')
        .ok_or_else(|| invalid("Invalid action ID"))?;
    match group {
        "apps" => apps::command(root, action),
        "power" => power_command(root, action),
        "volume" => Ok(vec![
            root.join("scripts/foundation")
                .to_string_lossy()
                .into_owned(),
            "volume".into(),
            action.into(),
        ]),
        _ => Err(invalid("Unknown action category")),
    }
}

pub fn list(root: &Path) -> Vec<DesktopAction> {
    SPECS
        .iter()
        .map(
            |&ActionSpec {
                 id,
                 label,
                 description,
                 icon,
                 category,
                 keywords,
                 invocation_policy,
             }| {
                let availability: Result<()> = if id.starts_with("volume.") {
                    if apps::executable("wpctl") {
                        Ok(())
                    } else {
                        Err(invalid("wpctl is unavailable"))
                    }
                } else {
                    plan(root, id).and_then(|p| {
                        if p.first().is_some_and(|s| apps::executable(s)) {
                            Ok(())
                        } else {
                            Err(invalid("Action executable is unavailable"))
                        }
                    })
                };
                DesktopAction {
                    id,
                    label,
                    description,
                    icon,
                    category,
                    keywords,
                    invocation_policy,
                    available: availability.is_ok(),
                    unavailable_reason: availability.err().map(|e| e.to_string()),
                }
            },
        )
        .collect()
}

pub fn invoke(root: &Path, id: &str) -> Result<()> {
    let command = plan(root, id)?;
    if let Some(direction) = id.strip_prefix("volume.") {
        volume::adjust(root, direction)
    } else {
        process::replace(&command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_routing_and_registry_ids_are_fixed() {
        assert_eq!(
            power_command(Path::new("/checkout"), "logout").unwrap(),
            ["/checkout/scripts/session-exit"]
        );
        assert_eq!(
            power_command(Path::new("/checkout"), "poweroff").unwrap(),
            ["systemctl", "poweroff"]
        );
        assert!(power_command(Path::new("/checkout"), "reboot; touch /tmp/no").is_err());
        assert!(plan(Path::new("/checkout"), "unknown").is_err());
        let ids: std::collections::BTreeSet<_> = SPECS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), SPECS.len());
    }
}
