use std::process::Command;

#[test]
fn volume_feedback_retains_first_frame_instant_updates_and_expiry() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Skipping volume QML fixture: Quickshell unavailable");
        return;
    }
    let runner =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-volume-feedback");
    let output = Command::new(runner).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
