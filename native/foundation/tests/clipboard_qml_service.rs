use std::process::Command;

#[test]
fn private_history_updates_and_reports_errors_through_the_qml_service() {
    if !desktop_foundationctl::apps::executable("quickshell") {
        eprintln!("Skipping clipboard service QML fixture: Quickshell unavailable");
        return;
    }
    let runner =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/test-clipboard-service");
    let output = Command::new(runner)
        .env(
            "DF_TEST_NATIVE_BINARY",
            env!("CARGO_BIN_EXE_desktop-foundationctl"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
