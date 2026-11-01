use std::process::Command;

#[test]
fn samples_process_and_preserves_json_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args([
            "baseline",
            &std::process::id().to_string(),
            "--seconds",
            "0.01",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["pid"], std::process::id());
    assert!(data["duration_s"].as_f64().unwrap() >= 0.01);
    assert!(data["cpu_percent_one_core"].as_f64().unwrap() >= 0.0);
    assert!(data["Rss_KiB"].as_u64().unwrap() > 0);
    assert!(data["Pss_KiB"].as_u64().is_some());
    assert!(
        data["main_thread_context_switches_per_s"]["voluntary_ctxt_switches"]
            .as_f64()
            .is_some()
    );
}
