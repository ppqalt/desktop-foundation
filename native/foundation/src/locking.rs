//! On-demand locker supervision. No passwords, PAM calls or persistent worker.
use crate::{Result, invalid, process};
use serde::Deserialize;
use std::{
    fs,
    io::{BufRead, BufReader},
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const SECURE: &str = "DF_LOCK_SECURE";
const UNLOCKED: &str = "DF_LOCK_AUTHENTICATED";
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    #[serde(default)]
    idle_lock_seconds: u32,
}
fn settings(root: &Path) -> Result<Settings> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let path = config.join("desktop-foundation/session-security.toml");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::read_to_string(root.join("config/session-security.toml"))?
        }
        Err(e) => return Err(e.into()),
    };
    parse_settings(&text)
}
fn parse_settings(text: &str) -> Result<Settings> {
    let value: Settings = toml::from_str(text)
        .map_err(|e| invalid(format!("Invalid session security settings: {e}")))?;
    if value.idle_lock_seconds != 0 && !(30..=86400).contains(&value.idle_lock_seconds) {
        return Err(invalid("idle_lock_seconds must be 0 or 30..86400"));
    }
    Ok(value)
}
fn systemctl(args: &[&str]) -> Result<()> {
    let mut command = vec!["systemctl".into(), "--user".into()];
    command.extend(args.iter().map(|s| (*s).into()));
    process::checked(&command, None, Duration::from_secs(15), true)?;
    Ok(())
}
fn ready(fallback: bool) -> Result<()> {
    process::checked(
        &[
            "systemd-notify".into(),
            "--ready".into(),
            if fallback {
                "--status=Fallback locker securely locked".into()
            } else {
                "--status=Session securely locked".into()
            },
        ],
        None,
        Duration::from_secs(2),
        true,
    )?;
    Ok(())
}
/// A private socketpair supplies swaylock's actual ready-fd. Process startup is
/// never accepted as proof of compositor coverage.
fn fallback() -> Result<()> {
    let (mut reader, writer) = UnixStream::pair()?;
    let fd = writer.as_raw_fd();
    // SAFETY: fd is live and owned here; only this inherited endpoint loses CLOEXEC.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut child = Command::new("swaylock")
        .args([
            "--config",
            "/dev/null",
            "--font",
            "Google Sans Flex",
            "--color",
            "101014",
            "--show-failed-attempts",
            "--ready-fd",
            &fd.to_string(),
        ])
        .stdin(Stdio::null())
        .spawn()?;
    drop(writer);
    reader.set_read_timeout(Some(Duration::from_secs(4)))?;
    let mut byte = [0u8];
    use std::io::Read;
    if reader.read_exact(&mut byte).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(invalid(
            "Fallback locker did not confirm secure coverage; suspend denied",
        ));
    }
    ready(true)?;
    if !child.wait()?.success() {
        return Err(invalid(
            "Fallback locker failed; Niri must retain the locked session",
        ));
    }
    Ok(())
}
fn serve(root: &Path) -> Result<()> {
    // This is its own Quickshell configuration: ordinary shell reloads/crashes
    // never destroy the lock. File watching is disabled in the QML entry point.
    let mut child = match Command::new("quickshell")
        .args(["--no-color", "--path"])
        .arg(root.join("shell/lock.qml"))
        .env("DF_FOUNDATION_ROOT", root)
        .env("QT_QUICK_CONTROLS_STYLE", "Basic")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return fallback(),
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("Locker output unavailable"))?;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            // These are fixed markers from the owned config; never log PAM data.
            let marker = if line.ends_with(SECURE) {
                Some(SECURE)
            } else if line.ends_with(UNLOCKED) {
                Some(UNLOCKED)
            } else {
                None
            };
            if let Some(marker) = marker {
                let _ = send.send(marker);
            }
        }
        let _ = send.send("DF_LOCK_PROCESS_CLOSED");
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut secured = false;
    let mut authenticated = false;
    loop {
        match receive.recv_timeout(if secured {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(20)
        }) {
            Ok(SECURE) if !secured => {
                ready(false)?;
                secured = true;
            }
            Ok(UNLOCKED) if secured => authenticated = true,
            Ok("DF_LOCK_PROCESS_CLOSED") => {
                let status = child.wait()?;
                return if status.success() && authenticated {
                    Ok(())
                } else {
                    fallback()
                };
            }
            _ => (),
        }
        if let Some(status) = child.try_wait()? {
            // Drain already-written proof before deciding whether exit was an
            // authenticated dismissal. Reader scheduling is not authentication.
            while let Ok(marker) = receive.recv_timeout(Duration::from_millis(100)) {
                if marker == UNLOCKED && secured {
                    authenticated = true;
                }
                if marker == "DF_LOCK_PROCESS_CLOSED" {
                    break;
                }
            }
            // An exit code alone is never an authentication result.
            if status.success() && authenticated {
                return Ok(());
            }
            return fallback();
        }
        if !secured && Instant::now() >= deadline {
            child.kill()?;
            let _ = child.wait();
            return fallback();
        }
    }
}
fn ensure(root: &Path) -> Result<()> {
    systemctl(&["start", "desktop-foundation-lock.service"])?;
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let reply = process::run(
            &[
                "quickshell".into(),
                "ipc".into(),
                "--path".into(),
                root.join("shell/lock.qml").to_string_lossy().into_owned(),
                "call".into(),
                "sessionLock".into(),
                "ensureLocked".into(),
            ],
            None,
            Duration::from_secs(1),
            true,
        )?;
        if reply.status.success() && String::from_utf8_lossy(&reply.stdout).trim() == "secure" {
            return Ok(());
        }
        let fallback = process::checked(
            &[
                "systemctl".into(),
                "--user".into(),
                "show".into(),
                "desktop-foundation-lock.service".into(),
                "--property=ActiveState".into(),
                "--property=StatusText".into(),
            ],
            None,
            Duration::from_secs(1),
            true,
        )?;
        let status = String::from_utf8_lossy(&fallback.stdout);
        if status.lines().any(|s| s == "ActiveState=active")
            && status
                .lines()
                .any(|s| s == "StatusText=Fallback locker securely locked")
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(invalid("Could not confirm session lock"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn watch(root: &Path) -> Result<()> {
    let settings = settings(root)?;
    let lock = shell_words::join([
        root.join("scripts/lock-screen")
            .to_string_lossy()
            .into_owned(),
        "lock".into(),
    ]);
    let mut command = vec!["swayidle".into(), "-w".into(), "lock".into(), lock.clone()];
    if settings.idle_lock_seconds > 0 {
        command.extend([
            "timeout".into(),
            settings.idle_lock_seconds.to_string(),
            lock,
        ]);
    }
    process::replace(&command)
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["lock"] => ensure(root),
        ["serve"] => serve(root),
        ["watch"] => watch(root),
        ["suspend"] => {
            ensure(root)?;
            process::replace(&["systemctl".into(), "suspend".into()])
        }
        ["settings"] => {
            println!(
                "{}",
                serde_json::json!({"idle_lock_seconds": settings(root)?.idle_lock_seconds})
            );
            Ok(())
        }
        _ => Err(invalid(
            "Expected lock-screen lock|serve|watch|suspend|settings",
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_policy_is_opt_in_and_validated() {
        assert_eq!(parse_settings("").unwrap().idle_lock_seconds, 0);
        assert_eq!(
            parse_settings("idle_lock_seconds=300")
                .unwrap()
                .idle_lock_seconds,
            300
        );
        for text in [
            "idle_lock_seconds=1",
            "idle_lock_seconds=-1",
            "idle_lock_seconds=86401",
            "unknown=true",
        ] {
            assert!(parse_settings(text).is_err());
        }
    }
}
