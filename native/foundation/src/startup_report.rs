//! Finite startup timeline collection. Popup probing is explicit opt-in only.
use crate::{
    Result,
    doctor::{self, Options, Runtime},
    invalid, state,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    thread,
    time::{Duration, Instant},
};
const UNITS: [(&str, &str); 11] = [
    ("niri", "niri.service"),
    ("shell", "desktop-foundation-shell.service"),
    ("notifications", "desktop-foundation-notifications.service"),
    (
        "clipboard_init",
        "desktop-foundation-clipboard-init.service",
    ),
    ("clipboard_capture", "desktop-foundation-clipboard.service"),
    (
        "clipboard_persist",
        "desktop-foundation-clipboard-persist.service",
    ),
    ("wallpaper", "desktop-foundation-wallpaper.service"),
    ("polkit", "hyprpolkitagent.service"),
    ("portal", "xdg-desktop-portal.service"),
    ("psd", "psd.service"),
    ("default", "default.target"),
];
pub fn decode_message(value: &Value) -> String {
    let message = if let Some(s) = value.as_str() {
        s.to_owned()
    } else if let Some(a) = value.as_array() {
        let bytes: Vec<u8> = a
            .iter()
            .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        String::new()
    };
    let mut out = String::new();
    let mut chars = message.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            let original = chars.clone();
            chars.next();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == ';' {
                    chars.next();
                } else {
                    break;
                }
            }
            if chars.peek() == Some(&'m') {
                chars.next();
            } else {
                out.push(c);
                chars = original;
            }
        } else {
            out.push(c);
        }
    }
    out
}
pub fn delta(events: &BTreeMap<String, f64>, start: &str, end: &str) -> Option<f64> {
    Some(((events.get(end)? - events.get(start)?) * 100000.0).round() / 100.0)
}
fn journal(r: &impl Runtime, user: bool, notes: &mut Vec<String>) -> Vec<(f64, String, String)> {
    let mut args = doctor::words(&["journalctl"]);
    if user {
        args.push("--user".into());
    }
    args.extend(doctor::words(&["-b", "-o", "json", "--no-pager"]));
    if user {
        args.extend(doctor::words(&["--grep","Reload requested from client|Reloading finished in|listening on Wayland socket:|Configuration Loaded"]));
    } else {
        args.extend(doctor::words(&[
            "-u",
            "greetd.service",
            "--grep",
            "session opened for user ",
        ]));
    }
    let reply = r.query(&args);
    if !reply.ok() {
        notes.push(format!(
            "{} journal unavailable: {}",
            if user { "User" } else { "Greetd" },
            reply.text
        ));
        return Vec::new();
    }
    reply
        .text
        .lines()
        .filter_map(|line| {
            let row: Value = serde_json::from_str(line).ok()?;
            let stamp = row["__MONOTONIC_TIMESTAMP"].as_str()?.parse::<f64>().ok()? / 1e6;
            Some((
                stamp,
                row["_SYSTEMD_USER_UNIT"].as_str().unwrap_or("").into(),
                decode_message(&row["MESSAGE"]),
            ))
        })
        .collect()
}
pub fn collect(r: &impl Runtime) -> Value {
    let mut events = BTreeMap::new();
    let mut notes = Vec::new();
    for (key, unit) in UNITS {
        let v = doctor::query(
            r,
            &[
                "systemctl",
                "--user",
                "show",
                unit,
                "-p",
                "ExecMainStartTimestampMonotonic",
                "-p",
                "ActiveEnterTimestampMonotonic",
                "-p",
                "Type",
                "-p",
                "LoadState",
            ],
        );
        if !v.ok() {
            notes.push(format!("{unit}: {}", v.text));
            continue;
        }
        let props: BTreeMap<_, _> = v
            .text
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        for (field, label) in [
            ("ExecMainStartTimestampMonotonic", "launch"),
            ("ActiveEnterTimestampMonotonic", "active"),
        ] {
            if let Some(n) = props
                .get(field)
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|n| *n > 0)
            {
                events.insert(format!("{key}_{label}"), n as f64 / 1e6);
            }
        }
    }
    let niri = events.get("niri_launch").copied().unwrap_or(0.0);
    let shell = events.get("shell_launch").copied().unwrap_or(f64::INFINITY);
    let mut reloads = 0;
    let mut reload_ms = 0u64;
    for (stamp, unit, message) in journal(r, true, &mut notes) {
        if stamp < niri {
            continue;
        }
        if unit == "niri.service" && message.contains("listening on Wayland socket:") {
            events.entry("niri_wayland_socket".into()).or_insert(stamp);
        }
        if unit == "desktop-foundation-shell.service"
            && stamp >= shell
            && message.contains("Configuration Loaded")
        {
            events.entry("shell_loaded".into()).or_insert(stamp);
        }
        if stamp <= shell
            && message.contains("Reload requested from client")
            && (message.contains("session\\x2dstart") || message.contains("(unit niri.service)"))
        {
            reloads += 1;
        }
        if stamp <= shell
            && let Some(end) = message
                .split("Reloading finished in ")
                .nth(1)
                .and_then(|s| {
                    let mut parts = s.split_whitespace();
                    let value = parts.next()?.parse::<u64>().ok()?;
                    (parts.next()? == "ms").then_some(value)
                })
        {
            reload_ms = reload_ms.saturating_add(end);
        }
    }
    let user = r.env("USER").unwrap_or_default();
    for (stamp, _, message) in journal(r, false, &mut notes) {
        if stamp <= niri
            && !user.is_empty()
            && message.contains(&format!("session opened for user {user}("))
        {
            events.insert("session_open".into(), stamp);
        }
    }
    let origin = events.get("session_open").copied().unwrap_or(niri);
    let timeline: BTreeMap<_, _> = events
        .iter()
        .filter(|(_, v)| **v >= origin)
        .map(|(k, v)| (k, ((v - origin) * 100000.0).round() / 100.0))
        .collect();
    let prior: BTreeMap<_, _> = events.iter().filter(|(_, v)| **v < origin).collect();
    json!({"boot_id":doctor::text(r,Path::new("/proc/sys/kernel/random/boot_id")).unwrap_or_default().trim(),"niri_launch_boot_seconds":niri,"events_boot_seconds":events,"timeline_ms":timeline,"previous_user_manager_events_boot_seconds":prior,"origin":if events.contains_key("session_open"){"session_open"}else{"niri_launch"},"metrics":{"niri_ready_to_shell_launch_ms":delta(&events,"niri_active","shell_launch"),"shell_launch_to_loaded_ms":delta(&events,"shell_launch","shell_loaded"),"niri_launch_to_shell_loaded_ms":delta(&events,"niri_launch","shell_loaded"),"session_open_to_niri_ready_ms":delta(&events,"session_open","niri_active"),"foundation_reload_requests_before_shell":reloads,"all_reload_duration_before_shell_ms":reload_ms},"default_target_report":doctor::query(r,&["systemd-analyze","--user","time"]).text,"limitations":["Service activation is not visual readiness.","Configuration Loaded is QML initialization, not proof of presentation.","Wallpaper first presentation and precise first launcher frame are not instrumented.","Missing journal evidence produces null timings.","Retained user-manager services can precede this login."],"diagnostic_notes":notes})
}
fn ipc(r: &impl Runtime, root: &Path, method: &str) -> Result<String> {
    let command = vec![
        "quickshell".into(),
        "ipc".into(),
        "--path".into(),
        root.join("shell").to_string_lossy().into_owned(),
        "call".into(),
        "foundation".into(),
        method.into(),
    ];
    let reply = r.query(&command);
    if reply.ok() {
        Ok(reply.text)
    } else {
        Err(invalid(format!("Shell IPC: {}", reply.text)))
    }
}
fn status(r: &impl Runtime, root: &Path, name: &str) -> Result<Value> {
    Ok(serde_json::from_str(&ipc(
        r,
        root,
        &format!("{name}Status"),
    )?)?)
}
pub fn benchmark(root: &Path, r: &impl Runtime) -> Result<Value> {
    const ACTIONS: [(&str, &str, &str); 4] = [
        ("launcher", "showLauncher", "hideLauncher"),
        ("clipboard", "showClipboard", "hideClipboard"),
        ("bluetooth", "toggleBluetooth", "toggleBluetooth"),
        ("power", "togglePower", "togglePower"),
    ];
    for (name, _, _) in ACTIONS {
        if status(r, root, name)?["visible"] == true {
            return Err(invalid(
                "Close Foundation popups before benchmarking; no additional shell is started",
            ));
        }
    }
    let mut measured = serde_json::Map::new();
    let mut owned = Vec::new();
    let result = (|| -> Result<()> {
        for (name, show, hide) in ACTIONS {
            let start = Instant::now();
            ipc(r, root, show)?;
            owned.push((name, hide));
            let deadline = start + Duration::from_secs(5);
            loop {
                let state = status(r, root, name)?;
                if state["visible"] == true
                    && (!["launcher", "clipboard"].contains(&name) || state["inputFocused"] == true)
                {
                    measured.insert(name.into(),json!({"creation_focus_ipc_roundtrip_ms":start.elapsed().as_secs_f64()*1000.0}));
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(invalid(format!("{name} failed to become ready")));
                }
                thread::sleep(Duration::from_millis(10));
            }
            ipc(r, root, hide)?;
            while status(r, root, name)?["visible"] == true {
                if Instant::now() >= deadline {
                    return Err(invalid(format!("{name} failed to close")));
                }
                thread::sleep(Duration::from_millis(20));
            }
            owned.pop();
            thread::sleep(Duration::from_millis(200));
        }
        Ok(())
    })();
    // Only close surfaces this invocation opened. Never invoke an item/action.
    for (name, hide) in owned {
        if status(r, root, name).is_ok_and(|v| v["visible"] == true) {
            let _ = ipc(r, root, hide);
        }
    }
    result?;
    Ok(
        json!({"surfaces":measured,"limitations":["Resident-shell creation/focus IPC roundtrip, not compositor presentation or physical keypress latency.","No Bluetooth connection, audio or power action is invoked."]}),
    )
}
fn label_valid(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 128
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn compare(directory: &Path) -> Result<String> {
    let mut groups: BTreeMap<String, BTreeMap<(String, u64), Value>> = BTreeMap::new();
    if !directory.exists() {
        return Ok(String::new());
    }
    let mut files: Vec<_> = fs::read_dir(directory)?.collect::<std::io::Result<_>>()?;
    files.sort_by_key(|f| f.file_name());
    for item in files {
        let p = item.path();
        if p.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(p)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Startup sample is not regular"));
        }
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("Startup sample exceeds 1 MiB"));
        }
        let sample: Value = serde_json::from_slice(&bytes)?;
        let label = sample["label"]
            .as_str()
            .filter(|s| label_valid(s))
            .ok_or_else(|| invalid("Invalid saved startup label"))?
            .to_owned();
        let boot = sample["boot_id"]
            .as_str()
            .ok_or_else(|| invalid("Missing sample boot ID"))?
            .to_owned();
        let launch = sample["niri_launch_boot_seconds"]
            .as_f64()
            .ok_or_else(|| invalid("Missing sample session timestamp"))?;
        groups
            .entry(label)
            .or_default()
            .insert((boot, launch.to_bits()), sample);
    }
    let mut output = String::new();
    for (label, samples) in groups {
        output.push_str(&format!("{label} unique sessions: {}\n", samples.len()));
        let mut values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for s in samples.values() {
            if let Some(metrics) = s["metrics"].as_object() {
                for (k, v) in metrics {
                    if let Some(v) = v.as_f64() {
                        values.entry(k.clone()).or_default().push(v);
                    }
                }
            }
        }
        for (k, mut v) in values {
            v.sort_by(f64::total_cmp);
            let median = if v.len() % 2 == 0 {
                (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
            } else {
                v[v.len() / 2]
            };
            output.push_str(&format!(
                "{k} median/min/max: {median} {} {}\n",
                v[0],
                v[v.len() - 1]
            ));
        }
    }
    Ok(output)
}
pub fn run(root: &Path, o: &Options, r: &impl Runtime) -> Result<()> {
    let directory =
        state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/startup/samples");
    if o.compare {
        print!("{}", compare(&directory)?);
        return Ok(());
    }
    if o.save.as_ref().is_some_and(|s| !label_valid(s)) {
        return Err(invalid("Use a simple startup sample label"));
    }
    let mut report = collect(r);
    if o.benchmark {
        report["benchmark"] = benchmark(root, r)?;
    }
    if let Some(label) = &o.save {
        let boot = report["boot_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
            .ok_or_else(|| invalid("Invalid boot ID; sample not saved"))?;
        let file = format!("{label}-{boot}-{}.json", report["niri_launch_boot_seconds"]);
        report["label"] = json!(label);
        state::private_directory(&directory)?;
        let mut bytes = serde_json::to_vec_pretty(&report)?;
        bytes.push(b'\n');
        state::atomic(&directory.join(file), &bytes, 0o600)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_cannot_escape_sample_directory() {
        for label in ["before", "after-2_test"] {
            assert!(label_valid(label));
        }
        for label in ["", "../escape", "/tmp/sample", "two words", "é"] {
            assert!(!label_valid(label));
        }
        assert!(!label_valid(&"x".repeat(129)));
    }
    #[test]
    fn comparison_deduplicates_sessions_and_rejects_symlinks() {
        let path = std::env::temp_dir().join(format!("foundation-startup-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let sample = |boot: &str, value: f64| json!({"label":"before","boot_id":boot,"niri_launch_boot_seconds":1.0,"metrics":{"shell_launch_to_loaded_ms":value}});
        fs::write(
            path.join("1.json"),
            serde_json::to_vec(&sample("a", 10.0)).unwrap(),
        )
        .unwrap();
        fs::write(
            path.join("2.json"),
            serde_json::to_vec(&sample("a", 10.0)).unwrap(),
        )
        .unwrap();
        fs::write(
            path.join("3.json"),
            serde_json::to_vec(&sample("b", 30.0)).unwrap(),
        )
        .unwrap();
        let report = compare(&path).unwrap();
        assert!(report.contains("before unique sessions: 2"));
        assert!(report.contains("median/min/max: 20 10 30"));
        std::os::unix::fs::symlink(path.join("1.json"), path.join("4.json")).unwrap();
        assert!(compare(&path).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}
