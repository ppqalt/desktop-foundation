//! Bounded shell readiness probes; presentation actions are sent exactly once.
use crate::{Result, invalid, process};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn call(root: &Path, method: &[String]) -> Result<()> {
    if method.is_empty() || method[0].is_empty() {
        return Err(invalid("Shell IPC method required"));
    }
    let mut args = vec![
        "quickshell".into(),
        "ipc".into(),
        "--path".into(),
        root.join("shell").to_string_lossy().into_owned(),
        "call".into(),
        "foundation".into(),
    ];
    let mut probe = args.clone();
    probe.push("status".into());
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut diagnostic = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(invalid(format!(
                "Desktop shell did not become ready within three seconds: {diagnostic}"
            )));
        }
        match process::checked(
            &probe,
            None,
            remaining.min(Duration::from_millis(500)),
            false,
        ) {
            Ok(_) => break,
            Err(error) => diagnostic = error.to_string(),
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
    args.extend_from_slice(method);
    // An unknown outcome must never be replayed: a toggle could undo itself.
    let output = process::checked(&args, None, Duration::from_secs(3), true)?;
    use std::io::Write;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    Ok(())
}
