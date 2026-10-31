//! Installer orchestration with inert effects; never invokes sudo or a real helper.
use desktop_foundationctl::{
    Result, deploy,
    install::{self, Options, Runtime},
    invalid,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    paths: deploy::Paths,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-installer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("checkout with spaces");
        for path in ["profiles/default", "packages", "config", "apps"] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        for group in ["core", "personal", "greeter", "firmware"] {
            fs::write(
                root.join(format!("packages/{group}.txt")),
                format!("# fixture\n{group}-package\n"),
            )
            .unwrap();
        }
        let app = json!({"package":"fixture-browser","executable":"fixture-browser","desktop":"fixture.desktop","mimes":[]});
        fs::write(
            root.join("config/application-roles.json"),
            json!({"browser":app,"personalBrowser":app,"terminal":app}).to_string(),
        )
        .unwrap();
        fs::write(
            root.join("config/app-mods.json"),
            json!({"spicetify":{"enabled":true},"geode":{"enabled":true}}).to_string(),
        )
        .unwrap();
        fs::write(
            root.join("apps/personal.json"),
            json!({"chatgpt":app,"steam":app,"vesktop":app}).to_string(),
        )
        .unwrap();
        let home = base.join("home");
        let paths = deploy::Paths {
            home: home.clone(),
            config: home.join("config"),
            state: home.join("state/desktop-foundation"),
            data: home.join("data"),
        };
        Self { base, root, paths }
    }
    fn options(&self, args: &[&str]) -> Options {
        Options::parse(
            &self.root,
            &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).unwrap();
    }
}
#[derive(Default)]
struct Fake {
    commands: Vec<Vec<String>>,
    queries: Vec<Vec<String>>,
    validations: RefCell<Vec<String>>,
    installed: bool,
    available: bool,
    variant: bool,
    conflict: bool,
    missing_app: bool,
    no_pacman: bool,
    root: bool,
    no_apparmor: bool,
    bad_boot: bool,
    failure: Option<String>,
}
impl Fake {
    fn named(&self, name: &str) -> bool {
        self.commands.iter().any(|c| {
            c.iter()
                .any(|s| Path::new(s).file_name().is_some_and(|v| v == name))
        })
    }
}
impl Runtime for Fake {
    fn query(&mut self, args: &[String]) -> Result<bool> {
        self.queries.push(args.to_vec());
        Ok(if args.get(1).map(String::as_str) == Some("-T") {
            self.installed
        } else {
            self.available
        })
    }
    fn command(&mut self, args: &[String]) -> Result<()> {
        self.commands.push(args.to_vec());
        if self.failure.as_ref().is_some_and(|name| {
            args.iter()
                .any(|s| Path::new(s).file_name().is_some_and(|v| v == name.as_str()))
        }) {
            return Err(invalid("fixture command failed"));
        }
        Ok(())
    }
    fn executable(&self, name: &str) -> bool {
        if name == "pacman" {
            !self.no_pacman
        } else {
            self.variant
        }
    }
    fn validate_app(&self, name: &str, _: &Value) -> Result<()> {
        self.validations.borrow_mut().push(name.into());
        if self.missing_app {
            Err(invalid("fixture application missing"))
        } else {
            Ok(())
        }
    }
    fn desktop_exists(&self, _: &str) -> Result<bool> {
        Ok(self.variant)
    }
    fn uid(&self) -> u32 {
        if self.root { 0 } else { 1000 }
    }
    fn conflicts(&self, root: &Path, paths: &deploy::Paths) -> Result<Vec<String>> {
        if self.conflict {
            Ok(vec!["fixture conflict".into()])
        } else {
            deploy::inspect(root, paths)
        }
    }
    fn apparmor_enabled(&self) -> bool {
        !self.no_apparmor
    }
    fn clean_boot_supported(&self) -> bool {
        !self.bad_boot
    }
}
#[test]
fn flags_preserve_layer_defaults_and_reject_unsafe_input() {
    let f = Fixture::new();
    assert!(f.options(&[]).personal);
    assert!(!f.options(&["--core"]).personal);
    assert!(!f.options(&["--no-greeter"]).greeter);
    assert_eq!(f.options(&["--profile=default"]).profile, "default");
    assert_eq!(
        f.options(&["--hardware-profile=default"]).profile,
        "default"
    );
    for args in [
        vec!["--core", "--personal"],
        vec!["--profile", "../default"],
        vec!["--dry-run", "--check"],
        vec!["--profile"],
        vec!["--no-such-option"],
    ] {
        assert!(
            Options::parse(
                &f.root,
                &args.iter().map(|s| (*s).into()).collect::<Vec<_>>()
            )
            .is_err()
        );
    }
}
#[test]
fn dry_run_is_read_only_and_only_calls_deployment_inspection() {
    let f = Fixture::new();
    let mut r = Fake::default();
    install::execute(&f.root, &f.paths, &f.options(&["--dry-run"]), &mut r).unwrap();
    assert!(!f.paths.home.exists());
    assert_eq!(r.commands.len(), 1);
    assert_eq!(Path::new(&r.commands[0][0]).file_name().unwrap(), "deploy");
    assert!(r.commands[0].contains(&"--dry-run".into()));
    assert!(!r.named("sudo"));
    assert!(!r.named("build-backend"));
}
#[test]
fn check_only_dispatches_read_only_doctor() {
    let f = Fixture::new();
    let mut r = Fake::default();
    install::execute(
        &f.root,
        &f.paths,
        &f.options(&["--check", "--core", "--no-greeter"]),
        &mut r,
    )
    .unwrap();
    assert_eq!(r.commands.len(), 1);
    assert!(r.named("doctor"));
    assert_eq!(
        &r.commands[0][1..],
        ["--installation", "--core", "--no-greeter"]
    );
    assert!(!f.paths.home.exists());
}
#[test]
fn core_without_packages_never_installs_packages_or_personal_adapters() {
    let f = Fixture::new();
    let mut r = Fake::default();
    let o = f.options(&["--core", "--no-greeter", "--no-packages"]);
    install::execute(&f.root, &f.paths, &o, &mut r).unwrap();
    for name in [
        "build-nothing",
        "build-backend",
        "preflight",
        "deploy",
        "preferences",
        "applications",
        "doctor",
    ] {
        assert!(r.named(name), "{name}");
    }
    for name in [
        "bootstrap",
        "paru-bootstrap",
        "install-app-mods",
        "browser-policy-setup",
        "boot-setup",
        "system-setup",
    ] {
        assert!(!r.named(name), "{name}");
    }
    assert!(
        !r.commands
            .iter()
            .any(|c| c.iter().any(|s| s == "pacman" || s == "paru"))
    );
    assert!(
        r.commands
            .iter()
            .all(|c| !c.iter().any(|s| s.contains("python") || s.ends_with(".py")))
    );
    assert_eq!(
        r.commands.last().unwrap()[1..],
        ["--installation", "--core", "--no-greeter"]
    );
}
#[test]
fn personal_without_packages_validates_first_and_uses_shared_modification_checks() {
    let f = Fixture::new();
    let mut r = Fake::default();
    install::execute(&f.root, &f.paths, &f.options(&["--no-packages"]), &mut r).unwrap();
    assert_eq!(
        *r.validations.borrow(),
        ["browser", "chatgpt", "steam", "vesktop"]
    );
    assert!(
        r.commands.iter().any(
            |c| c[0].ends_with("install-app-mods") && c[1..] == ["check", "--skip-unavailable"]
        )
    );
    assert!(r.named("browser-policy-setup"));
    assert!(r.named("theme-apply"));
    assert!(
        !r.commands
            .iter()
            .any(|c| c.iter().any(|s| s == "pacman" || s == "paru"))
    );
}
#[test]
fn ownership_missing_app_root_and_boot_conflicts_stop_before_sudo_or_builds() {
    let f = Fixture::new();
    for mut r in [
        Fake {
            conflict: true,
            ..Fake::default()
        },
        Fake {
            missing_app: true,
            ..Fake::default()
        },
        Fake {
            root: true,
            ..Fake::default()
        },
        Fake {
            no_pacman: true,
            ..Fake::default()
        },
        Fake {
            bad_boot: true,
            ..Fake::default()
        },
    ] {
        assert!(
            install::execute(
                &f.root,
                &f.paths,
                &f.options(&["--no-packages", "--clean-boot"]),
                &mut r
            )
            .is_err()
        );
        assert!(r.commands.is_empty());
    }
}
#[test]
fn native_ownership_preflight_rejects_another_checkout() {
    let f = Fixture::new();
    fs::create_dir_all(&f.paths.state).unwrap();
    fs::write(
        f.paths.state.join("manifest.json"),
        json!({"root":f.base.join("other"),"entries":[]}).to_string(),
    )
    .unwrap();
    let mut r = Fake::default();
    assert!(install::execute(&f.root, &f.paths, &f.options(&["--core"]), &mut r).is_err());
    assert!(r.commands.is_empty());
}
#[test]
fn failure_does_not_continue_to_deploy_or_report_success_through_doctor() {
    let f = Fixture::new();
    let mut r = Fake {
        failure: Some("preflight".into()),
        ..Fake::default()
    };
    assert!(
        install::execute(
            &f.root,
            &f.paths,
            &f.options(&["--core", "--no-packages"]),
            &mut r
        )
        .is_err()
    );
    assert!(!r.named("deploy"));
    assert!(!r.named("doctor"));
}
#[test]
fn signed_packages_aur_review_and_foreign_variant_rules_are_preserved() {
    let f = Fixture::new();
    let app = json!({"package":"fixture","executable":"fixture","desktop":"fixture.desktop"});
    let mut adopted = Fake {
        installed: true,
        ..Fake::default()
    };
    install::provision(&mut adopted, &f.root, "fixture", &app, true).unwrap();
    assert!(adopted.commands.is_empty());
    let mut foreign = Fake {
        variant: true,
        ..Fake::default()
    };
    assert!(install::provision(&mut foreign, &f.root, "fixture", &app, true).is_err());
    assert!(foreign.commands.is_empty());
    let mut signed = Fake {
        available: true,
        ..Fake::default()
    };
    install::provision(&mut signed, &f.root, "fixture", &app, false).unwrap();
    assert_eq!(
        signed.commands,
        [vec!["sudo", "pacman", "-Syu", "--needed", "fixture"]]
    );
    let mut aur = Fake::default();
    install::provision(&mut aur, &f.root, "fixture", &app, true).unwrap();
    assert!(aur.named("paru-bootstrap"));
    assert_eq!(aur.commands[1], ["paru", "-S", "--needed", "fixture"]);
    let mut absent = Fake::default();
    assert!(install::provision(&mut absent, &f.root, "fixture", &app, false).is_err());
    assert!(absent.commands.is_empty());
}
#[test]
fn configuration_override_and_clean_boot_are_explicit() {
    let f = Fixture::new();
    fs::create_dir_all(f.paths.config.join("desktop-foundation")).unwrap();
    fs::write(
        f.paths.config.join("desktop-foundation/app-mods.json"),
        json!({"geode":{"enabled":false}}).to_string(),
    )
    .unwrap();
    assert_eq!(
        desktop_foundationctl::app_mods::configured_at(&f.root, &f.paths.config).unwrap(),
        ["spicetify"]
    );
    let mut r = Fake::default();
    install::execute(
        &f.root,
        &f.paths,
        &f.options(&["--core", "--no-packages", "--clean-boot"]),
        &mut r,
    )
    .unwrap();
    for name in [
        "boot-setup",
        "greeter-console-setup",
        "boot-optimize",
        "system-setup",
    ] {
        assert!(r.named(name));
    }
    assert!(!r.named("apparmor-setup"));
}
#[test]
fn invalid_package_and_modification_inputs_are_rejected_before_commands() {
    let f = Fixture::new();
    fs::write(f.root.join("packages/core.txt"), "--overwrite\n").unwrap();
    let mut r = Fake::default();
    assert!(install::execute(&f.root, &f.paths, &f.options(&[]), &mut r).is_err());
    assert!(r.commands.is_empty());
    fs::write(f.root.join("packages/core.txt"), "package\n").unwrap();
    fs::write(
        f.root.join("config/app-mods.json"),
        "{\"geode\":{\"enabled\":\"yes\"}}",
    )
    .unwrap();
    assert!(install::execute(&f.root, &f.paths, &f.options(&[]), &mut r).is_err());
    assert!(r.commands.is_empty());
}

#[test]
fn core_ignores_unselected_personal_modification_configuration() {
    let f = Fixture::new();
    fs::write(f.root.join("config/app-mods.json"), "not JSON").unwrap();
    let mut r = Fake::default();
    install::execute(
        &f.root,
        &f.paths,
        &f.options(&["--core", "--no-packages", "--no-greeter"]),
        &mut r,
    )
    .unwrap();
    assert!(!r.named("install-app-mods"));
}

#[test]
fn real_cli_plan_needs_no_python_and_leaves_private_home_absent() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let f = Fixture::new();
    let bin = f.base.join("bin");
    fs::create_dir(&bin).unwrap();
    fs::create_dir(f.root.join("scripts")).unwrap();
    for file in [bin.join("pacman"), f.root.join("scripts/deploy")] {
        fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .env_clear()
        .env("HOME", &f.paths.home)
        .env("PATH", &bin)
        .args(["--root", f.root.to_str().unwrap(), "install", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Roles:"));
    assert!(!f.paths.home.exists());
}
