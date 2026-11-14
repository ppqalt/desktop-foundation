//! Boot fixtures only: no live journal reads or service changes.
use desktop_foundationctl::{
    Result,
    boot_report::{self, Options},
    deploy,
    doctor::{Reply, Runtime},
    invalid,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
fn record(message: &str, stamp: u64, source: Option<u64>) -> Value {
    let mut row = json!({"MESSAGE":message,"__MONOTONIC_TIMESTAMP":stamp.to_string(),"_BOOT_ID":"abcd","_TRANSPORT":"kernel"});
    if let Some(source) = source {
        row["_SOURCE_MONOTONIC_TIMESTAMP"] = json!(source.to_string());
    }
    row
}
fn journal(rows: Vec<Value>) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
fn stages(firmware: u64) -> String {
    journal(vec![record(
        &format!(
            "Startup finished in {firmware}s (firmware) + 2s (loader) + 384ms (kernel) + 1min 2.345s (initrd) + 10us (userspace) = 1min 6.72901s."
        ),
        100000000,
        None,
    )])
}
#[derive(Default)]
struct Fake {
    boots: BTreeMap<String, Reply>,
    calls: RefCell<Vec<Vec<String>>>,
}
impl Runtime for Fake {
    fn query(&self, args: &[String]) -> Reply {
        self.calls.borrow_mut().push(args.to_vec());
        if args.first().is_some_and(|s| s == "journalctl")
            && args.get(1).is_some_and(|s| s == "-b")
            && args.get(2).is_some_and(|s| s.starts_with('-'))
        {
            return self.boots.get(&args[2]).cloned().unwrap_or(Reply {
                code: 1,
                text: "fixture journal missing".into(),
            });
        }
        Reply {
            code: 1,
            text: "fixture session unavailable".into(),
        }
    }
    fn read(&self, _: &Path) -> Result<Vec<u8>> {
        Err(invalid("fixture no session"))
    }
    fn exists(&self, _: &Path) -> bool {
        false
    }
    fn executable(&self, _: &str) -> bool {
        false
    }
    fn env(&self, _: &str) -> Option<String> {
        None
    }
    fn protected_helper(&self, _: &Path) -> bool {
        false
    }
    fn role_check(&self, _: &Path, _: bool) -> Result<Value> {
        panic!("not a boot-report operation")
    }
    fn validate_role(&self, _: &str, _: &Value) -> Result<()> {
        panic!("not a boot-report operation")
    }
    fn conflicts(&self, _: &Path, _: &deploy::Paths) -> Result<Vec<String>> {
        panic!("not a boot-report operation")
    }
    fn mods(&self, _: &Path, _: &Path) -> Result<Vec<String>> {
        panic!("not a boot-report operation")
    }
    fn hash(&self, _: &Path) -> Result<String> {
        panic!("not a boot-report operation")
    }
    fn spotify(&self, _: &Path) -> Result<()> {
        panic!("not a boot-report operation")
    }
}
#[test]
fn duration_parser_handles_compound_and_subsecond_units() {
    for (input, expected) in [
        ("1min 2.345s", 62.345),
        ("384ms", 0.384),
        ("10us", 0.00001),
        ("0s", 0.0),
    ] {
        assert!((boot_report::seconds(input).unwrap() - expected).abs() < 1e-9);
    }
    for bad in ["", "seconds", "1", "1ms garbage", "NaNs", "-1s"] {
        assert!(boot_report::seconds(bad).is_none(), "{bad}");
    }
}
#[test]
fn kernel_intervals_use_source_not_buffered_receipt_timestamps() {
    let rows = journal(vec![
        record("Trying to unpack rootfs image", 20000000, Some(1000000)),
        record("Freeing initrd memory:", 21000000, Some(1250000)),
        record("Reached target Initrd Root File System.", 5000000, None),
        record("Switching root.", 7000000, None),
        record("Switching root.", 8000000, None),
    ]);
    let report = boot_report::collect_boot(&rows).unwrap();
    assert_eq!(report["kernel_source_events_seconds"]["unpack_start"], 1.0);
    assert_eq!(
        report["intervals_seconds"]["initramfs_unpack_interval"],
        0.25
    );
    assert_eq!(
        report["intervals_seconds"]["root_ready_to_switch_root"],
        2.0
    );
    assert!(
        report["intervals_seconds"]
            .get("amdgpu_initialization")
            .is_none()
    );
}
#[test]
fn startup_stages_and_binary_messages_are_understood() {
    let report = boot_report::collect_boot(&stages(2)).unwrap();
    assert_eq!(report["stages_seconds"]["firmware"], 2.0);
    assert_eq!(report["stages_seconds"]["initrd"], 62.345);
    let mut row = record("", 3000000, None);
    row["MESSAGE"] = json!(b"Started Greeter daemon.".to_vec());
    let report = boot_report::collect_boot(&row.to_string()).unwrap();
    assert_eq!(report["journal_events_seconds"]["greetd_started"], 3.0);
    assert!(boot_report::collect_boot("not json").is_err());
    let mut row = record("Trying to unpack rootfs image", 20000000, None);
    row["_SOURCE_MONOTONIC_TIMESTAMP"] = json!("invalid");
    assert!(
        boot_report::collect_boot(&row.to_string()).unwrap()["kernel_source_events_seconds"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn historical_medians_are_finite_read_only_and_missing_boots_fail() {
    let mut r = Fake::default();
    for (i, time) in [1, 3, 5, 7].into_iter().enumerate() {
        r.boots.insert(
            format!("-{i}"),
            Reply {
                code: 0,
                text: stages(time),
            },
        );
    }
    let report = boot_report::collect(
        &Options {
            boots: 4,
            save: None,
        },
        &r,
    )
    .unwrap();
    assert_eq!(report["stage_medians_seconds"]["firmware"], 4.0);
    assert!(report["current_session"]["metrics"]["shell_launch_to_loaded_ms"].is_null());
    assert!(
        r.calls
            .borrow()
            .iter()
            .all(|c| ["journalctl", "systemctl", "systemd-analyze"].contains(&c[0].as_str()))
    );
    assert!(
        !r.calls
            .borrow()
            .iter()
            .flatten()
            .any(|arg| ["start", "stop", "restart", "enable", "ipc"].contains(&arg.as_str()))
    );
    assert!(
        boot_report::collect(
            &Options {
                boots: 5,
                save: None
            },
            &r
        )
        .unwrap_err()
        .to_string()
        .contains("Boot -4 journal unavailable")
    );
    assert!(
        boot_report::collect(
            &Options {
                boots: 0,
                save: None
            },
            &r
        )
        .is_err()
    );
    r.boots.insert(
        "-0".into(),
        Reply {
            code: 0,
            text: String::new(),
        },
    );
    assert!(
        boot_report::collect(
            &Options {
                boots: 1,
                save: None
            },
            &r
        )
        .unwrap_err()
        .to_string()
        .contains("no journal evidence")
    );
}
#[test]
fn options_reject_unsafe_labels_and_out_of_range_boot_counts() {
    for args in [
        vec!["--boots", "0"],
        vec!["--boots=21"],
        vec!["--boots=no"],
        vec!["--save", "../escape"],
        vec!["--boots"],
        vec!["--unknown=1"],
    ] {
        assert!(Options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err());
    }
    assert_eq!(
        Options::parse(&["--boots=2".into(), "--save=before".into()])
            .unwrap()
            .boots,
        2
    );
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "foundation-boot-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn snapshots_are_private_atomic_and_refuse_escaped_paths() {
    let temp = Temp::new();
    let dir = temp.0.join("samples");
    let report = json!({"boots":[{"boot_id":"abcd-1234"}]});
    assert!(boot_report::save(&report, "../escape", &dir).is_err());
    assert!(!dir.exists());
    assert!(
        boot_report::save(&json!({"boots":[{"boot_id":"../escape"}]}), "before", &dir).is_err()
    );
    assert!(!dir.exists());
    let path = boot_report::save(&report, "before", &dir).unwrap();
    assert_eq!(fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        report
    );
    let outside = temp.0.join("outside");
    fs::write(&outside, "untouched").unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    boot_report::save(&report, "before", &dir).unwrap();
    assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
    assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
}

#[test]
fn command_uses_xdg_cache_without_python_and_failed_collection_preserves_snapshot() {
    // The operational CLI deliberately refuses privileged user-tool execution.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let temp = Temp::new();
    let bin = temp.0.join("bin");
    fs::create_dir(&bin).unwrap();
    for (name, body) in [
        (
            "journalctl",
            "if [ \"$1\" = -b ] && [ \"$2\" = -0 ]; then printf '%s\\n' \"$DF_BOOT_FIXTURE\"; else exit 1; fi",
        ),
        ("systemctl", "exit 1"),
        ("systemd-analyze", "exit 1"),
    ] {
        let path = bin.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let cache = temp.0.join("custom-cache");
    let run = |boots: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
            .args(["boot-report", "--boots", boots, "--save", "before"])
            .env_clear()
            .env("HOME", &temp.0)
            .env("XDG_CACHE_HOME", &cache)
            .env("PATH", &bin)
            .env("DF_BOOT_FIXTURE", stages(2))
            .output()
            .unwrap()
    };
    let output = run("1");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let target = cache.join("desktop-foundation/boot-audit/optimization/before-abcd.json");
    let before = fs::read(&target).unwrap();
    assert!(!temp.0.join(".cache").exists());
    let output = run("2");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Boot -1 journal unavailable"));
    assert_eq!(fs::read(&target).unwrap(), before);
}

#[test]
fn nvidia_intervals_use_kernel_clock_without_claiming_causality() {
    let rows = journal(vec![
        record(
            "nvidia: loading out-of-tree module taints kernel",
            9000000,
            Some(2000000),
        ),
        record(
            "NVRM: loading NVIDIA UNIX Open Kernel Module",
            9500000,
            Some(3500000),
        ),
        record(
            "nvme nvme0: pci function 0000:01:00.0",
            9600000,
            Some(3600000),
        ),
        record("[drm] Initialized nvidia-drm 0.0.0", 9700000, Some(4000000)),
    ]);
    let report = boot_report::collect_boot(&rows).unwrap();
    assert_eq!(report["intervals_seconds"]["nvidia_module_to_drm"], 2.0);
    assert_eq!(
        report["intervals_seconds"]["nvidia_announcement_to_nvme_discovery"],
        0.1
    );
}
