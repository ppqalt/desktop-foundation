use std::process::Command;

#[test]
fn ranks_and_filters_application_search_results() {
    if !desktop_foundationctl::apps::executable("node") {
        eprintln!("Skipping JavaScript search fixture: node unavailable");
        return;
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/search.mjs");
    assert!(
        Command::new("node")
            .arg(fixture)
            .status()
            .unwrap()
            .success()
    );
}
