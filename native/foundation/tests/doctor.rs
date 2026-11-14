//! Inert diagnostic transport: no real services, desktop controls or account data.
use desktop_foundationctl::{
    Result, deploy,
    doctor::{self, Options, Reply, Runtime},
    invalid, startup_report,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Default)]
struct Fake {
    calls: RefCell<Vec<Vec<String>>>,
    replies: BTreeMap<String, String>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}
impl Runtime for Fake {
    fn query(&self, args: &[String]) -> Reply {
        self.calls.borrow_mut().push(args.to_vec());
        if let Some(text) = self.replies.get(&args.join(" ")) {
            return Reply {
                code: 0,
                text: text.clone(),
            };
        }
        if args.first().is_some_and(|s| s == "systemctl") && args.iter().any(|s| s == "is-active") {
            return Reply {
                code: if args.last().unwrap() == "xdg-desktop-portal.service" {
                    3
                } else {
                    0
                },
                text: "fixture service".into(),
            };
        }
        Reply {
            code: 1,
            text: "fixture unavailable".into(),
        }
    }
    fn read(&self, p: &Path) -> Result<Vec<u8>> {
        self.files
            .get(p)
            .cloned()
            .ok_or_else(|| invalid("fixture missing file"))
    }
    fn exists(&self, p: &Path) -> bool {
        self.files.contains_key(p)
    }
    fn executable(&self, _: &str) -> bool {
        true
    }
    fn env(&self, k: &str) -> Option<String> {
        (k == "USER").then(|| "fixture".into())
    }
    fn protected_helper(&self, _: &Path) -> bool {
        true
    }
    fn role_check(&self, _: &Path, _: bool) -> Result<Value> {
        Ok(json!({"messages":[],"failures":[]}))
    }
    fn validate_role(&self, _: &str, _: &Value) -> Result<()> {
        Ok(())
    }
    fn conflicts(&self, _: &Path, _: &deploy::Paths) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn mods(&self, _: &Path, _: &Path) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn hash(&self, _: &Path) -> Result<String> {
        Ok("fixture".into())
    }
    fn spotify(&self, _: &Path) -> Result<()> {
        Ok(())
    }
}
#[test]
fn options_preserve_modes_and_reject_ambiguous_requests() {
    for args in [
        vec!["--installation", "--personal"],
        vec!["--json"],
        vec!["--startup", "--save=before"],
        vec!["--startup", "--benchmark"],
    ] {
        assert!(Options::parse(&doctor::words(&args)).is_ok());
    }
    for args in [
        vec!["--core", "--personal"],
        vec!["--json", "--installation"],
        vec!["--benchmark"],
        vec!["--save"],
        vec!["--unknown"],
    ] {
        assert!(Options::parse(&doctor::words(&args)).is_err());
    }
}
#[test]
fn empty_instances_differ_from_invalid_responses() {
    for s in [
        "",
        "[]",
        "No running instances for \"/checkout with spaces/shell/shell.qml\"\nUse --all to list all instances.\n",
    ] {
        assert_eq!(doctor::shell_instance_count(s).unwrap(), 0);
    }
    assert_eq!(doctor::shell_instance_count("[{\"pid\":1}]").unwrap(), 1);
    for s in ["null", "{}", "invalid"] {
        assert!(doctor::shell_instance_count(s).is_err());
    }
}
#[test]
fn missing_pam_and_portal_are_actionable_and_session_is_read_only() {
    let r = Fake::default();
    let p = deploy::Paths {
        config: "/fixture/config".into(),
        state: "/fixture/state".into(),
        data: "/fixture/data".into(),
        home: "/fixture".into(),
    };
    let report = doctor::session(Path::new("/fixture/checkout"), &p, &r);
    assert!(
        report
            .failures
            .iter()
            .any(|s| s.contains("greetd PAM configuration unreadable"))
    );
    assert!(
        report
            .failures
            .iter()
            .any(|s| s.contains("xdg-desktop-portal: fixture service; inspect journalctl"))
    );
    for call in r.calls.borrow().iter() {
        assert!(
            !call.iter().any(|s| [
                "start",
                "restart",
                "stop",
                "enable",
                "disable",
                "connect",
                "disconnect",
                "set",
                "set-volume"
            ]
            .contains(&s.as_str())),
            "{call:?}"
        );
    }
}
#[test]
fn journal_messages_and_missing_timings_are_preserved() {
    assert_eq!(
        startup_report::decode_message(&json!([27, 91, 51, 49, 109, 72, 105, 27, 91, 48, 109])),
        "Hi"
    );
    assert_eq!(
        startup_report::decode_message(&json!("\u{1b}[2Jkeep")),
        "\u{1b}[2Jkeep"
    );
    let report = startup_report::collect(&Fake::default());
    assert!(report["metrics"]["shell_launch_to_loaded_ms"].is_null());
    assert!(!report["diagnostic_notes"].as_array().unwrap().is_empty());
}
#[test]
fn startup_timeline_uses_current_login_and_filters_stale_shell_logs() {
    let mut r = Fake::default();
    for (unit, start, active) in [
        ("niri.service", 10, 11),
        ("desktop-foundation-shell.service", 12, 13),
    ] {
        r.replies.insert(format!("systemctl --user show {unit} -p ExecMainStartTimestampMonotonic -p ActiveEnterTimestampMonotonic -p Type -p LoadState"),format!("ExecMainStartTimestampMonotonic={}\nActiveEnterTimestampMonotonic={}",start*1000000,active*1000000));
    }
    let row = |time: u64, unit: &str, msg: &str| {
        json!({"__MONOTONIC_TIMESTAMP":(time*1000000).to_string(),"_SYSTEMD_USER_UNIT":unit,"MESSAGE":msg}).to_string()
    };
    r.replies.insert("journalctl --user -b -o json --no-pager --grep Reload requested from client|Reloading finished in|listening on Wayland socket:|Configuration Loaded|DF_FRAME_SUBMITTED surface=".into(),[
 row(2,"desktop-foundation-shell.service","Configuration Loaded"),row(11,"niri.service","listening on Wayland socket: wayland-1"),row(14,"desktop-foundation-shell.service","Configuration Loaded"),row(15,"desktop-foundation-shell.service","DF_FRAME_SUBMITTED surface=clock")].join("\n"));
    r.replies.insert(
        "journalctl -b -o json --no-pager -u greetd.service -u sddm.service --grep session opened for user ".into(),
        row(9, "", "session opened for user fixture(uid=1000)"),
    );
    let report = startup_report::collect(&r);
    assert_eq!(report["origin"], "session_open");
    assert_eq!(report["metrics"]["shell_launch_to_loaded_ms"], 2000.0);
    assert_eq!(report["timeline_ms"]["niri_active"], 2000.0);
    assert_eq!(
        report["metrics"]["shell_launch_to_first_frame_submitted_ms"],
        3000.0
    );
}
#[test]
fn benchmark_failure_cannot_open_a_surface() {
    let r = Fake::default();
    assert!(startup_report::benchmark(Path::new("/fixture"), &r).is_err());
    assert!(
        r.calls
            .borrow()
            .iter()
            .all(|args| args.last().unwrap().ends_with("Status"))
    );
}

