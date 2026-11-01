//! Finite shell benchmarks. Every measured shell is owned by this invocation.
use crate::{Result, baseline, invalid, process, trace};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OBSERVATION: &str = "not captured by endpoint snapshots; use strace -f -e trace=process for exact finite exec tracing";

fn ipc(pid: u32, method: &str) -> Option<String> {
    let args = [
        "quickshell".into(),
        "ipc".into(),
        "--pid".into(),
        pid.to_string(),
        "call".into(),
        "foundation".into(),
        method.into(),
    ];
    let result = process::run(&args, None, Duration::from_secs(3), true).ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).into_owned())
}

fn wait_status(pid: u32, key: &str, desired: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ipc(pid, "status")
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|s| s.get(key).and_then(Value::as_bool))
            == Some(desired)
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(invalid("Timed out waiting for shell state"))
}

pub(crate) fn thread_switches(proc: &Path) -> Result<BTreeMap<String, u64>> {
    let mut threads = BTreeMap::new();
    for entry in fs::read_dir(proc.join("task"))? {
        let entry = entry?;
        let text = match fs::read_to_string(entry.path().join("status")) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let mut sum = 0_u64;
        for line in text.lines().filter(|line| line.contains("ctxt_switches:")) {
            let value = line
                .split_once(':')
                .ok_or_else(|| invalid("Invalid thread counter"))?
                .1
                .trim()
                .parse::<u64>()
                .map_err(|_| invalid("Invalid thread counter"))?;
            sum = sum
                .checked_add(value)
                .ok_or_else(|| invalid("Thread counter overflow"))?;
        }
        threads.insert(entry.file_name().to_string_lossy().into_owned(), sum);
    }
    Ok(threads)
}

fn parent_pid(stat: &str) -> Option<u32> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn descendants(root: u32) -> Result<Vec<u32>> {
    let mut parents = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(stat) = fs::read_to_string(entry.path().join("stat"))
            && let Some(parent) = parent_pid(&stat)
        {
            parents.push((pid, parent));
        }
    }
    let mut found = BTreeSet::from([root]);
    loop {
        let len = found.len();
        for &(pid, parent) in &parents {
            if found.contains(&parent) {
                found.insert(pid);
            }
        }
        if len == found.len() {
            break;
        }
    }
    Ok(found.into_iter().collect())
}

fn measure(pid: u32, seconds: f64, phase: &str) -> Result<Value> {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let before = baseline::sample(&proc)?;
    let threads = thread_switches(&proc)?;
    let started = Instant::now();
    let family_before = descendants(pid)?;
    thread::sleep(Duration::from_secs_f64(seconds));
    let after = baseline::sample(&proc)?;
    let after_threads = thread_switches(&proc)?;
    let elapsed = started.elapsed().as_secs_f64();
    // SAFETY: this sysconf query has no pointer arguments.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz <= 0 {
        return Err(invalid("Cannot determine system clock tick rate"));
    }
    let ticks = after
        .ticks
        .checked_sub(before.ticks)
        .ok_or_else(|| invalid("Process counters reset"))?;
    let mut switches = 0_u64;
    for (id, initial) in &threads {
        if let Some(final_value) = after_threads.get(id) {
            switches = switches
                .checked_add(
                    final_value
                        .checked_sub(*initial)
                        .ok_or_else(|| invalid("Thread counters reset"))?,
                )
                .ok_or_else(|| invalid("Thread counter overflow"))?;
        }
    }
    let mut value = json!({"phase":phase,"duration_s":elapsed,"cpu_percent_one_core":100.0 * ticks as f64 / hz as f64 / elapsed,"all_surviving_threads_context_switches_per_s":switches as f64 / elapsed,"thread_set_changed":threads.keys().ne(after_threads.keys()),"processes_before":family_before,"processes_after":descendants(pid)?,"transient_subprocess_observation":OBSERVATION});
    for (key, amount) in after.memory {
        value[key] = json!(amount);
    }
    Ok(value)
}

struct OwnedShell(Child);
impl Drop for OwnedShell {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // SAFETY: the child was started in its own process group.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGTERM);
            }
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until {
                if self.0.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            // SAFETY: the still-running process group belongs to this child.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
}

