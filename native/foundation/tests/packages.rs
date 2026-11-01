//! Package cache and deployed helper fixtures, without a real package manager.
use desktop_foundationctl::theme_render;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    bin: PathBuf,
    db: PathBuf,
    log: PathBuf,
    cache: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-package-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let bin = base.join("bin");
        let db = base.join("database");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(db.join("local")).unwrap();
        fs::create_dir_all(db.join("sync")).unwrap();
        fs::write(db.join("sync/core.db"), "initial").unwrap();
        let f = Self {
            cache: base.join("cache/desktop-foundation/fastfetch-foreign.json"),
            log: base.join("queries"),
            base,
            bin,
            db,
        };
        f.fake("pacman-conf", "#!/bin/sh\nif [ \"$1\" = DBPath ]; then printf '%s\\n' \"$DF_DATABASE\"; else printf '%s' \"$DF_REPOSITORIES\"; fi\n");
        f.fake("fastfetch", "#!/bin/sh\nif [ -n \"${DF_EXPECT_CONFIG:-}\" ]; then\n if [ \"${DF_EXPECT_CANONICAL:-}\" = 1 ]; then case \"$2\" in /*) ;; *) exit 9;; esac; [ \"$(/usr/bin/realpath \"$2\")\" = \"$DF_EXPECT_CONFIG\" ] || exit 9; else [ \"$2\" = \"$DF_EXPECT_CONFIG\" ] || exit 9; fi\nfi\nprintf '%s\\n' '1234 (pacman)'\n");
        f.fake("pacman", "#!/bin/sh\nprintf '%s\\n' query >> \"$DF_PACKAGE_LOG\"\ncase \"${DF_PACMAN_MODE:-one}\" in zero) exit 1;; error) printf '%s\\n' 'database error' >&2; exit 1;; esac\nprintf '%s\\n' foreign\n");
        f
    }
    fn fake(&self, name: &str, text: &str) {
        let file = self.bin.join(name);
        fs::write(&file, text).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn command(&self, executable: &Path) -> Command {
        let mut c = Command::new(executable);
        c.env("PATH", format!("{}:/usr/bin", self.bin.display()))
            .env("HOME", &self.base)
            .env("XDG_CACHE_HOME", self.base.join("cache"))
            .env("DF_DATABASE", &self.db)
            .env("DF_PACKAGE_LOG", &self.log)
            .env("DF_REPOSITORIES", "core\n")
            .env_remove("DF_PACMAN_MODE")
            .env_remove("DF_EXPECT_CONFIG")
            .env_remove("DF_EXPECT_CANONICAL");
        c
    }
    fn invoke(&self, extra: &[(&str, &str)]) -> Output {
        let mut c = self.command(Path::new(env!("CARGO_BIN_EXE_desktop-foundationctl")));
        c.args(["system", "packages"]);
        for (k, v) in extra {
            c.env(k, v);
        }
        c.output().unwrap()
    }
    fn checked(&self, extra: &[(&str, &str)]) -> String {
        let o = self.invoke(extra);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    }
    fn queries(&self) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn cache_reuses_and_invalidates_with_database_and_repository_changes() {
    let f = Fixture::new();
    assert_eq!(f.checked(&[]), "1234 (pacman), 1 (AUR)\n");
    f.checked(&[]);
    assert_eq!(f.queries(), 1);
    fs::create_dir(f.db.join("local/new-package")).unwrap();
    f.checked(&[]);
    assert_eq!(f.queries(), 2);
    fs::write(f.db.join("sync/core.db"), "repository update").unwrap();
    f.checked(&[]);
    assert_eq!(f.queries(), 3);
    f.checked(&[("DF_REPOSITORIES", "core\nextra\n")]);
    assert_eq!(f.queries(), 4);
}
#[test]
fn empty_transactions_and_query_errors_remain_distinct() {
    let f = Fixture::new();
    assert!(f.checked(&[("DF_PACMAN_MODE", "zero")]).contains("0 (AUR)"));
    f.checked(&[("DF_PACMAN_MODE", "zero")]);
    assert_eq!(f.queries(), 1);
    fs::write(f.db.join("db.lck"), "").unwrap();
    f.checked(&[("DF_PACMAN_MODE", "zero")]);
    f.checked(&[("DF_PACMAN_MODE", "zero")]);
    assert_eq!(f.queries(), 3);
    let result = f.invoke(&[("DF_PACMAN_MODE", "error")]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("database error"));
}
#[test]
fn legacy_cache_schema_and_invalid_counts_are_handled() {
    let f = Fixture::new();
    let signature: Vec<_> = [f.db.join("local"), f.db.join("sync/core.db")]
        .iter()
        .map(|path| {
            let meta = fs::metadata(path).unwrap();
            json!([
                path,
                meta.ino(),
                meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
                meta.ctime() * 1_000_000_000 + meta.ctime_nsec(),
                meta.len()
            ])
        })
        .collect();
    fs::create_dir_all(f.cache.parent().unwrap()).unwrap();
    fs::write(
        &f.cache,
        serde_json::to_vec(&json!({"signature":signature,"count":7})).unwrap(),
    )
    .unwrap();
    assert!(f.checked(&[]).contains("7 (AUR)"));
    assert_eq!(f.queries(), 0);
    fs::write(f.db.join("sync/core.db"), "changed content").unwrap();
    f.checked(&[]);
    let mut saved: Value = serde_json::from_slice(&fs::read(&f.cache).unwrap()).unwrap();
    assert_eq!(saved["count"], 1);
    assert_eq!(saved["signature"][0], signature[0]);
    assert_eq!(
        fs::metadata(&f.cache).unwrap().permissions().mode() & 0o777,
        0o600
    );
    for value in [json!(true), json!(-1), json!("5")] {
        saved["count"] = value;
        fs::write(&f.cache, serde_json::to_vec(&saved).unwrap()).unwrap();
        f.checked(&[]);
    }
    assert_eq!(f.queries(), 4);
}
fn create_checkout(f: &Fixture, name: &str) -> PathBuf {
    let root = f.base.join(name);
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = root.join("native/foundation/target/release/desktop-foundationctl");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_desktop-foundationctl"), binary).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("config/application-roles.json"), "{}").unwrap();
    theme_render::copy_tree(
        &original.join("terminal/fastfetch"),
        &root.join("terminal/fastfetch"),
    )
    .unwrap();
    root
}
fn invoke_helper(f: &Fixture, helper: &Path, cwd: Option<&Path>, config: &Path, canonical: bool) {
    let mut c = f.command(helper);
    if let Some(cwd) = cwd {
        c.current_dir(cwd);
    }
    c.env("DF_EXPECT_CONFIG", config)
        .env("DF_EXPECT_CANONICAL", if canonical { "1" } else { "0" });
    let result = c.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "1234 (pacman), 1 (AUR)\n"
    );
}
#[test]
fn relocated_and_relative_helpers_preserve_package_config() {
    let f = Fixture::new();
    let root = create_checkout(&f, "relocated");
    let installed = f.base.join("user-config");
    fs::create_dir(&installed).unwrap();
    symlink(root.join("terminal/fastfetch"), installed.join("fastfetch")).unwrap();
    invoke_helper(
        &f,
        &installed.join("fastfetch/packages"),
        None,
        &installed.join("fastfetch/packages.jsonc"),
        false,
    );
    let root = create_checkout(&f, "relative source checkout");
    let config = root.join("terminal/fastfetch/packages.jsonc");
    for helper in [
        "./terminal/fastfetch/packages",
        "terminal/fastfetch/packages",
    ] {
        invoke_helper(&f, Path::new(helper), Some(&root), &config, true);
    }
}
#[test]
fn generated_revision_keeps_owner_and_uses_revision_config() {
    let f = Fixture::new();
    let root = create_checkout(&f, "owner checkout with ' quote");
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["kitty", "fish"] {
        theme_render::copy_tree(
            &original.join("terminal").join(name),
            &root.join("terminal").join(name),
        )
        .unwrap();
    }
    for name in ["theme", "profiles", "compositor"] {
        symlink(original.join(name).canonicalize().unwrap(), root.join(name)).unwrap();
    }
    for name in ["input.lua", "window-appearance.lua", "monitor.toml"] {
        symlink(
            original.join("config").join(name).canonicalize().unwrap(),
            root.join("config").join(name),
        )
        .unwrap();
    }
    let revision = f.base.join("state/theme/revisions/example");
    fs::create_dir_all(&revision).unwrap();
    let semantic: Value =
        serde_json::from_slice(&fs::read(original.join("theme/fallback/semantic.json")).unwrap())
            .unwrap();
    theme_render::render(&root, &revision, &semantic, "default", true).unwrap();
    let current = f.base.join("state/theme/current");
    symlink(&revision, &current).unwrap();
    let installed = f.base.join("config");
    fs::create_dir(&installed).unwrap();
    symlink(
        current.join("terminal/fastfetch"),
        installed.join("fastfetch"),
    )
    .unwrap();
    invoke_helper(
        &f,
        &installed.join("fastfetch/packages"),
        None,
        &installed.join("fastfetch/packages.jsonc"),
        false,
    );
}
