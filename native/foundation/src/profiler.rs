//! Finite QML profiler and shell memory report.
use crate::{Result, baseline, benchmark, invalid, process, theme_render};
use quick_xml::{events::Event, reader::Reader};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    net::TcpListener,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Temporary(PathBuf);
impl Temporary {
    fn create() -> Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("Invalid clock"))?
            .as_nanos();
        for n in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "foundation-qml-profile-{}-{nonce}-{n}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(invalid("Cannot create temporary profile directory"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Owned(Child);
impl Owned {
    fn stop(&mut self) -> Result<()> {
        if self.0.try_wait()?.is_some() {
            return Ok(());
        }
        // SAFETY: the group belongs exclusively to the child we spawned.
        unsafe {
            libc::kill(-(self.0.id() as libc::pid_t), libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.0.try_wait()?.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                // SAFETY: the child remains alive and owns its process group.
                unsafe {
                    libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
                }
                self.0.wait()?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn memory(pid: u32) -> Result<BTreeMap<String, u64>> {
    let mut result = BTreeMap::new();
    for line in fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))?.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() == 3 && fields[2] == "kB" {
            result.insert(
                fields[0].trim_end_matches(':').to_string(),
                fields[1]
                    .parse()
                    .map_err(|_| invalid("Invalid memory counter"))?,
            );
        }
    }
    Ok(result)
}
fn counters(pid: u32) -> Result<(u64, u64)> {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let ticks = baseline::sample(&proc)?.ticks;
    let switches = benchmark::thread_switches(&proc)?
        .values()
        .try_fold(0_u64, |sum, value| {
            sum.checked_add(*value)
                .ok_or_else(|| invalid("Context-switch counter overflow"))
        })?;
    Ok((ticks, switches))
}
fn idle(pid: u32, seconds: Duration) -> Result<Value> {
    let before = counters(pid)?;
    let start = Instant::now();
    thread::sleep(seconds);
    let elapsed = start.elapsed().as_secs_f64();
    let after = counters(pid)?;
    // SAFETY: sysconf takes a constant and has no pointer arguments.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz <= 0 {
        return Err(invalid("Cannot determine clock tick rate"));
    }
    Ok(
        json!({"seconds":elapsed,"cpu_percent_of_one_core":after.0.checked_sub(before.0).ok_or_else(|| invalid("CPU counters reset"))? as f64 / hz as f64 / elapsed * 100.0,"thread_context_switches_per_second":(i128::from(after.1) - i128::from(before.1)) as f64 / elapsed}),
    )
}
fn ipc(shell: &Path, path: &Path, method: &str) -> Result<Value> {
    let args = vec![
        shell.to_string_lossy().into_owned(),
        "ipc".into(),
        "--path".into(),
        path.to_string_lossy().into_owned(),
        "call".into(),
        "foundation".into(),
        method.into(),
    ];
    let output = process::checked(&args, None, Duration::from_secs(5), true)?;
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn complete_xml(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().check_end_names = true;
    let mut buffer = Vec::new();
    let mut depth = 0_i64;
    let mut root_seen = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(_)) => {
                if depth == 0 && root_seen {
                    return false;
                }
                root_seen = true;
                depth += 1;
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    if root_seen {
                        return false;
                    }
                    root_seen = true;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Ok(Event::Eof) => return root_seen && depth == 0,
            Err(_) => return false,
            _ => {}
        }
        buffer.clear();
    }
}
struct Options {
    seconds: u64,
    popups: bool,
}
fn options(args: &[String]) -> Result<Options> {
    let mut value = Options {
        seconds: 15,
        popups: false,
    };
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--popups" {
            value.popups = true;
        } else if args[i] == "--seconds" {
            i += 1;
            value.seconds = args
                .get(i)
                .ok_or_else(|| invalid("--seconds requires a value"))?
                .parse()
                .map_err(|_| invalid("Invalid seconds"))?;
        } else if let Some(s) = args[i].strip_prefix("--seconds=") {
            value.seconds = s.parse().map_err(|_| invalid("Invalid seconds"))?;
        } else {
            return Err(invalid(
                "Expected shell-profile [--seconds SECONDS] [--popups]",
            ));
        }
        i += 1;
    }
    if !(5..=45).contains(&value.seconds) {
        return Err(invalid("Sample duration must be 5–45 seconds"));
    }
    Ok(value)
}
fn required_bool(value: &Value, key: &str) -> Result<bool> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("Missing shell state: {key}")))
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let options = options(args)?;
    let command = [
        "systemctl".into(),
        "--user".into(),
        "show".into(),
        "desktop-foundation-shell.service".into(),
        "-p".into(),
        "MainPID".into(),
        "--value".into(),
    ];
    let output = process::checked(&command, None, Duration::from_secs(5), true)?;
    let pid: u32 = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Invalid shell PID"))?
        .trim()
        .parse()
        .map_err(|_| invalid("Invalid shell PID"))?;
    if pid == 0 {
        return Err(invalid("Shell is not running"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| invalid("HOME is not set"))?;
    let folder = PathBuf::from(home).join(".cache/desktop-foundation/boot-audit/optimization");
    execute(
        root,
        &folder,
        pid,
        &options,
        Path::new("quickshell"),
        Path::new("/usr/lib/qt6/bin/qmlprofiler"),
        Duration::from_secs(options.seconds),
    )
}

fn execute(
    root: &Path,
    folder: &Path,
    pid: u32,
    options: &Options,
    shell_exec: &Path,
    profiler_exec: &Path,
    sample_time: Duration,
) -> Result<()> {
    fs::create_dir_all(folder)?;
    let mut report = json!({"live_before_kib":memory(pid)?, "live_idle":idle(pid, sample_time)?});
    let trace = folder.join("shell-qml.qtd");
    if let Err(e) = fs::remove_file(&trace)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(e.into());
    }
    let temp = Temporary::create()?;
    let path = temp.0.join("shell");
    theme_render::copy_tree(&root.join("shell"), &path)?;
    let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    let log = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(folder.join("qml-profile.log"))?;
    let mut shell = Owned(
        Command::new(shell_exec)
            .args(["--path"])
            .arg(&path)
            .args(["--debug", &port.to_string(), "--waitfordebug"])
            .process_group(0)
            .stdout(log.try_clone()?)
            .stderr(log.try_clone()?)
            .spawn()?,
    );
    let mut profiler = Owned(
        Command::new(profiler_exec)
            .args([
                "--attach",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--interactive",
                "--output",
            ])
            .arg(&trace)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let state = loop {
        if shell.0.try_wait()?.is_some() || profiler.0.try_wait()?.is_some() {
            return Err(invalid("Profiler/shell exited; inspect qml-profile.log"));
        }
        if let Ok(value) = ipc(shell_exec, &path, "status")
            && value.get("ready").and_then(Value::as_bool) == Some(true)
        {
            break value;
        }
        if Instant::now() >= deadline {
            return Err(invalid("Isolated shell readiness timeout"));
        }
        thread::sleep(Duration::from_millis(100));
    };
    report["isolated_root"] = json!({"launcherAlive":required_bool(&state,"launcherAlive")?,"clipboardAlive":required_bool(&state,"clipboardAlive")?,"probeAlive":required_bool(&state,"probeAlive")?,"ready":required_bool(&state,"ready")?});
    report["isolated_initial_kib"] = json!(memory(shell.0.id())?);
    if options.popups {
        let mut samples = Vec::new();
        for _ in 0..3 {
            for (show, close) in [
                ("showLauncher", "hideLauncher"),
                ("showClipboard", "hideClipboard"),
                ("toggleBluetooth", "toggleBluetooth"),
                ("togglePower", "togglePower"),
            ] {
                ipc(shell_exec, &path, show)?;
                thread::sleep(Duration::from_millis(500));
                ipc(shell_exec, &path, close)?;
                thread::sleep(Duration::from_millis(500));
            }
            let status = ipc(shell_exec, &path, "status")?;
            let bluetooth = ipc(shell_exec, &path, "bluetoothStatus")?;
            let power = ipc(shell_exec, &path, "powerStatus")?;
            samples.push(json!({"kib":memory(shell.0.id())?,"launcher_alive":required_bool(&status,"launcherAlive")?,"clipboard_alive":required_bool(&status,"clipboardAlive")?,"bluetooth_alive":bluetooth.get("alive").and_then(Value::as_bool).unwrap_or(false),"power_visible":required_bool(&power,"visible")?}));
        }
        report["isolated_closed_popup_cycles"] = json!(samples);
    }
    report["isolated_idle"] = idle(shell.0.id(), sample_time)?;
    report["isolated_final_kib"] = json!(memory(shell.0.id())?);
    let stdin = profiler
        .0
        .stdin
        .as_mut()
        .ok_or_else(|| invalid("Profiler input unavailable"))?;
    stdin.write_all(b"flush\n")?;
    stdin.flush()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !complete_xml(&trace) {
        if Instant::now() >= deadline {
            return Err(invalid("Profiler did not finish writing a complete trace"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    fs::set_permissions(&trace, fs::Permissions::from_mode(0o600))?;
    profiler.stop()?;
    shell.stop()?;
    report["live_after_kib"] = json!(memory(pid)?);
    report["limitations"] = json!([
        "Debug tracing adds overhead; isolated startup is not a real login benchmark.",
        "RSS includes shared pages; use PSS for proportional attribution.",
        "Thread context switches are a scheduling proxy, not an exact timer-wakeup count.",
        "Allocator/graphics caches may retain freed pages; a higher RSS alone is not a leak."
    ]);
    let text = format!("{}\n", serde_json::to_string_pretty(&report)?);
    fs::write(folder.join("shell-profile.json"), &text)?;
    fs::set_permissions(
        folder.join("shell-profile.json"),
        fs::Permissions::from_mode(0o600),
    )?;
    print!("{text}");
    println!(
        "Trace/log/report saved in {}; isolated process cleaned up.",
        folder.display()
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments() {
        assert_eq!(options(&[]).unwrap().seconds, 15);
        assert!(options(&["--seconds=4".into()]).is_err());
        assert!(options(&["--seconds".into(), "46".into()]).is_err());
        assert!(options(&["--popups".into()]).unwrap().popups);
    }
    #[test]
    fn complete_trace_requires_well_formed_xml() {
        let temp = Temporary::create().unwrap();
        let file = temp.0.join("x");
        fs::write(&file, "<root><event></root>").unwrap();
        assert!(!complete_xml(&file));
        fs::write(&file, "<root><event/></root>").unwrap();
        assert!(complete_xml(&file));
        fs::write(&file, "<root><event>").unwrap();
        assert!(!complete_xml(&file));
    }

    #[test]
    fn isolated_profile_flushes_trace_and_cleans_up_processes() {
        let temp = Temporary::create().unwrap();
        let root = temp.0.join("root");
        fs::create_dir_all(root.join("shell")).unwrap();
        fs::write(root.join("shell/shell.qml"), "fixture").unwrap();
        let folder = temp.0.join("report");
        let pid_file = temp.0.join("shell.pid");
        let shell = temp.0.join("shell-command");
        let shell_script = format!(
            "#!/bin/sh\nif [ \"$1\" = ipc ]; then\n case \"$6\" in\n status) printf '%s\\n' '{{\"ready\":true,\"launcherAlive\":false,\"clipboardAlive\":false,\"probeAlive\":false}}';;\n bluetoothStatus) printf '%s\\n' '{{\"alive\":false}}';;\n powerStatus) printf '%s\\n' '{{\"visible\":false}}';;\n *) printf '%s\\n' null;;\n esac\n exit 0\nfi\nprintf '%s' \"$$\" > {}\ntrap 'exit 0' TERM\nwhile :; do /bin/sleep 1; done\n",
            shell_words::quote(pid_file.to_str().unwrap())
        );
        fs::write(&shell, shell_script).unwrap();
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700)).unwrap();
        let profiler = temp.0.join("profiler-command");
        fs::write(&profiler, "#!/bin/sh\nread command\n[ \"$command\" = flush ] || exit 2\nprintf '%s' '<trace><event/></trace>' > \"$7\"\ntrap 'exit 0' TERM\nwhile :; do /bin/sleep 1; done\n").unwrap();
        fs::set_permissions(&profiler, fs::Permissions::from_mode(0o700)).unwrap();
        let options = Options {
            seconds: 5,
            popups: true,
        };
        execute(
            &root,
            &folder,
            std::process::id(),
            &options,
            &shell,
            &profiler,
            Duration::from_millis(10),
        )
        .unwrap();
        let result: Value =
            serde_json::from_slice(&fs::read(folder.join("shell-profile.json")).unwrap()).unwrap();
        assert_eq!(
            result["isolated_closed_popup_cycles"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert!(result["isolated_idle"]["seconds"].as_f64().unwrap() >= 0.01);
        assert!(complete_xml(&folder.join("shell-qml.qtd")));
        assert_eq!(
            fs::metadata(folder.join("shell-qml.qtd"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let child: u32 = fs::read_to_string(pid_file).unwrap().parse().unwrap();
        assert!(!Path::new(&format!("/proc/{child}")).exists());
    }

    #[test]
    fn failed_profiler_does_not_leave_the_owned_shell_running() {
        let temp = Temporary::create().unwrap();
        let root = temp.0.join("root");
        fs::create_dir_all(root.join("shell")).unwrap();
        fs::write(root.join("shell/shell.qml"), "fixture").unwrap();
        let pid_file = temp.0.join("shell.pid");
        let shell = temp.0.join("shell-command");
        let script = format!(
            "#!/bin/sh\n[ \"$1\" = ipc ] && exit 1\nprintf '%s' \"$$\" > {}\ntrap 'exit 0' TERM\nwhile :; do /bin/sleep 1; done\n",
            shell_words::quote(pid_file.to_str().unwrap())
        );
        fs::write(&shell, script).unwrap();
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700)).unwrap();
        let options = Options {
            seconds: 5,
            popups: false,
        };
        let result = execute(
            &root,
            &temp.0.join("report"),
            std::process::id(),
            &options,
            &shell,
            Path::new("/bin/false"),
            Duration::from_millis(10),
        );
        assert!(result.is_err());
        if let Ok(pid) = fs::read_to_string(pid_file) {
            let pid: u32 = pid.parse().unwrap();
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
        }
    }
}
