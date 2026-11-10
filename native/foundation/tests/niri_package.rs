//! Package and compositor effects are inert; only private fixture files change.
use desktop_foundationctl::{
    Result, input_config, invalid,
    niri_package::{self, Artifact, Effects, Paths, Phase},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn sha(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    paths: Paths,
    e: Fake,
}
struct Fake {
    cache: PathBuf,
    source: PathBuf,
    binary: PathBuf,
    name: String,
    version: String,
    builds: usize,
    calls: Vec<Vec<String>>,
    bad_signature: bool,
    bad_identity: bool,
    bad_validation: bool,
    fail_build: bool,
    fail_after: bool,
    fail_before: bool,
    fail_recovery: bool,
    fail_render: bool,
}
impl Fake {
    fn payload(path: &Path) -> Result<Vec<u8>> {
        let bytes = fs::read(path)?;
        if bytes == b"stock-archive" {
            Ok(b"stock-binary".to_vec())
        } else if bytes.starts_with(b"candidate-archive-") {
            Ok([
                b"candidate-binary-".as_slice(),
                &bytes[b"candidate-archive-".len()..],
            ]
            .concat())
        } else {
            Err(invalid("bad archive"))
        }
    }
    fn stock(&mut self) {
        self.name = "niri".into();
        self.version = "26.04-1.1".into();
        fs::write(&self.binary, b"stock-binary").unwrap();
    }
    fn mutations(&self) -> usize {
        self.calls
            .iter()
            .filter(|c| c.first().is_some_and(|v| v == "sudo"))
            .count()
    }
}
impl Effects for Fake {
    fn capture(&mut self, args: &[String]) -> Result<Vec<u8>> {
        self.calls.push(args.to_vec());
        let parts: Vec<_> = args.iter().map(String::as_str).collect();
        match parts.as_slice() {
            ["pacman", "-Q", name] if *name == self.name => {
                Ok(format!("{} {}\n", self.name, self.version).into_bytes())
            }
            ["pacman", "-Q", _] => Err(invalid("not installed")),
            ["pacman-conf", "CacheDir"] => Ok(format!("{}\n", self.cache.display()).into_bytes()),
            ["pacman", "-Qp", path] => {
                let bytes = fs::read(path)?;
                Ok(if bytes == b"stock-archive" {
                    b"niri 26.04-1.1\n".to_vec()
                } else if self.bad_identity {
                    b"foreign-niri 26.04-1\n".to_vec()
                } else {
                    b"niri-foundation 26.04-1\n".to_vec()
                })
            }
            ["pacman-key", "--verify", signature, _] => {
                if self.bad_signature || fs::read(signature)? != b"valid signature" {
                    Err(invalid("signature failed"))
                } else {
                    Ok(vec![])
                }
            }
            ["bsdtar", "-xOf", path, "usr/bin/niri"] => Self::payload(Path::new(path)),
            [program, "validate", "-c", _] if program.ends_with("candidate-niri") => {
                if self.bad_validation {
                    Err(invalid("candidate rejects configuration"))
                } else {
                    Ok(vec![])
                }
            }
            ["niri", "msg", "action", "load-config-file"] => Ok(vec![]),
            _ => Err(invalid(format!("Unexpected fixture query: {parts:?}"))),
        }
    }
    fn interactive(&mut self, args: &[String]) -> Result<()> {
        self.calls.push(args.to_vec());
        if args.len() != 4 || args[..3] != ["sudo", "pacman", "-U"] {
            return Err(invalid("Unexpected privileged operation"));
        }
        let path = Path::new(&args[3]);
        let data = fs::read(path)?;
        let stock = data == b"stock-archive";
        if (!stock && self.fail_before) || (stock && self.fail_recovery) {
            return Err(invalid("inert package failure"));
        }
        fs::write(&self.binary, Self::payload(path)?)?;
        self.name = if stock { "niri" } else { "niri-foundation" }.into();
        self.version = if stock { "26.04-1.1" } else { "26.04-1" }.into();
        if !stock && self.fail_after {
            self.fail_after = false; // Fail the attempted update, not its recovery.
            return Err(invalid("inert post-transaction failure"));
        }
        Ok(())
    }
    fn build(&mut self, _: &Path) -> Result<PathBuf> {
        self.builds += 1;
        if self.fail_build {
            Err(invalid("inert compiler failure"))
        } else {
            Ok(self.source.clone())
        }
    }
    fn render(&mut self, _: &Path, _: &str) -> Result<()> {
        self.calls.push(vec!["render".into()]);
        if self.fail_render {
            Err(invalid("inert render failure"))
        } else {
            Ok(())
        }
    }
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "df-niri-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("source");
        let cache = base.join("cache");
        let config = base.join("config");
        for path in [
            &root.join("patches/niri"),
            &cache,
            &config.join("niri"),
            &config.join("desktop-foundation"),
        ] {
            fs::create_dir_all(path).unwrap();
        }
        fs::write(
            root.join("patches/niri/PKGBUILD"),
            "pkgver=26.04\npkgrel=1\n",
        )
        .unwrap();
        fs::write(
            root.join("patches/niri/overview-ctrl-scroll.patch"),
            "fixture patch",
        )
        .unwrap();
        fs::write(
            cache.join("niri-26.04-1.1-x86_64.pkg.tar.zst"),
            b"stock-archive",
        )
        .unwrap();
        fs::write(
            cache.join("niri-26.04-1.1-x86_64.pkg.tar.zst.sig"),
            b"valid signature",
        )
        .unwrap();
        let source = base.join("candidate.pkg.tar.zst");
        fs::write(&source, b"candidate-archive-v1").unwrap();
        let binary = base.join("niri");
        fs::write(&binary, b"stock-binary").unwrap();
        fs::write(config.join("niri/config.kdl"), "working custom config").unwrap();
        Self {
            root,
            paths: Paths {
                directory: base.join("state/desktop-foundation/niri-package"),
                config,
                binary: binary.clone(),
            },
            e: Fake {
                cache,
                source,
                binary,
                name: "niri".into(),
                version: "26.04-1.1".into(),
                builds: 0,
                calls: vec![],
                bad_signature: false,
                bad_identity: false,
                bad_validation: false,
                fail_build: false,
                fail_after: false,
                fail_before: false,
                fail_recovery: false,
                fail_render: false,
            },
            base,
        }
    }
    fn install(&mut self) -> Result<()> {
        niri_package::install(&self.root, &self.paths, None, &mut self.e)
    }
    fn journal(&self) -> niri_package::Journal {
        niri_package::read(&self.paths).unwrap().unwrap()
    }
    fn save(&self, j: &niri_package::Journal) {
        fs::write(
            self.paths.directory.join("journal.json"),
            serde_json::to_vec(j).unwrap(),
        )
        .unwrap();
    }
    fn owner(&self) {
        let state = self.paths.directory.parent().unwrap();
        fs::create_dir_all(state.join("theme")).unwrap();
        fs::write(
            state.join("manifest.json"),
            serde_json::to_vec(&serde_json::json!({"root":self.root})).unwrap(),
        )
        .unwrap();
        fs::write(
            state.join("theme/profile.json"),
            "{\"profile\":\"default\"}",
        )
        .unwrap();
    }
    fn input(&self) -> bool {
        input_config::load(
            serde_json::json!({"layout":"fi","repeatRate":35,"repeatDelay":350}),
            Path::new("base"),
            &self.root.join("profile"),
            &self.paths.config,
        )
        .unwrap()
        .settings
        .overview_ctrl_scroll
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn installs_only_after_signed_backup_and_configuration_validation_and_reruns_are_idle() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let j = f.journal();
    assert_eq!(j.phase, Phase::Installed);
    assert!(
        !f.paths
            .config
            .join("desktop-foundation/niri-package-input.toml")
            .exists()
    );
    assert_eq!(fs::read(&j.original.archive).unwrap(), b"stock-archive");
    assert_eq!(
        fs::metadata(&j.original.archive)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let mutation = f.e.calls.iter().position(|c| c[0] == "sudo").unwrap();
    assert!(f.e.calls[..mutation].iter().any(|c| c[0] == "pacman-key"));
    assert_eq!(
        f.e.calls[..mutation]
            .iter()
            .filter(|c| c.get(1).is_some_and(|v| v == "validate"))
            .count(),
        2
    );
    f.install().unwrap();
    assert_eq!(f.e.builds, 1);
    assert_eq!(f.e.mutations(), 1);
    assert!(!f.e.calls.iter().any(|c| c[0] == "systemctl"));
}
#[test]
fn bad_signature_build_identity_or_validation_never_reaches_package_installation() {
    for failure in 0..4 {
        let mut f = Fixture::new();
        match failure {
            0 => f.e.bad_signature = true,
            1 => f.e.fail_build = true,
            2 => f.e.bad_identity = true,
            _ => f.e.bad_validation = true,
        };
        assert!(f.install().is_err());
        assert_eq!(f.e.mutations(), 0);
        assert_eq!(fs::read(&f.paths.binary).unwrap(), b"stock-binary");
        assert!(!f.paths.directory.join("journal.json").exists());
    }
}
#[test]
fn failed_install_before_or_after_mutation_recovers_exact_stock_and_retries_cached_build() {
    for after in [false, true] {
        let mut f = Fixture::new();
        f.e.fail_after = after;
        f.e.fail_before = !after;
        assert!(f.install().is_err());
        assert_eq!(f.journal().phase, Phase::Restored);
        assert_eq!(fs::read(&f.paths.binary).unwrap(), b"stock-binary");
        f.e.fail_after = false;
        f.e.fail_before = false;
        f.install().unwrap();
        assert_eq!(f.e.builds, 1);
        assert_eq!(f.journal().phase, Phase::Installed);
    }
}
#[test]
fn failed_recovery_keeps_journal_and_can_adopt_an_already_verified_transaction() {
    let mut f = Fixture::new();
    f.e.fail_after = true;
    f.e.fail_recovery = true;
    assert!(f.install().is_err());
    assert_eq!(f.journal().phase, Phase::Prepared);
    assert!(f.journal().original.archive.is_file());
    f.e.fail_after = false;
    f.e.fail_recovery = false;
    let mutations = f.e.mutations();
    f.install().unwrap();
    assert_eq!(f.journal().phase, Phase::Installed);
    assert_eq!(f.e.mutations(), mutations);
}
#[test]
fn interrupted_prepared_transaction_reuses_candidate_without_losing_stock_backup() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let mut j = f.journal();
    j.phase = Phase::Prepared;
    let original = j.original.sha256.clone();
    f.save(&j);
    f.e.stock();
    f.install().unwrap();
    assert_eq!(f.e.builds, 1);
    assert_eq!(f.journal().original.sha256, original);
}
#[test]
fn recipe_update_rolls_back_to_previous_patched_package_without_rebasing_original() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let original = f.journal().original.sha256;
    let old_recipe = f.journal().recipe;
    fs::write(
        f.root.join("patches/niri/overview-ctrl-scroll.patch"),
        "new fixture patch",
    )
    .unwrap();
    fs::write(&f.e.source, b"candidate-archive-v2").unwrap();
    f.e.fail_after = true;
    assert!(f.install().is_err());
    assert_eq!(fs::read(&f.paths.binary).unwrap(), b"candidate-binary-v1");
    assert_eq!(f.journal().recipe, old_recipe);
    assert_eq!(f.journal().original.sha256, original);
    assert_eq!(f.journal().phase, Phase::Installed);
}
#[test]
fn restore_is_idempotent_preserves_preferences_and_masks_unsupported_stock_option() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let user = f.paths.config.join("desktop-foundation/input.toml");
    fs::write(&user, "overview_ctrl_scroll = true\n").unwrap();
    niri_package::restore(&f.root, &f.paths, &mut f.e).unwrap();
    assert!(!f.input());
    assert_eq!(
        fs::read_to_string(&user).unwrap(),
        "overview_ctrl_scroll = true\n"
    );
    let count = f.e.mutations();
    niri_package::restore(&f.root, &f.paths, &mut f.e).unwrap();
    assert_eq!(f.e.mutations(), count);
    assert_eq!(f.journal().phase, Phase::Restored);
    f.install().unwrap();
    assert_eq!(f.journal().phase, Phase::Installed);
    assert_eq!(f.e.builds, 1);
}
#[test]
fn corrupt_backup_or_external_configuration_is_preserved_and_blocks_restoration() {
    for backup in [true, false] {
        let mut f = Fixture::new();
        f.install().unwrap();
        if backup {
            fs::write(&f.journal().original.archive, "changed").unwrap();
        } else {
            fs::write(
                f.paths
                    .config
                    .join("desktop-foundation/niri-package-input.toml"),
                "external contents",
            )
            .unwrap();
        }
        let count = f.e.mutations();
        assert!(niri_package::restore(&f.root, &f.paths, &mut f.e).is_err());
        assert_eq!(f.e.mutations(), count);
        assert_eq!(fs::read(&f.paths.binary).unwrap(), b"candidate-binary-v1");
    }
}
#[test]
fn unmanaged_input_policy_and_symlink_candidate_are_not_adopted() {
    let mut f = Fixture::new();
    fs::write(
        f.paths
            .config
            .join("desktop-foundation/niri-package-input.toml"),
        "overview_ctrl_scroll=true",
    )
    .unwrap();
    assert!(f.install().is_err());
    assert_eq!(f.e.mutations(), 0);
    fs::remove_file(
        f.paths
            .config
            .join("desktop-foundation/niri-package-input.toml"),
    )
    .unwrap();
    let link = f.base.join("link.pkg.tar.zst");
    symlink(&f.e.source, &link).unwrap();
    assert!(niri_package::install(&f.root, &f.paths, Some(&link), &mut f.e).is_err());
    assert_eq!(f.e.mutations(), 0);
}
#[test]
fn externally_replaced_compositor_is_not_overwritten_during_upgrade() {
    let mut f = Fixture::new();
    f.install().unwrap();
    fs::write(&f.paths.binary, "custom replacement").unwrap();
    fs::write(
        f.root.join("patches/niri/overview-ctrl-scroll.patch"),
        "new patch",
    )
    .unwrap();
    let count = f.e.mutations();
    assert!(f.install().is_err());
    assert_eq!(f.e.mutations(), count);
    assert_eq!(fs::read(&f.paths.binary).unwrap(), b"custom replacement");
}
#[test]
fn activation_waits_for_verified_running_binary_and_preserves_explicit_preferences() {
    let mut f = Fixture::new();
    f.install().unwrap();
    f.owner();
    let original = f.journal().original.binary_sha256;
    let candidate = f.journal().candidate.binary_sha256;
    niri_package::activate_with(&f.root, &f.paths, Some(&original), &mut f.e).unwrap();
    assert_eq!(f.journal().capability, None);
    assert!(!f.input());
    niri_package::activate_with(&f.root, &f.paths, Some(&candidate), &mut f.e).unwrap();
    assert!(f.input());
    fs::write(
        f.paths.config.join("desktop-foundation/input.toml"),
        "overview_ctrl_scroll=false\n",
    )
    .unwrap();
    assert!(!f.input());
    let renders = f.e.calls.iter().filter(|c| c[0] == "render").count();
    niri_package::activate_with(&f.root, &f.paths, Some(&candidate), &mut f.e).unwrap();
    assert_eq!(
        f.e.calls.iter().filter(|c| c[0] == "render").count(),
        renders
    );
}
#[test]
fn activation_retries_after_interrupted_render_and_rejects_another_deployment_owner() {
    let mut f = Fixture::new();
    f.install().unwrap();
    f.owner();
    let candidate = f.journal().candidate.binary_sha256;
    f.e.fail_render = true;
    assert!(niri_package::activate_with(&f.root, &f.paths, Some(&candidate), &mut f.e).is_err());
    assert!(f.journal().activation_pending);
    f.e.fail_render = false;
    niri_package::activate_with(&f.root, &f.paths, Some(&candidate), &mut f.e).unwrap();
    assert!(!f.journal().activation_pending);
    let other = f.base.join("other");
    fs::create_dir(&other).unwrap();
    assert!(niri_package::activate_with(&other, &f.paths, Some(&candidate), &mut f.e).is_err());
}
#[test]
fn prepared_upgrade_retains_previous_package_history_during_retry() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let previous = f.journal().candidate;
    let previous_recipe = f.journal().recipe;
    fs::write(
        f.root.join("patches/niri/overview-ctrl-scroll.patch"),
        "v2 patch",
    )
    .unwrap();
    fs::write(&f.e.source, b"candidate-archive-v2").unwrap();
    let staged = f.paths.directory.join("prepared-v2.pkg.tar.zst");
    fs::copy(&f.e.source, &staged).unwrap();
    let mut j = f.journal();
    j.phase = Phase::Prepared;
    j.previous = Some(previous.clone());
    j.previous_recipe = Some(previous_recipe.clone());
    let mut recipe = fs::read(f.root.join("patches/niri/PKGBUILD")).unwrap();
    recipe.extend(fs::read(f.root.join("patches/niri/overview-ctrl-scroll.patch")).unwrap());
    j.recipe = sha(&recipe);
    j.candidate = Artifact {
        name: "niri-foundation".into(),
        version: "26.04-1".into(),
        archive: staged,
        sha256: sha(b"candidate-archive-v2"),
        binary_sha256: sha(b"candidate-binary-v2"),
        signature: None,
        signature_sha256: None,
    };
    f.save(&j);
    f.e.fail_after = true;
    assert!(f.install().is_err());
    assert_eq!(f.e.builds, 1);
    assert_eq!(f.journal().candidate.binary_sha256, previous.binary_sha256);
    assert_eq!(f.journal().recipe, previous_recipe);
    assert_eq!(f.journal().phase, Phase::Installed);
}

