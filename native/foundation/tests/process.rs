use desktop_foundationctl::process;
use std::time::{Duration, Instant};

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
