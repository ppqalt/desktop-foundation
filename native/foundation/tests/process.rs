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
            // group exits. Preserve stdin on fd 3 before POSIX shells replace
            // background stdin with /dev/null; the child must retain the pipe.
            "exec 3<&0; setsid sh -c 'echo $$ > \"$1\"; sleep 20' child \"$1\" <&3 3<&- & while [ ! -s \"$1\" ]; do sleep 0.01; done".into(),
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
        let error = result.expect_err("The detached child must retain its pipe");
        assert!(
            error.to_string().contains("timed out"),
            "capture={capture}: {error}"
        );
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

#[test]
fn larger_snapshot_limit_does_not_loosen_stderr_or_default_limits() {
    let command = ["sh".into(), "-c".into(), "head -c 100000 /dev/zero".into()];
    assert!(process::run(&command, None, Duration::from_secs(2), true).is_err());
    assert_eq!(
        process::checked_capture(&command, Duration::from_secs(2), 128 * 1024)
            .unwrap()
            .stdout
            .len(),
        100000
    );
    assert!(
        process::checked_capture(
            &[
                "sh".into(),
                "-c".into(),
                "head -c 100000 /dev/zero >&2".into()
            ],
            Duration::from_secs(2),
            128 * 1024
        )
        .is_err()
    );
    for limit in [0, 16 * 1024 * 1024 + 1] {
        assert!(process::checked_capture(&command, Duration::from_secs(2), limit).is_err());
    }
}

#[test]
fn interactive_selector_preserves_status_and_bounds_output() {
    let output = process::interactive(
        &[
            "sh".into(),
            "-c".into(),
            "sleep 0.1; printf 'selection cancelled' >&2; exit 1".into(),
        ],
        Some(Vec::new()),
    )
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"selection cancelled");
    assert!(
        process::interactive(
            &["sh".into(), "-c".into(), "head -c 100000 /dev/zero".into()],
            None
        )
        .is_err()
    );
}

#[test]
fn image_capture_has_a_separate_binary_cap_and_keeps_error_limits() {
    let command = [
        "sh".into(),
        "-c".into(),
        "head -c 18000000 /dev/zero".into(),
    ];
    assert_eq!(
        process::checked_image(&command, Duration::from_secs(3))
            .unwrap()
            .stdout
            .len(),
        18_000_000
    );
    assert!(
        process::checked_image(
            &[
                "sh".into(),
                "-c".into(),
                "head -c 100000 /dev/zero >&2".into()
            ],
            Duration::from_secs(2)
        )
        .is_err()
    );
    assert!(
        process::checked_image(
            &["sh".into(), "-c".into(), "sleep 20".into()],
            Duration::from_millis(100)
        )
        .is_err()
    );
}

#[test]
fn subscription_delivers_bounded_chunks_then_eof_and_reaps_on_drop() {
    let mut subscription = process::Subscription::open(&[
        "sh".into(),
        "-c".into(),
        "printf 'first\\nsecond\\n'".into(),
    ])
    .unwrap();
    let pid = subscription.pid() as libc::pid_t;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut data = Vec::new();
    loop {
        match subscription.read(deadline).unwrap() {
            process::StreamRead::Data(chunk) => {
                assert!(chunk.len() <= 8192);
                data.extend(chunk);
            }
            process::StreamRead::Eof => break,
            process::StreamRead::Timeout => panic!("Fixture exited without reaching EOF"),
        }
    }
    assert_eq!(data, b"first\nsecond\n");
    drop(subscription);
    // SAFETY: signal zero probes only the known, now-reaped fixture child.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
fn subscription_deadline_waits_for_events_and_drop_stops_its_group() {
    let mut subscription =
        process::Subscription::open(&["sh".into(), "-c".into(), "sleep 20 & wait".into()]).unwrap();
    let pid = subscription.pid() as libc::pid_t;
    let start = Instant::now();
    assert!(matches!(
        subscription
            .read(start + Duration::from_millis(150))
            .unwrap(),
        process::StreamRead::Timeout
    ));
    assert!(start.elapsed() >= Duration::from_millis(140));
    assert!(start.elapsed() < Duration::from_secs(2));
    drop(subscription);
    // SAFETY: the direct child belongs to the isolated subscription fixture.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
}

#[test]
fn machine_requests_and_subscription_use_c_locale() {
    let args = ["sh".into(), "-c".into(), "printf '%s' \"$LC_ALL\"".into()];
    assert_eq!(
        process::checked_c_locale(&args, Duration::from_secs(2), 64)
            .unwrap()
            .stdout,
        b"C"
    );
    let mut subscription = process::Subscription::open(&args).unwrap();
    match subscription
        .read(Instant::now() + Duration::from_secs(2))
        .unwrap()
    {
        process::StreamRead::Data(data) => assert_eq!(data, b"C"),
        _ => panic!("Fixture must report its effective locale"),
    }
}