#[test]
fn native_activation_uses_peer_credentials_and_does_not_touch_unmatched_session() {
    let mut f = Fixture::new();
    f.install().unwrap();
    let socket = f.base.join("ipc.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"))
        .args(["--root", root.to_str().unwrap(), "niri-package", "activate"])
        .env("XDG_STATE_HOME", f.base.join("state"))
        .env("XDG_CONFIG_HOME", &f.paths.config)
        .env("NIRI_SOCKET", &socket)
        .output()
        .unwrap();
    // Root operation is intentionally rejected; ordinary-user tests reach the
    // mismatched peer-executable check. Neither path can mutate a desktop.
    if unsafe { libc::geteuid() } != 0 {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(f.journal().capability, None);
}

#[test]
fn invalid_recovery_metadata_is_an_error_before_any_privileged_effect() {
    for invalid in 0..3 {
        let mut f = Fixture::new();
        f.install().unwrap();
        let mut j = f.journal();
        match invalid {
            0 => j.previous = Some(j.candidate.clone()),
            1 => j.recipe = "not-a-digest".into(),
            _ => j.original.name = "--unsafe-argument".into(),
        };
        f.save(&j);
        let count = f.e.mutations();
        assert!(niri_package::restore(&f.root, &f.paths, &mut f.e).is_err());
        assert_eq!(f.e.mutations(), count);
    }
}

