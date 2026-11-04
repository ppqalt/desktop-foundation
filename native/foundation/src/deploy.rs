//! Write-ahead deployment journal, compatible with existing Python installations.
//! Configuration links are adopted only through this journal, never inferred.
use crate::{Result, deploy_units, invalid, preferences, process, state, theme_runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const COMMAND: Duration = Duration::from_secs(30);
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub path: PathBuf,
    pub source: PathBuf,
    pub backup: Option<PathBuf>,
    #[serde(default)]
    pub original: Option<[u64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_source: Option<PathBuf>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositor: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
    pub data: PathBuf,
    pub home: PathBuf,
}
impl Paths {
    pub fn environment() -> Result<Self> {
        Ok(Self {
            config: state::xdg("XDG_CONFIG_HOME", ".config")?,
            state: state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation"),
            data: state::xdg("XDG_DATA_HOME", ".local/share")?,
            home: state::home()?,
        })
    }
}
pub fn fingerprint(path: &Path) -> Result<Option<[u64; 3]>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some([meta.dev(), meta.ino(), u64::from(meta.mode())])),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn owned(entry: &Entry) -> bool {
    fs::read_link(&entry.path).is_ok_and(|source| {
        source == entry.source || entry.pending_source.as_ref() == Some(&source)
    })
}
pub fn read(paths: &Paths) -> Result<Option<Manifest>> {
    let path = paths.state.join("manifest.json");
    let Some(meta) = fingerprint(&path)? else {
        return Ok(None);
    };
    if meta[2] & libc::S_IFMT as u64 != libc::S_IFREG as u64 {
        return Err(invalid("Deployment journal must be a regular file"));
    }
    let mut file = Vec::new();
    File::open(&path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut file)?;
    if file.len() > 4 * 1024 * 1024 {
        return Err(invalid("Deployment journal is too large"));
    }
    let manifest: Manifest = serde_json::from_slice(&file)?;
    let mut targets = HashSet::new();
    for entry in &manifest.entries {
        if !entry.path.is_absolute()
            || !entry.source.is_absolute()
            || entry
                .pending_source
                .as_ref()
                .is_some_and(|path| !path.is_absolute())
            || !targets.insert(entry.path.clone())
        {
            return Err(invalid("Invalid or duplicate deployment entry"));
        }
        if let Some(backup) = &entry.backup {
            let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if backup.parent() != Some(&paths.state)
                || !name
                    .strip_prefix("backup-")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
            {
                return Err(invalid("Invalid deployment backup path"));
            }
        }
    }
    Ok(Some(manifest))
}
pub fn save(paths: &Paths, manifest: &Manifest) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(manifest)?;
    bytes.push(b'\n');
    state::atomic(&paths.state.join("manifest.json"), &bytes, 0o600)
}
pub fn require_backends(root: &Path) -> Result<()> {
    for (path, build) in [
        (
            "native/foundation/target/release/desktop-foundationctl",
            "scripts/build-backend",
        ),
        (
            "native/nothing/target/release/foundation-nothing",
            "scripts/build-nothing",
        ),
    ] {
        if !root
            .join(path)
            .metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        {
            return Err(invalid(format!(
                "Required backend is not built or executable. Run {build}; nothing deployed."
            )));
        }
    }
    Ok(())
}
fn run(args: &[String]) -> Result<()> {
    process::checked(args, None, COMMAND, true)?;
    Ok(())
}
fn systemctl(args: &[&str]) -> Result<()> {
    let mut command = vec!["systemctl".into(), "--user".into()];
    command.extend(args.iter().map(|a| (*a).into()));
    run(&command)
}
fn common(root: &Path, paths: &Paths) -> Vec<deploy_units::Target> {
    vec![
        (
            paths.home.join(".local/bin/df-update"),
            root.join("scripts/df-update"),
        ),
        (
            paths
                .config
                .join("fontconfig/conf.d/60-desktop-foundation.conf"),
            root.join("theme/fontconfig/60-desktop-foundation.conf"),
        ),
        (
            paths.config.join("gtk-3.0/settings.ini"),
            root.join("theme/gtk-3.0/settings.ini"),
        ),
        (
            paths.config.join("gtk-4.0/settings.ini"),
            root.join("theme/gtk-4.0/settings.ini"),
        ),
        (
            paths
                .config
                .join("wireplumber/wireplumber.conf.d/60-desktop-foundation-bluetooth.conf"),
            root.join("audio/wireplumber/60-desktop-foundation-bluetooth.conf"),
        ),
        (
            paths.config.join("kitty"),
            paths.state.join("theme/current/terminal/kitty"),
        ),
        (paths.config.join("fish"), paths.state.join("terminal/fish")),
        (
            paths.config.join("fastfetch"),
            paths.state.join("theme/current/terminal/fastfetch"),
        ),
        (
            paths.data.join("fonts/desktop-foundation"),
            root.join("fonts"),
        ),
    ]
}
pub fn targets(
    root: &Path,
    paths: &Paths,
    manifest: &Manifest,
    write: bool,
) -> Result<Vec<deploy_units::Target>> {
    let mut targets = vec![
        (
            paths.config.join("niri/config.kdl"),
            paths.state.join("niri.kdl"),
        ),
        (
            paths.config.join("quickshell/desktop-foundation"),
            root.join("shell"),
        ),
    ];
    let recorded = manifest
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect();
    targets.extend(deploy_units::targets(
        root,
        &paths.config,
        &paths.state,
        &paths.data,
        &recorded,
        write,
    )?);
    targets.extend(common(root, paths));
    Ok(targets)
}
fn initial(root: &Path) -> Manifest {
    Manifest {
        root: root.into(),
        entries: Vec::new(),
        profile: None,
        compositor: None,
        extra: BTreeMap::new(),
    }
}
pub fn inspect(root: &Path, paths: &Paths) -> Result<Vec<String>> {
    let Some(manifest) = read(paths)? else {
        return Ok(Vec::new());
    };
    if manifest.root != root {
        return Ok(vec![
            "A different checkout owns deployment; restore it first".into(),
        ]);
    }
    Ok(manifest
        .entries
        .iter()
        .filter(|e| !owned(e))
        .map(|e| format!("{}", e.path.display()))
        .collect())
}
/// Link one target. The original inode and backup name are durable before rename.
pub fn install_link(
    paths: &Paths,
    manifest: &mut Manifest,
    path: &Path,
    source: &Path,
) -> Result<()> {
    if let Some(index) = manifest.entries.iter().position(|e| e.path == path) {
        if !owned(&manifest.entries[index]) {
            return Err(invalid(format!(
                "Deployed path changed outside setup: {}",
                path.display()
            )));
        }
        if manifest.entries[index].source != source
            || manifest.entries[index].pending_source.is_some()
        {
            manifest.entries[index].pending_source = Some(source.into());
            save(paths, manifest)?;
            let temporary = path.with_file_name(format!(
                ".foundation-link-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            symlink(source, &temporary)?;
            if let Err(error) = fs::rename(&temporary, path) {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
            File::open(
                path.parent()
                    .ok_or_else(|| invalid("Missing target parent"))?,
            )?
            .sync_all()?;
            manifest.entries[index].source = source.into();
            manifest.entries[index].pending_source = None;
            save(paths, manifest)?;
        }
        return Ok(());
    }
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("Missing target parent"))?,
    )?;
    let original = fingerprint(path)?;
    let backup = original.map(|_| {
        paths
            .state
            .join(format!("backup-{}", manifest.entries.len()))
    });
    if let Some(backup) = &backup
        && fingerprint(backup)?.is_some()
    {
        return Err(invalid(format!(
            "Unexpected backup exists: {}",
            backup.display()
        )));
    }
    manifest.entries.push(Entry {
        path: path.into(),
        source: source.into(),
        backup: backup.clone(),
        original,
        pending_source: None,
        extra: BTreeMap::new(),
    });
    save(paths, manifest)?;
    if let Some(backup) = backup {
        fs::rename(path, backup)?;
    }
    symlink(source, path)?;
    File::open(path.parent().expect("target parent"))?.sync_all()?;
    Ok(())
}
pub fn validate_restore(manifest: &mut Manifest) -> Result<()> {
    for entry in &mut manifest.entries {
        let backup = entry
            .backup
            .as_deref()
            .map(fingerprint)
            .transpose()?
            .flatten();
        if entry.original.is_none() && entry.backup.is_some() {
            entry.original = backup;
            if backup.is_none() {
                return Err(invalid(
                    "Legacy original backup is missing; journal retained",
                ));
            }
        }
        let actual = fingerprint(&entry.path)?;
        let original_present = entry.original.is_some() && actual == entry.original;
        if actual.is_some() && !owned(entry) && !original_present {
            return Err(invalid(format!(
                "Refusing to replace modified config: {}",
                entry.path.display()
            )));
        }
        if original_present && backup.is_some() {
            return Err(invalid(format!(
                "Ambiguous recovery: {}",
                entry.path.display()
            )));
        }
        if let Some(original) = entry.original
            && !original_present
            && backup != Some(original)
        {
            return Err(invalid(format!(
                "Original backup is missing or changed: {}",
                entry.path.display()
            )));
        }
    }
    Ok(())
}
pub fn restore_files(paths: &Paths, manifest: &mut Manifest) -> Result<()> {
    validate_restore(manifest)?;
    save(paths, manifest)?;
    while let Some(entry) = manifest.entries.last().cloned() {
        if owned(&entry) {
            fs::remove_file(&entry.path)?;
        }
        if let Some(backup) = &entry.backup
            && fingerprint(backup)?.is_some()
        {
            fs::rename(backup, &entry.path)?;
        }
        if let Some(parent) = entry.path.parent()
            && parent.is_dir()
        {
            File::open(parent)?.sync_all()?;
        }
        manifest.entries.pop();
        save(paths, manifest)?;
    }
    fs::remove_file(paths.state.join("manifest.json"))?;
    File::open(&paths.state)?.sync_all()?;
    Ok(())
}
fn restore(root: &Path, paths: &Paths, manifest: &mut Manifest) -> Result<()> {
    if manifest.root != root {
        return Err(invalid("A different checkout owns this installation"));
    }
    validate_restore(manifest)?;
    save(paths, manifest)?;
    let units: BTreeSet<_> = manifest
        .entries
        .iter()
        .filter(|entry| {
            owned(entry)
                && entry.path.starts_with(paths.config.join("systemd/user"))
                && !entry
                    .path
                    .starts_with(paths.config.join("systemd/user/niri.service.wants"))
        })
        .filter_map(|entry| entry.path.file_name().and_then(|n| n.to_str()))
        .filter(|name| name.starts_with("desktop-foundation-") && !name.contains('@'))
        .collect();
    if !units.is_empty() {
        let mut stop = vec!["stop"];
        stop.extend(units);
        systemctl(&stop)?;
        let _ = systemctl(&[
            "stop",
            "desktop-foundation-clipboard@text.service",
            "desktop-foundation-clipboard@image.service",
        ]);
    }
    restore_files(paths, manifest)?;
    systemctl(&["daemon-reload"])?;
    run(&["fc-cache".into(), "-f".into()])?;
    println!("Original configuration restored.");
    Ok(())
}
fn fish(root: &Path, paths: &Paths) -> Result<()> {
    let runtime = paths.state.join("terminal/fish");
    fs::create_dir_all(&runtime)?;
    for entry in fs::read_dir(root.join("terminal/fish"))? {
        let entry = entry?;
        let target = runtime.join(entry.file_name());
        let source = entry.path();
        if entry.file_name() == "theme.fish" {
            if fs::read_link(&target).ok().as_ref() == Some(&source) {
                fs::remove_file(&target)?;
            }
            if fingerprint(&target)?.is_none() {
                symlink(
                    paths.state.join("theme/current/terminal/fish/theme.fish"),
                    &target,
                )?;
            }
        } else if fingerprint(&target)?.is_none() {
            symlink(source, target)?;
        }
    }
    run(&[
        "fish".into(),
        "--no-config".into(),
        "-n".into(),
        root.join("terminal/fish/config.fish")
            .to_string_lossy()
            .into_owned(),
    ])
}
pub fn run_command(root: &Path, args: &[String]) -> Result<()> {
    let mut action = "install";
    let mut profile = "default";
    let mut dry = false;
    let mut update = false;
    let mut index = 0;
    if let Some(first) = args.first()
        && ["install", "restore"].contains(&first.as_str())
    {
        action = first;
        index = 1;
    }
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => dry = true,
            "--update" => update = true,
            "--profile" => {
                index += 1;
                profile = args.get(index).ok_or_else(|| invalid("Missing profile"))?;
            }
            "--compositor" => {
                index += 1;
                if args.get(index).map(String::as_str) != Some("niri") {
                    return Err(invalid("Desktop Foundation supports Niri only"));
                }
            }
            _ => {
                return Err(invalid(
                    "Expected deploy [install|restore] [--profile NAME] [--compositor niri] [--dry-run] [--update]",
                ));
            }
        }
        index += 1;
    }
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(invalid("Invalid profile name"));
    }
    let root = root.canonicalize()?;
    let paths = Paths::environment()?;
    if dry {
        let manifest = read(&paths)?.unwrap_or_else(|| initial(&root));
        for (path, source) in targets(&root, &paths, &manifest, false)? {
            println!(
                "Deploy with backup: {} <- {}",
                path.display(),
                source.display()
            );
        }
        return Ok(());
    }
    if action == "install" {
        require_backends(&root)?;
    }
    state::private_directory(&paths.state)?;
    let _lock = state::lock(&paths.state.join("lock"), Duration::from_secs(30))?;
    let existing = read(&paths)?;
    if update && existing.is_none() {
        return Err(invalid(
            "No existing deployment to update; run the installer first",
        ));
    }
    let mut manifest = existing.unwrap_or_else(|| initial(&root));
    if manifest.root != root {
        return Err(invalid(
            "A different checkout owns this installation; restore it first",
        ));
    }
    if action == "restore" {
        if paths.state.join("manifest.json").exists() {
            return restore(&root, &paths, &mut manifest);
        }
        println!("Nothing deployed.");
        return Ok(());
    }
    for entry in &manifest.entries {
        if !owned(entry) {
            return Err(invalid(format!(
                "Deployed path changed externally: {}. Restore/recover the journal before reinstalling.",
                entry.path.display()
            )));
        }
    }
    theme_runtime::ensure(&root, profile)?;
    run(&[
        root.join("scripts/monitor").to_string_lossy().into_owned(),
        "prepare".into(),
    ])?;
    let wrapper = format!(
        "include {}\n",
        serde_json::to_string(&paths.state.join("theme/current/niri.kdl"))?
    );
    let candidate = paths.state.join(format!(
        ".candidate-{}-{}.kdl",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    crate::theme_render::write(&candidate, &wrapper)?;
    let validation = run(&[
        "niri".into(),
        "validate".into(),
        "-c".into(),
        candidate.to_string_lossy().into_owned(),
    ]);
    let _ = fs::remove_file(candidate);
    validation?;
    let targets = targets(&root, &paths, &manifest, true)?;
    fish(&root, &paths)?;
    crate::theme_render::write(&paths.state.join("niri.kdl"), &wrapper)?;
    let operation = (|| {
        for (path, source) in &targets {
            install_link(&paths, &mut manifest, path, source)?;
        }
        Ok(())
    })();
    if let Err(error) = operation {
        if !update && let Err(recovery) = restore(&root, &paths, &mut manifest) {
            eprintln!("Deployment recovery journal retained: {recovery}");
        }
        return Err(error);
    }
    systemctl(&["daemon-reload"])?;
    if systemctl(&["is-active", "--quiet", "desktop-foundation-session.target"]).is_ok() {
        let _ = systemctl(&[
            "stop",
            "desktop-foundation-clipboard@text.service",
            "desktop-foundation-clipboard@image.service",
        ]);
        systemctl(&["restart", "desktop-foundation-clipboard.service"])?;
    }
    if !update {
        preferences::setup(&root, &["install".into(), "--theme-only".into()])?;
    }
    manifest.profile = Some(profile.into());
    manifest.compositor = Some("niri".into());
    save(&paths, &manifest)?;
    run(&["fc-cache".into(), "-f".into()])?;
    println!(
        "Deployed Niri profile {profile}; backups: {}",
        paths.state.display()
    );
    Ok(())
}
