use std::process::Command;

#[test]
fn power_surface_keeps_focus_error_feedback_and_animated_exit() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Skipping power QML fixture: Quickshell unavailable");
        return;
    }
    let runner =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-power-focus");
    let output = Command::new(runner).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
