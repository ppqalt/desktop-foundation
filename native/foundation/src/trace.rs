//! Finite process tracing of an isolated shell; the resident shell is untouched.
use crate::{Result, apps, invalid};
use std::{
    fs,
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Temporary(PathBuf);
impl Temporary {
    fn create() -> Result<Self> {
        let base = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("Invalid clock"))?
            .as_nanos();
        for attempt in 0..100 {
            let path = base.join(format!(
                "foundation-trace-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(invalid("Cannot create a private trace directory"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// The tracer and its traced child are created in a dedicated process group.
struct OwnedTracer(Child);
impl OwnedTracer {
    fn stop(&mut self) -> Result<()> {
        let group = self.0.id() as libc::pid_t;
        let children = fs::read_to_string(format!("/proc/{group}/task/{group}/children"))?;
        for pid in children
            .split_whitespace()
            .filter_map(|s| s.parse::<libc::pid_t>().ok())
            .filter(|pid| *pid > 0)
        {
            // SAFETY: these PIDs are direct children of our owned tracer.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.try_wait()?.is_none() {
            if Instant::now() >= deadline {
                // SAFETY: this is the same owned child process group.
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
                self.0.wait()?;
                return Err(invalid("Timed out stopping the trace"));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
impl Drop for OwnedTracer {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // SAFETY: the process group was created exclusively for this child.
            unsafe {
                libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
}

fn duration(args: &[String]) -> Result<f64> {
    let mut seconds = 3.0_f64;
    let mut i = 0;
    while i < args.len() {
        let value = if args[i] == "--seconds" {
            i += 1;
            args.get(i)
                .ok_or_else(|| invalid("--seconds requires a value"))?
                .as_str()
        } else if let Some(value) = args[i].strip_prefix("--seconds=") {
            value
        } else {
            return Err(invalid("Expected trace [--seconds SECONDS]"));
        };
        seconds = value.parse().map_err(|_| invalid("Invalid seconds"))?;
        i += 1;
    }
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 30.0 {
        return Err(invalid("seconds must be in (0,30]"));
    }
    Ok(seconds)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let seconds = duration(args)?;
    if !apps::executable("strace") {
        println!("Unavailable: install sudo pacman -Syu --needed strace");
        return Ok(());
    }
    let temp = Temporary::create()?;
    let log = temp.0.join("process.log");
    let child = Command::new("strace")
        .args(["-f", "-e", "trace=process", "-o"])
        .arg(&log)
        .arg("quickshell")
        .arg("--path")
        .arg(root.join("shell"))
        .process_group(0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut tracer = OwnedTracer(child);
    let children = format!("/proc/{}/task/{}/children", tracer.0.id(), tracer.0.id());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if tracer.0.try_wait()?.is_some() {
            return Err(invalid("strace unavailable: check ptrace permissions"));
        }
        if fs::read_to_string(&children).is_ok_and(|text| !text.trim().is_empty()) {
            break;
        }
        if Instant::now() >= deadline {
            return Err(invalid("No traced child"));
        }
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_secs_f64(seconds));
    tracer.stop()?;
    let contents = fs::read_to_string(log)?;
    let execs: Vec<_> = contents
        .lines()
        .filter(|line| line.contains("execve(") || line.contains("execveat("))
        .collect();
    println!(
        "{seconds:?}s trace: {} exec requests, including initial Quickshell launch; no conclusion about future wakeups.",
        execs.len()
    );
    if execs.len() > 1 {
        println!("{}", execs.join("\n"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_seconds() {
        assert_eq!(duration(&[]).unwrap(), 3.0);
        assert_eq!(duration(&["--seconds=0.5".into()]).unwrap(), 0.5);
        for value in ["0", "31", "nan", "inf"] {
            assert!(duration(&["--seconds".into(), value.into()]).is_err());
        }
        assert!(duration(&["--other".into()]).is_err());
    }
}
