//! Guarded retention for reproducible caches and immutable theme revisions.
//! Current/previous and foreign files are never candidates for removal.
use crate::{Result, invalid, state};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub const REVISIONS: usize = 6;
pub const PALETTES: usize = 32;
pub const OVERVIEWS: usize = 16;

#[derive(Deserialize)]
struct ThemeRevision {
    root: PathBuf,
}

#[derive(Deserialize)]
struct PaletteReference {
    #[serde(rename = "wallpaperHash")]
    wallpaper_hash: Option<String>,
}
#[derive(Deserialize)]
struct BackdropReference {
    image: Option<String>,
}

#[derive(Serialize)]
pub struct Removal {
    pub path: PathBuf,
    pub bytes: u64,
    pub kind: &'static str,
}

#[derive(Default, Serialize)]
pub struct CachePlan {
    pub removals: Vec<Removal>,
    pub bytes: u64,
    pub retained_revisions: usize,
    pub retained_palettes: usize,
    pub retained_overviews: usize,
}

pub struct CachePaths {
    pub state: PathBuf,
    pub cache: PathBuf,
}
impl CachePaths {
    pub fn from_environment() -> Result<Self> {
        Ok(Self {
            state: state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation"),
            cache: state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation"),
        })
    }
}

fn real_directory(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(invalid(format!(
            "Cache directory is not a real directory: {}",
            path.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn revision(s: &str) -> bool {
    s.split_once('-').is_some_and(|(time, suffix)| {
        !time.is_empty()
            && time.len() <= 20
            && time.bytes().all(|c| c.is_ascii_digit())
            && suffix.len() == 8
            && suffix
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

fn metadata(path: &Path) -> Result<ThemeRevision> {
    read_json(&path.join("metadata.json"))
}

fn read_json<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T> {
    use std::io::Read;
    if !fs::symlink_metadata(file)?.is_file() {
        return Err(invalid(format!(
            "Cache metadata must be a regular file: {}",
            file.display()
        )));
    }
    let mut bytes = Vec::new();
    fs::File::open(file)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err(invalid("Theme metadata is too large"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn bytes(path: &Path) -> Result<u64> {
    let m = fs::symlink_metadata(path)?;
    if m.is_dir() {
        fs::read_dir(path)?.try_fold(
            0u64,
            |n, entry| Ok(n.saturating_add(bytes(&entry?.path())?)),
        )
    } else if m.is_file() {
        Ok(m.len())
    } else {
        Ok(0)
    }
}

fn retain(
    plan: &mut CachePlan,
    mut entries: Vec<PathBuf>,
    protected: &BTreeSet<PathBuf>,
    limit: usize,
    kind: &'static str,
) -> Result<usize> {
    // A cache hit keeps its timestamp; active/rollback revisions remain protected
    // regardless of age. Ties use the path for a deterministic plan.
    let mut dated = Vec::new();
    for path in entries.drain(..) {
        dated.push((fs::metadata(&path)?.modified()?, path));
    }
    dated.sort_by(|a, b| b.cmp(a));
    let mut kept = protected.len();
    for (_, path) in dated {
        if protected.contains(&path) {
            continue;
        }
        if kept < limit {
            kept += 1;
            continue;
        }
        let size = bytes(&path)?;
        plan.bytes = plan.bytes.saturating_add(size);
        plan.removals.push(Removal {
            path,
            bytes: size,
            kind,
        });
    }
    Ok(kept)
}

pub fn plan(root: &Path, paths: &CachePaths) -> Result<CachePlan> {
    let root = root.canonicalize()?;
    let theme = paths.state.join("theme");
    if !real_directory(&paths.state)? || !real_directory(&theme)? {
        return Ok(CachePlan::default());
    }
    let revisions = theme.join("revisions");
    if !real_directory(&revisions)? {
        return Err(invalid("Managed theme has no revisions directory"));
    }
    let revisions = revisions.canonicalize()?;
    let mut protected = BTreeSet::new();
    for pointer in ["current", "previous"] {
        let link = theme.join(pointer);
        let link_metadata = match fs::symlink_metadata(&link) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && pointer == "previous" => continue,
            Err(e) => return Err(e.into()),
        };
        if !link_metadata.file_type().is_symlink() {
            return Err(invalid(
                "Theme pointers must be symlinks; caches left intact",
            ));
        }
        let target = link.canonicalize()?;
        if target.parent() != Some(revisions.as_path())
            || !target
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(revision)
            || metadata(&target)?.root != root
        {
            return Err(invalid(
                "Another checkout or foreign path owns theme pointers; caches left intact",
            ));
        }
        protected.insert(target);
    }
    let mut owned = Vec::new();
    for entry in fs::read_dir(&revisions)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.file_name().to_str().is_some_and(revision) {
            let path = entry.path();
            if metadata(&path)?.root == root {
                owned.push(path);
            }
        }
    }
    let mut plan = CachePlan::default();
    plan.retained_revisions = retain(&mut plan, owned, &protected, REVISIONS, "theme-revision")?;
    // Cache files are reproducible; unrelated names, symlinks and manifests stay.
    if real_directory(&paths.cache)? {
        for (folder, suffix, limit, kind) in [
            ("themes", "-graphite-v1.json", PALETTES, "palette"),
            ("overview", ".png", OVERVIEWS, "overview"),
        ] {
            let directory = paths.cache.join(folder);
            if !real_directory(&directory)? {
                continue;
            }
            let mut files = Vec::new();
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                if entry.file_type()?.is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|s| s.strip_suffix(suffix).is_some_and(hash))
                {
                    files.push(entry.path());
                }
            }
            let mut protected_files = BTreeSet::new();
            if folder == "themes" {
                for path in &protected {
                    let semantic: PaletteReference = read_json(&path.join("semantic.json"))?;
                    if let Some(digest) = semantic.wallpaper_hash.as_deref().filter(|s| hash(s)) {
                        let file = directory.join(format!("{digest}{suffix}"));
                        if files.contains(&file) {
                            protected_files.insert(file);
                        }
                    }
                }
            } else {
                let manifest = directory.join("backdrop.json");
                if manifest.is_file() {
                    let value: BackdropReference = read_json(&manifest)?;
                    if let Some(image) = value.image.as_deref().and_then(|s| s.rsplit('/').next()) {
                        let file = directory.join(image);
                        if files.contains(&file) {
                            protected_files.insert(file);
                        }
                    }
                }
            }
            let kept = retain(&mut plan, files, &protected_files, limit, kind)?;
            if folder == "themes" {
                plan.retained_palettes = kept;
            } else {
                plan.retained_overviews = kept;
            }
        }
    }
    Ok(plan)
}

pub fn prune(root: &Path, paths: &CachePaths) -> Result<CachePlan> {
    if !real_directory(&paths.state)? {
        return Ok(CachePlan::default());
    }
    let _lock = state::lock(&paths.state.join("theme.lock"), Duration::from_secs(5))?;
    let plan = plan(root, paths)?; // Validate the entire plan before any removal.
    for removal in &plan.removals {
        if removal.kind == "theme-revision" {
            fs::remove_dir_all(&removal.path)?;
        } else {
            fs::remove_file(&removal.path)?;
        }
    }
    Ok(plan)
}