#[test]
fn managed_capability_file_is_bounded_and_rejects_unexpected_keys() {
    let f = Fixture::new();
    let path = f
        .paths
        .config
        .join("desktop-foundation/niri-package-input.toml");
    for contents in [
        "overview_ctrl_scroll='yes'".to_string(),
        "overview_ctrl_scroll=true\nlayout='changed'".to_string(),
        "#".repeat(5000),
    ] {
        fs::write(&path, contents).unwrap();
        assert!(
            input_config::load(
                serde_json::json!({"layout":"fi","repeatRate":35,"repeatDelay":350}),
                Path::new("base"),
                &f.root.join("profile"),
                &f.paths.config
            )
            .is_err()
        );
    }
}

#[test]
fn native_capture_extracts_real_package_binary_above_snapshot_limit() {
    let f = Fixture::new();
    let member = f.base.join("usr/bin");
    fs::create_dir_all(&member).unwrap();
    let content = vec![0x5a; 17 * 1024 * 1024];
    fs::write(member.join("niri"), &content).unwrap();
    let archive = f.base.join("real.tar");
    assert!(
        Command::new("tar")
            .args(["-cf"])
            .arg(&archive)
            .arg("-C")
            .arg(&f.base)
            .arg("usr/bin/niri")
            .status()
            .unwrap()
            .success()
    );
    let mut native = niri_package::Native { gui_auth: false };
    let extracted = native
        .capture(&[
            "bsdtar".into(),
            "-xOf".into(),
            archive.to_str().unwrap().into(),
            "usr/bin/niri".into(),
        ])
        .unwrap();
    assert_eq!(extracted, content);
    assert!(native.capture(&["/bin/true".into()]).unwrap().is_empty());
}