#[test]
fn installation_reports_corrupt_journal_and_theme_hash_mismatch_without_session_probe() {
    let root =
        std::env::temp_dir().join(format!("foundation-doctor-install-{}", std::process::id()));
    std::fs::create_dir_all(root.join("packages")).unwrap();
    std::fs::write(
        root.join("packages/core.toml"),
        "packages = [\"fixture-package\"]\n",
    )
    .unwrap();
    let paths = deploy::Paths {
        config: root.join("config"),
        state: root.join("state"),
        data: root.join("data"),
        home: root.clone(),
    };
    let mut r = Fake::default();
    r.files.insert(
        paths.state.join("application-roles.json"),
        b"not json".to_vec(),
    );
    r.files.insert(
        paths.state.join("theme/current/metadata.json"),
        br#"{"wallpaperHash":"other"}"#.to_vec(),
    );
    r.files.insert(
        paths.state.join("theme/current/wallpaper.toml"),
        b"image = '/fixture/wallpaper.png'".to_vec(),
    );
    let report = doctor::installation(
        &root,
        &paths,
        &Options {
            installation: true,
            no_greeter: true,
            ..Default::default()
        },
        &r,
    );
    assert!(
        report
            .failures
            .iter()
            .any(|s| s.contains("Application role journal"))
    );
    assert!(
        report
            .failures
            .iter()
            .any(|s| s.contains("Runtime wallpaper content hash"))
    );
    assert!(
        report
            .failures
            .iter()
            .any(|s| s.contains("Theme output semantic.json"))
    );
    assert!(
        !r.calls
            .borrow()
            .iter()
            .any(|args| args.first().is_some_and(|s| s == "quickshell"))
    );
    r.files.insert(
        PathBuf::from("/proc/sys/kernel/osrelease"),
        b"7.3.3-2-cachyos-eac".to_vec(),
    );
    r.files.insert(
        PathBuf::from("/proc/cmdline"),
        b"root=UUID=fixture lsm=landlock,lockdown,yama,integrity,bpf".to_vec(),
    );
    let eac = doctor::installation(
        &root,
        &paths,
        &Options {
            installation: true,
            no_greeter: true,
            ..Default::default()
        },
        &r,
    );
    assert!(
        eac.lines
            .iter()
            .any(|line| line.starts_with("PASS: EAC LSM policy"))
    );
    assert!(!eac.failures.iter().any(|line| line.contains("AppArmor")));
    r.files
        .insert(PathBuf::from("/proc/cmdline"), b"lsm=apparmor".to_vec());
    let bad = doctor::installation(
        &root,
        &paths,
        &Options {
            installation: true,
            no_greeter: true,
            ..Default::default()
        },
        &r,
    );
    assert!(
        bad.failures
            .iter()
            .any(|line| line.contains("EAC LSM policy"))
    );
    std::fs::remove_dir_all(root).unwrap();
}
