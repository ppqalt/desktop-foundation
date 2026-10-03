use desktop_foundationctl::process;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Detached {
    directory: PathBuf,
}
impl Detached {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "foundation-detached-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self { directory }
    }
    fn command(&self) -> Vec<String> {
        vec![
            "sh".into(),
            "-c".into(),
            // The readiness file ensures setsid completed before the original
            // group exits. The detached child retains all inherited pipe ends.
            "setsid sh -c 'echo $$ > \"$1\"; sleep 20' child \"$1\" <&0 & while [ ! -s \"$1\" ]; do sleep 0.01; done".into(),
            "parent".into(),
            self.directory.join("pid").to_string_lossy().into_owned(),
        ]
    }
    fn pid(&self) -> libc::pid_t {
        fs::read_to_string(self.directory.join("pid"))
            .unwrap()
            .trim()
            .parse::<libc::pid_t>()
            .unwrap()
    }
}
impl Drop for Detached {
    fn drop(&mut self) {
        if let Ok(pid) = fs::read_to_string(self.directory.join("pid"))
            && let Ok(pid) = pid.trim().parse::<libc::pid_t>()
        {
            // SAFETY: this positive PID was published by our isolated setsid
            // fixture; kill only its known group after the assertion/timeout.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn timeout_cleans_up_children_holding_output_pipes() {
    let start = Instant::now();
    assert!(
        process::run(
            &["sh".into(), "-c".into(), "sleep 20 & wait".into()],
            None,
            Duration::from_millis(150),
            true
        )
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
fn output_and_stdin_are_bounded_and_failures_are_visible() {
    assert!(
        process::checked(
            &[
                "sh".into(),
                "-c".into(),
                "printf failure >&2; exit 7".into()
            ],
            None,
            Duration::from_secs(2),
            true
        )
        .unwrap_err()
        .to_string()
        .contains("failure")
    );
    assert!(
        process::run(
            &["sh".into(), "-c".into(), "head -c 100000 /dev/zero".into()],
            None,
            Duration::from_secs(2),
            true
        )
        .is_err()
    );
    let payload = "ä🙂\0\n".as_bytes().to_vec();
    let result = process::checked(
        &["cat".into()],
        Some(payload.clone()),
        Duration::from_secs(2),
        true,
    )
    .unwrap();
    assert_eq!(result.stdout, payload);
}
#[test]
fn a_nonreading_child_cannot_block_stdin_forever() {
    let start = Instant::now();
    assert!(
        process::run(
            &["sleep".into(), "20".into()],
            Some(vec![0; 1024 * 1024]),
            Duration::from_millis(150),
            false
        )
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn detached_descendants_cannot_extend_output_or_stdin_deadlines() {
    for capture in [true, false] {
        let fixture = Detached::new();
        let start = Instant::now();
        let result = process::run(
            &fixture.command(),
            (!capture).then(|| vec![0; 1024 * 1024]),
            Duration::from_millis(250),
            capture,
        );
        assert!(result.unwrap_err().to_string().contains("timed out"));
        assert!(fixture.pid() > 0, "The detached child must actually start");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "An escaped child must not hold a stream worker past the deadline"
        );
    }
}

#[test]
fn successful_clipboard_style_ownership_is_not_killed() {
    let fixture = Detached::new();
    let output = process::checked(&fixture.command(), None, Duration::from_secs(2), false).unwrap();
    assert!(output.status.success());
    // SAFETY: signal zero only probes our isolated child's known PID.
    assert_eq!(unsafe { libc::kill(fixture.pid(), 0) }, 0);
}
