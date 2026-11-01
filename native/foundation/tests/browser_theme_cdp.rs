use std::process::Command;

#[test]
fn restricted_lifecycle_client_does_not_access_browser_data() {
    if !desktop_foundationctl::apps::executable("node") {
        eprintln!("Skipping browser theme lifecycle fixture: node unavailable");
        return;
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/brave_theme_cdp.test.mjs");
    let output = Command::new("node")
        .arg("--test")
        .arg(fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
