use desktop_foundationctl::cache::{self, CachePaths};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    paths: CachePaths,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "foundation-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("checkout");
        fs::create_dir_all(&root).unwrap();
        let paths = CachePaths {
            state: base.join("state"),
            cache: base.join("cache"),
        };
        for dir in [
            paths.state.join("theme/revisions"),
            paths.cache.join("themes"),
            paths.cache.join("overview"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        Self { base, root, paths }
    }
    fn revision(&self, n: u64, root: &Path) -> PathBuf {
        let path = self
            .paths
            .state
            .join(format!("theme/revisions/{n}-deadbeef"));
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("metadata.json"),
            serde_json::json!({"root":root}).to_string(),
        )
        .unwrap();
        fs::write(
            path.join("semantic.json"),
            serde_json::json!({"wallpaperHash":format!("{:064x}",n)}).to_string(),
        )
        .unwrap();
        fs::write(path.join("wallpaper.png"), [0; 1024]).unwrap();
        date(&path, n);
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn date(path: &Path, n: u64) {
    fs::File::open(path)
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(n)),
        )
        .unwrap();
}

#[test]
fn retention_preserves_old_active_rollback_foreign_and_unrecognized_files() {
    let fixture = Fixture::new();
    let mut revisions = Vec::new();
    for n in 0..12 {
        revisions.push(fixture.revision(n, &fixture.root));
    }
    symlink(&revisions[0], fixture.paths.state.join("theme/current")).unwrap();
    symlink(&revisions[1], fixture.paths.state.join("theme/previous")).unwrap();
    let foreign = fixture.revision(999, Path::new("/other-checkout"));
    let backup = fixture.paths.state.join("theme/revisions/manual-backup");
    fs::create_dir(&backup).unwrap();
    for (folder, suffix, count) in [
        ("themes", "-graphite-v1.json", 40),
        ("overview", ".png", 20),
    ] {
        let directory = fixture.paths.cache.join(folder);
        for n in 0..count {
            let path = directory.join(format!("{n:064x}{suffix}"));
            fs::write(&path, "fixture").unwrap();
            date(&path, n);
        }
        fs::write(directory.join("user-file"), "keep").unwrap();
        symlink(
            "/no-target",
            directory.join(format!("{:064x}{suffix}", 999)),
        )
        .unwrap();
    }
    fs::write(
        fixture.paths.cache.join("overview/backdrop.json"),
        serde_json::json!({"image":format!("file:///cache/{:064x}.png",0)}).to_string(),
    )
    .unwrap();
    let plan = cache::plan(&fixture.root, &fixture.paths).unwrap();
    assert_eq!(plan.retained_revisions, 6);
    assert_eq!(plan.retained_palettes, 32);
    assert_eq!(plan.retained_overviews, 16);
    assert_eq!(plan.removals.len(), 6 + 8 + 4);
    assert!(plan.bytes > 0);
    assert!(
        revisions.iter().all(|p| p.exists()),
        "Planning must not delete anything"
    );
    let removed: Vec<_> = plan.removals.iter().map(|p| p.path.clone()).collect();
    cache::prune(&fixture.root, &fixture.paths).unwrap();
    assert!(removed.iter().all(|p| !p.exists()));
    assert!(revisions[0].exists() && revisions[1].exists() && foreign.exists() && backup.exists());
    assert!(fixture.paths.cache.join("themes/user-file").exists());
    assert!(
        fixture
            .paths
            .cache
            .join(format!("overview/{:064x}.png", 0))
            .exists()
    );
    assert!(
        cache::plan(&fixture.root, &fixture.paths)
            .unwrap()
            .removals
            .is_empty()
    );
}

#[test]
fn foreign_or_broken_pointer_and_bad_metadata_block_all_removal() {
    let fixture = Fixture::new();
    let current = fixture.revision(0, &fixture.root);
    for n in 1..12 {
        fixture.revision(n, &fixture.root);
    }
    symlink(&current, fixture.paths.state.join("theme/current")).unwrap();
    let revisions = fixture.paths.state.join("theme/revisions");
    let before = fs::read_dir(&revisions).unwrap().count();
    fs::write(current.join("metadata.json"), "{\"root\":\"/foreign\"}").unwrap();
    assert!(cache::prune(&fixture.root, &fixture.paths).is_err());
    fs::write(
        current.join("metadata.json"),
        serde_json::json!({"root":fixture.root}).to_string(),
    )
    .unwrap();
    symlink(
        revisions.join("missing"),
        fixture.paths.state.join("theme/previous"),
    )
    .unwrap();
    assert!(cache::prune(&fixture.root, &fixture.paths).is_err());
    fs::remove_file(fixture.paths.state.join("theme/previous")).unwrap();
    fs::write(revisions.join("1-deadbeef/metadata.json"), "invalid").unwrap();
    assert!(cache::prune(&fixture.root, &fixture.paths).is_err());
    assert_eq!(fs::read_dir(&revisions).unwrap().count(), before);
}

#[test]
fn uninitialized_state_is_an_empty_read_only_plan() {
    let fixture = Fixture::new();
    fs::remove_dir_all(&fixture.paths.state).unwrap();
    assert!(
        cache::plan(&fixture.root, &fixture.paths)
            .unwrap()
            .removals
            .is_empty()
    );
    assert!(
        cache::prune(&fixture.root, &fixture.paths)
            .unwrap()
            .removals
            .is_empty()
    );
    assert!(!fixture.paths.state.exists());
}