/// Measure readiness of a newly spawned, owned shell process.
pub fn startup(root: &Path, args: &[String]) -> Result<()> {
    if !args.is_empty() {
        return Err(invalid("startup takes no arguments"));
    }
    let started = Instant::now();
    let child = Command::new("quickshell")
        .arg("--path")
        .arg(root.join("shell"))
        .arg("--no-color")
        .process_group(0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut shell = OwnedShell(child);
    let deadline = started + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(status) = shell.0.try_wait()? {
            return Err(invalid(format!("Quickshell exited: {status}")));
        }
        if ipc(shell.0.id(), "status")
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|s| s.get("ready").and_then(Value::as_bool))
            == Some(true)
        {
            println!(
                "{}",
                json!({"backend_ready_ms": started.elapsed().as_secs_f64() * 1000.0})
            );
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(invalid("Timed out waiting for backend readiness"))
}

struct Options {
    runs: usize,
    seconds: f64,
    output: Option<PathBuf>,
    trace: bool,
}
fn options(args: &[String]) -> Result<Options> {
    let mut result = Options {
        runs: 3,
        seconds: 5.0,
        output: None,
        trace: false,
    };
    let mut i = 0;
    while i < args.len() {
        let (key, inline) = args[i]
            .split_once('=')
            .map_or((args[i].as_str(), None), |(k, v)| (k, Some(v)));
        if key == "--trace" && inline.is_none() {
            result.trace = true;
            i += 1;
            continue;
        }
        let value = if let Some(value) = inline {
            value
        } else {
            i += 1;
            args.get(i)
                .ok_or_else(|| invalid(format!("{key} requires a value")))?
                .as_str()
        };
        match key {
            "--runs" => result.runs = value.parse().map_err(|_| invalid("Invalid runs"))?,
            "--seconds" => {
                result.seconds = value.parse().map_err(|_| invalid("Invalid seconds"))?
            }
            "--output" => result.output = Some(PathBuf::from(value)),
            _ => return Err(invalid(format!("Unknown benchmark option: {key}"))),
        }
        i += 1;
    }
    if !(1..=10).contains(&result.runs)
        || !result.seconds.is_finite()
        || result.seconds <= 0.0
        || result.seconds > 30.0
    {
        return Err(invalid("runs must be 1..10 and seconds (0,30]"));
    }
    Ok(result)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let options = options(args)?;
    let mut records = Vec::new();
    let mut readiness = Vec::new();
    for iteration in 0..options.runs {
        let started = Instant::now();
        let child = Command::new("quickshell")
            .arg("--path")
            .arg(root.join("shell"))
            .arg("--no-color")
            .process_group(0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let shell = OwnedShell(child);
        let pid = shell.0.id();
        wait_status(pid, "ready", true)?;
        let ready = started.elapsed().as_secs_f64() * 1000.0;
        readiness.push(ready);
        let cold = measure(pid, options.seconds, "fresh-process, warm filesystem cache")?;
        let started = Instant::now();
        ipc(pid, "showProbe");
        wait_status(pid, "probeAlive", true)?;
        let create = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        ipc(pid, "hideProbe");
        wait_status(pid, "probeAlive", false)?;
        let destroy = started.elapsed().as_secs_f64() * 1000.0;
        let warm = measure(pid, options.seconds, "warm-process after surface use")?;
        records.push(json!({"run":iteration+1,"ready_ms":ready,"probe_request_roundtrip_create_ms":create,"probe_request_roundtrip_destroy_ms":destroy,"idle":[cold,warm]}));
    }
    readiness.sort_by(f64::total_cmp);
    let middle = readiness.len() / 2;
    let median = if readiness.len() % 2 == 0 {
        (readiness[middle - 1] + readiness[middle]) / 2.0
    } else {
        readiness[middle]
    };
    let report = json!({"runs":records,"ready_median_ms":median,"limitations":["Probe timings include IPC harness overhead; not first-frame latency","No dropped caches; no zero-wakeup claim","PSS includes sharing with the resident shell"]});
    let output = format!("{}\n", serde_json::to_string_pretty(&report)?);
    if let Some(path) = options.output {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &output)?;
    }
    println!("{output}");
    if options.trace {
        trace::run(root, &[])?;
    }
    Ok(())
}

/// Measure the existing shell only on explicit request, restoring the launcher.
pub fn launcher(args: &[String]) -> Result<()> {
    if !args.is_empty() {
        return Err(invalid("launcher-bench takes no arguments"));
    }
    let command = [
        "systemctl".into(),
        "--user".into(),
        "show".into(),
        "-p".into(),
        "MainPID".into(),
        "--value".into(),
        "desktop-foundation-shell.service".into(),
    ];
    let output = process::checked(&command, None, Duration::from_secs(10), true)?;
    let pid: u32 = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("Invalid shell PID"))?
        .trim()
        .parse()
        .map_err(|_| invalid("Invalid shell PID"))?;
    if pid == 0 {
        return Err(invalid("Shell is not running"));
    }
    struct HideLauncher(u32);
    impl Drop for HideLauncher {
        fn drop(&mut self) {
            ipc(self.0, "hideLauncher");
        }
    }
    let _cleanup = HideLauncher(pid);
    ipc(pid, "hideLauncher");
    wait_status(pid, "launcherAlive", false)?;
    let closed = measure(pid, 5.0, "launcher hidden, warmed resident")?;
    let started = Instant::now();
    ipc(pid, "showLauncher");
    wait_status(pid, "launcherAlive", true)?;
    let opened_ms = started.elapsed().as_secs_f64() * 1000.0;
    thread::sleep(Duration::from_millis(500));
    let opened = measure(pid, 5.0, "launcher open, focused text caret")?;
    let started = Instant::now();
    ipc(pid, "hideLauncher");
    wait_status(pid, "launcherAlive", false)?;
    let closed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let after = measure(pid, 5.0, "launcher destroyed, warmed resident")?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"samples":[closed,opened,after],"creation_roundtrip_ms":opened_ms,"exit_and_destruction_roundtrip_ms":closed_ms,"limitations":["Single warmed resident process; retained Qt/icon caches are included","Finite IPC roundtrips are not first-frame presentation latency","Open text cursor intentionally wakes; no zero-wakeup claim"]})
        )?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_options_and_bounds() {
        let o = options(&[
            "--runs=2".into(),
            "--seconds".into(),
            "0.01".into(),
            "--trace".into(),
        ])
        .unwrap();
        assert_eq!(o.runs, 2);
        assert_eq!(o.seconds, 0.01);
        assert!(o.trace);
        for args in [
            vec!["--runs", "0"],
            vec!["--seconds", "nan"],
            vec!["--seconds", "31"],
            vec!["--bad", "1"],
        ] {
            assert!(options(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }
    #[test]
    fn parent_parsing_handles_spaces_and_parentheses() {
        assert_eq!(parent_pid("7 (some ) name) S 42 8"), Some(42));
    }
}
