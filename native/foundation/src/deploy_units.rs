//! Deployment-time session definitions. Login never generates these files.
use crate::{Result, invalid, local_ai, state as durable, theme_render};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};
pub type Target = (PathBuf, PathBuf);
pub fn quote(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('\n', "\\n")
    )
}
fn executable(name: &str, fallback: PathBuf) -> PathBuf {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|p| env::split_paths(&p).collect::<Vec<_>>())
        .map(|p| p.join(name))
        .find(|p| p.is_file())
        .unwrap_or(fallback)
}
pub fn definitions(root: &Path, state: &Path) -> Vec<(String, String)> {
    let common = "[Unit]\nPartOf=desktop-foundation-session.target graphical-session.target\nConditionEnvironment=WAYLAND_DISPLAY\nStartLimitIntervalSec=60\nStartLimitBurst=3\n";
    let mako = executable("mako", state.join("bin/mako"));
    let mut units=vec![
 ("desktop-foundation-recording.socket".into(),"[Unit]\nDescription=Desktop Foundation recording requests\nPartOf=niri.service graphical-session.target\n[Socket]\nListenDatagram=%t/desktop-foundation-recording/requests.sock\nSocketMode=0600\nDirectoryMode=0700\nRemoveOnStop=yes\n".into()),
 ("desktop-foundation-recording.service".into(),format!("[Unit]\nDescription=Desktop Foundation recording owner\nPartOf=niri.service graphical-session.target\nRequires=desktop-foundation-recording.socket\nAfter=desktop-foundation-recording.socket\nStartLimitIntervalSec=0\n[Service]\nType=exec\nExecStart={} serve\nExecStopPost={} cleanup\nRestart=no\nKillMode=mixed\nKillSignal=SIGINT\nTimeoutStopSec=75\nUMask=0077\n",quote(&root.join("scripts/record")),quote(&root.join("scripts/record")))),
 ("desktop-foundation-transparency.socket".into(),"[Unit]\nDescription=Desktop Foundation Transparency requests\nPartOf=niri.service graphical-session.target\n[Socket]\nListenDatagram=%t/desktop-foundation-transparency.sock\nSocketMode=0600\nRemoveOnStop=yes\n".into()),
 ("desktop-foundation-transparency.service".into(),format!("[Unit]\nDescription=Desktop Foundation Transparency audio owner\nPartOf=niri.service graphical-session.target\nRequires=desktop-foundation-transparency.socket\nAfter=desktop-foundation-transparency.socket\nStartLimitIntervalSec=0\n[Service]\nType=exec\nExecStart={} serve\nExecStopPost={} recover\nTimeoutStopSec=8\nUMask=0077\n",quote(&root.join("scripts/transparency")),quote(&root.join("scripts/transparency")))),
 ("desktop-foundation-replay-start.service".into(),format!("[Unit]\nDescription=Desktop Foundation configured replay startup\nConditionEnvironment=NIRI_SOCKET\nConditionEnvironment=WAYLAND_DISPLAY\nPartOf=niri.service graphical-session.target\nAfter=desktop-foundation-recording.socket\nRequires=desktop-foundation-recording.socket\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart={} autostart\nUMask=0077\n",quote(&root.join("scripts/record")))),
 ("desktop-foundation-monitor.service".into(),format!("[Unit]\nDescription=Desktop Foundation btop sidebar\nPartOf=niri.service graphical-session.target\nAfter=niri.service\nConditionEnvironment=NIRI_SOCKET\nStartLimitIntervalSec=0\n[Service]\nType=exec\nExecStart={} serve\nRestart=no\nKillMode=mixed\nSendSIGHUP=no\nTimeoutStopSec=3\nUMask=0077\n",quote(&root.join("scripts/monitor")))),
 ("desktop-foundation-session.target".into(),"[Unit]\nDescription=Desktop foundation interactive session\nPartOf=graphical-session.target\nWants=desktop-foundation-replay-start.service desktop-foundation-shell.service desktop-foundation-notifications.service desktop-foundation-clipboard-init.service desktop-foundation-clipboard.service hyprpolkitagent.service\n".into()),
 ("desktop-foundation-shell.service".into(),format!("{common}[Service]\nType=exec\nExecStart={}\nRestart=on-failure\nRestartSec=1\n",quote(&root.join("scripts/run-shell")))),
 ("desktop-foundation-clipboard-init.service".into(),format!("{common}[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart={} --root {} clipboard init\nUMask=0077\n",quote(&root.join("native/foundation/target/release/desktop-foundationctl")),quote(root))),
 ("desktop-foundation-clipboard.service".into(),format!("{common}Requires=desktop-foundation-clipboard-init.service\nAfter=desktop-foundation-clipboard-init.service\n[Service]\nType=exec\nExecStart={}\nRestart=on-failure\nRestartSec=1\nUMask=0077\n",quote(&root.join("scripts/clipboard-watch")))),
 ("desktop-foundation-notifications.service".into(),format!("{common}ConditionEnvironment=NIRI_SOCKET\n[Service]\nType=dbus\nBusName=org.freedesktop.Notifications\nExecCondition={}\nExecStart={} --config {}\nRestart=on-failure\nRestartSec=1\n",quote(&root.join("scripts/notification-owner-check")),quote(&mako),quote(&state.join("theme/current/notifications.conf"))))];
    for (name, script) in [
        ("wallpaper", "niri-wallpaper-start"),
        ("clipboard-persist", "clipboard-persist"),
    ] {
        units.push((format!("desktop-foundation-{name}.service"),format!("[Unit]\nDescription=Desktop foundation Niri {name}\nPartOf=niri.service graphical-session.target\nAfter=niri.service\nConditionEnvironment=NIRI_SOCKET\nStartLimitIntervalSec=60\nStartLimitBurst=3\n[Service]\nExecStart={}\nRestart=on-failure\nRestartSec=1\nUMask=0077\n[Install]\nWantedBy=niri.service\n",quote(&root.join("scripts").join(script)))));
    }
    units
}
fn keyring_source(root: &Path, profile: &str, name: &str) -> Result<PathBuf> {
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid keyring profile name"));
    }
    let candidate = root
        .join("profiles")
        .join(profile)
        .join("session/keyring")
        .join(name);
    match fs::symlink_metadata(&candidate) {
        Ok(info) if info.is_file() => Ok(candidate),
        Ok(_) => Err(invalid("Profile keyring override must be a regular file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(root.join("session/keyring").join(name))
        }
        Err(error) => Err(error.into()),
    }
}
pub fn targets(
    root: &Path,
    config: &Path,
    state: &Path,
    data: &Path,
    recorded: &HashSet<PathBuf>,
    write: bool,
    profile: &str,
) -> Result<Vec<Target>> {
    let generated = state.join("session-units");
    if write {
        durable::private_directory(&generated)?;
    }
    let mut targets = vec![(
        data.join("wallpapers/wallhaven-135w7w.png"),
        root.join("wallpapers/wallhaven-135w7w.png"),
    )];
    for (name, content) in definitions(root, state) {
        let source = generated.join(&name);
        if write && fs::read_to_string(&source).ok().as_deref() != Some(&content) {
            theme_render::write(&source, &content)?;
        }
        targets.push((config.join("systemd/user").join(&name), source.clone()));
        if [
            "desktop-foundation-wallpaper.service",
            "desktop-foundation-clipboard-persist.service",
        ]
        .contains(&name.as_str())
        {
            targets.push((
                config.join("systemd/user/niri.service.wants").join(&name),
                source,
            ));
        }
    }
    for name in [
        "gnome-keyring-pkcs11.desktop",
        "gnome-keyring-secrets.desktop",
    ] {
        targets.push((
            config.join("autostart").join(name),
            keyring_source(root, profile, name)?,
        ));
    }
    for (name, source) in [
        (
            "default.target.wants/oo7-daemon.service",
            "/usr/lib/systemd/user/oo7-daemon.service",
        ),
        ("gnome-keyring-daemon.service", "/dev/null"),
        ("gnome-keyring-daemon.socket", "/dev/null"),
    ] {
        let path = config.join("systemd/user").join(name);
        let source = PathBuf::from(source);
        if !recorded.contains(&path) && fs::read_link(&path).ok().as_ref() == Some(&source) {
            continue;
        }
        targets.push((path, source));
    }
    targets.extend([
        (
            config.join("systemd/user/mako.service"),
            config.join("systemd/user/desktop-foundation-notifications.service"),
        ),
        (
            config.join("autostart/blueman.desktop"),
            root.join("session/bluetooth/blueman.desktop"),
        ),
        (
            config.join("autostart/arch-update-tray.desktop"),
            root.join("session/autostart/arch-update-tray.desktop"),
        ),
        (
            data.join("dbus-1/services/org.freedesktop.secrets.service"),
            keyring_source(root, profile, "org.freedesktop.secrets.service")?,
        ),
    ]);
    let path = data.join("applications/desktop-foundation-odysseus.desktop");
    let source = state.join("launchers/odysseus.desktop");
    let configured = local_ai::plan(root)?["configured"] == true;
    if configured || recorded.contains(&path) {
        if write {
            theme_render::write(
                &source,
                &format!(
                    "[Desktop Entry]\nType=Application\nName=Odysseus\nComment=Open local AI\nExec={} start\nIcon=applications-science\nTerminal=false\nCategories=Utility;\nKeywords=Ollama;Odysseus;Odusseys;local;AI;\n{}",
                    quote(&root.join("scripts/local-ai")),
                    if configured { "" } else { "Hidden=true\n" }
                ),
            )?;
        }
        targets.push((path, source));
    }
    Ok(targets)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_systemd_paths_without_interpreting_specifiers() {
        assert_eq!(
            quote(Path::new("/a %name/with \"quotes\"")),
            "\"/a %%name/with \\\"quotes\\\"\""
        );
    }
    #[test]
    fn session_does_not_start_optional_sidebar_or_ai() {
        let units = definitions(Path::new("/source"), Path::new("/state"));
        let target = &units
            .iter()
            .find(|(name, _)| name == "desktop-foundation-session.target")
            .unwrap()
            .1;
        assert!(!target.contains("monitor.service"));
        assert!(!target.contains("ollama"));
        assert!(!target.contains("odysseus"));
        assert!(target.contains("clipboard.service"));
    }
    #[test]
    fn configured_replay_startup_uses_the_same_socket_owner() {
        let units = definitions(Path::new("/source"), Path::new("/state"));
        let unit = &units
            .iter()
            .find(|(n, _)| n == "desktop-foundation-replay-start.service")
            .unwrap()
            .1;
        assert!(unit.contains("Requires=desktop-foundation-recording.socket"));
        assert!(unit.contains("autostart"));
        assert!(!unit.contains("gpu-screen-recorder"));
        let target = &units
            .iter()
            .find(|(n, _)| n == "desktop-foundation-session.target")
            .unwrap()
            .1;
        assert!(target.contains("desktop-foundation-replay-start.service"));
    }
}
