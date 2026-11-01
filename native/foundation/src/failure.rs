//! Explicit, supervised shell recovery checks. Never exits the compositor.
use crate::{Result, invalid, process};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn command(args: &[String]) -> Result<process::Output> {
    process::run(args, None, Duration::from_secs(10), true)
}
fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}
fn checked(items: &[&str]) -> Result<process::Output> {
    let command = args(items);
    process::checked(&command, None, Duration::from_secs(10), true)
}
fn identities(output: &[u8], key: &str) -> Result<BTreeSet<String>> {
    let value: Value = serde_json::from_slice(output)?;
    let windows = value
        .as_array()
        .ok_or_else(|| invalid("Invalid compositor window list"))?;
    windows
        .iter()
        .map(|window| {
            let id = window
                .get(key)
                .filter(|v| v.is_number() || v.is_string())
                .ok_or_else(|| invalid("Window identity missing"))?;
            Ok(id.to_string())
        })
        .collect()
}
struct Temporary(PathBuf);
impl Temporary {
    fn create() -> Result<Self> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("Invalid clock"))?
            .as_nanos();
        for n in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "foundation-failure-{}-{now}-{n}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(invalid("Cannot create isolated failure fixture"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn shell_pid() -> Result<u32> {
    let output = checked(&[
        "systemctl",
        "--user",
        "show",
        "-p",
        "MainPID",
        "--value",
        "desktop-foundation-shell.service",
    ])?;
    let pid: u32 = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Invalid shell PID"))?
        .trim()
        .parse()
        .map_err(|_| invalid("Invalid shell PID"))?;
    if pid < 2 {
        return Err(invalid("No running supervised shell"));
    }
    Ok(pid)
}

pub fn run(root: &Path, options: &[String]) -> Result<()> {
    if !options.is_empty() {
        return Err(invalid("failure-check takes no arguments"));
    }
    let niri = std::env::var_os("NIRI_SOCKET").is_some_and(|v| !v.is_empty());
    let query = if niri {
        vec!["niri", "msg", "-j", "windows"]
    } else {
        vec!["hyprctl", "-j", "clients"]
    };
    let key = if niri { "id" } else { "address" };
    let before = identities(&checked(&query)?.stdout, key)?;
    let pid = shell_pid()?;
    let metadata = fs::metadata(format!("/proc/{pid}"))?;
    // SAFETY: geteuid has no arguments or pointer access.
    if metadata.uid() != unsafe { libc::geteuid() } || pid == std::process::id() {
        return Err(invalid(
            "Supervised shell PID does not belong to the current user",
        ));
    }
    // SAFETY: the supervised PID was read from the current user's service and validated.
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut recovered = false;
    while Instant::now() < deadline {
        if let Ok(new_pid) = shell_pid()
            && new_pid != pid
        {
            let request = vec![
                "quickshell".into(),
                "ipc".into(),
                "--pid".into(),
                new_pid.to_string(),
                "call".into(),
                "foundation".into(),
                "status".into(),
            ];
            if let Ok(result) = command(&request)
                && result.status.success()
                && serde_json::from_slice::<Value>(&result.stdout)
                    .ok()
                    .and_then(|v| v.get("ready").and_then(Value::as_bool))
                    == Some(true)
            {
                recovered = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    if !recovered {
        return Err(invalid("Supervised shell did not recover"));
    }
    let after = identities(&checked(&query)?.stdout, key)?;
    if !before.is_subset(&after) {
        return Err(invalid("Existing clients must survive shell failure"));
    }
    let runner = root
        .join("scripts/run-shell")
        .to_string_lossy()
        .into_owned();
    let mut overrides = vec![vec!["DF_COMPOSITOR=unsupported"]];
    overrides.push(if niri {
        vec![
            "DF_COMPOSITOR=niri",
            "NIRI_SOCKET=/tmp/nonexistent-niri-socket",
        ]
    } else {
        vec![
            "DF_COMPOSITOR=hyprland",
            "HYPRLAND_INSTANCE_SIGNATURE=nonexistent-test-instance",
        ]
    });
    for values in overrides {
        let mut invocation = vec!["env".to_string()];
        invocation.extend(values.into_iter().map(String::from));
        invocation.push(runner.clone());
        let output = command(&invocation)?;
        let error = String::from_utf8_lossy(&output.stderr);
        if output.status.success()
            || !(error.contains("unavailable") || error.contains("Unsupported"))
        {
            return Err(invalid(
                "Expected unavailable or unsupported compositor error",
            ));
        }
    }
    let temp = Temporary::create()?;
    fs::write(
        temp.0.join("shell.qml"),
        "import Quickshell\nShellRoot { invalid syntax !!! }",
    )?;
    let invocation = vec![
        "quickshell".into(),
        "--path".into(),
        temp.0.to_string_lossy().into_owned(),
        "--no-color".into(),
    ];
    let output = command(&invocation)?;
    if output.status.success()
        || !format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .contains("ERROR")
    {
        return Err(invalid(
            "Invalid isolated shell configuration was not rejected",
        ));
    }
    let monitors = if niri {
        vec!["niri", "msg", "-j", "outputs"]
    } else {
        vec!["hyprctl", "-j", "monitors"]
    };
    checked(&monitors)?;
    println!(
        "PASS: supervised crash/restart, clients survive, unavailable IPC, unsupported compositor, isolated syntax failure"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_identity_validation() {
        assert_eq!(
            identities(br#"[{"id":2},{"id":1}]"#, "id").unwrap(),
            BTreeSet::from(["1".into(), "2".into()])
        );
        assert!(identities(br#"[{"other":1}]"#, "id").is_err());
        assert!(identities(br#"{}"#, "id").is_err());
    }
}
