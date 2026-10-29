//! Update only Desktop Foundation, using the installation's build/deploy helpers.
use crate::{Result, invalid, process, state};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{OpenOptionsExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SHORT: Duration = Duration::from_secs(30);
const BUILD: Duration = Duration::from_secs(1800);
const BINARIES: [&str; 4] = [
    "native/foundation/target/release/desktop-foundationctl",
    "native/foundation/target/release/foundation-oo7-ready",
    "native/foundation/target/release/foundation-browser-policy",
    "native/nothing/target/release/foundation-nothing",
];

fn text(args: &[String]) -> Result<String> {
    let output = process::checked(args, None, SHORT, true)?;
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| invalid("Update metadata is not UTF-8"))
}
fn git(root: &Path, args: &[&str]) -> Vec<String> {
    let mut command = vec![
        "git".into(),
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "-C".into(),
        root.to_string_lossy().into_owned(),
    ];
    command.extend(args.iter().map(|s| (*s).into()));
    command
}
fn read(path: &Path) -> Result<Value> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 1024 * 1024 {
        return Err(invalid("Invalid installation metadata"));
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("Installation metadata is too large"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
struct Installation {
    profile: String,
    compositor: String,
}
fn installation(root: &Path, home: &Path) -> Result<Installation> {
    let manifest = read(&home.join("manifest.json")).map_err(|e| {
        invalid(format!(
            "Cannot update an unrecognized installation: {e}. Run the installer first."
        ))
    })?;
    if manifest["root"].as_str() != root.to_str() {
        return Err(invalid("This checkout does not own the installation"));
    }
    let entries = manifest["entries"]
        .as_array()
        .ok_or_else(|| invalid("Invalid deployment entries"))?;
    for entry in entries {
        let path = entry["path"]
            .as_str()
            .ok_or_else(|| invalid("Invalid deployment path"))?;
        let source = fs::read_link(path).map_err(|_| {
            invalid(format!(
                "Deployed path changed outside Desktop Foundation: {path}"
            ))
        })?;
        if source.to_str() != entry["source"].as_str()
            && source.to_str() != entry["pending_source"].as_str()
        {
            return Err(invalid(format!(
                "Deployed path changed outside Desktop Foundation: {path}"
            )));
        }
    }
    let profile = if let Some(profile) = manifest["profile"].as_str() {
        profile.to_owned()
    } else {
        read(&home.join("theme/profile.json"))?["profile"]
            .as_str()
            .ok_or_else(|| invalid("Cannot identify the installed hardware profile"))?
            .to_owned()
    };
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid("Invalid installed hardware profile"));
    }
    let config = state::xdg("XDG_CONFIG_HOME", ".config")?;
    let has = |path: &Path| {
        entries
            .iter()
            .any(|entry| entry["path"].as_str() == path.to_str())
    };
    let niri = has(&config.join("niri/config.kdl"));
    let hypr = has(&config.join("hypr/hyprland.lua"));
    let compositor = match manifest["compositor"].as_str() {
        Some("niri") => "niri",
        Some("hyprland") => "hyprland",
        None if niri && (!hypr || env::var_os("NIRI_SOCKET").is_some()) => "niri",
        None if hypr && (!niri || env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()) => {
            "hyprland"
        }
        _ => {
            return Err(invalid(
                "Cannot identify the installed compositor; redeploy with the selected compositor first",
            ));
        }
    };
    Ok(Installation {
        profile,
        compositor: compositor.into(),
    })
}
fn checkout_revision(root: &Path) -> Result<String> {
    if text(&git(root, &["rev-parse", "--show-toplevel"]))? != root.to_string_lossy() {
        return Err(invalid("df-update requires the owning Git checkout"));
    }
    if text(&git(root, &["symbolic-ref", "--short", "HEAD"]))? != "main" {
        return Err(invalid(
            "Switch to main before running df-update; other branches are preserved",
        ));
    }
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
    ] {
        let path = PathBuf::from(text(&git(root, &["rev-parse", "--git-path", name]))?);
        if (if path.is_absolute() {
            path
        } else {
            root.join(path)
        })
        .exists()
        {
            return Err(invalid(
                "Finish the in-progress Git operation before updating",
            ));
        }
    }
    let remote = text(&git(root, &["config", "--get", "remote.origin.url"]))?;
    if ![
        "https://github.com/ppqalt/desktop-foundation",
        "https://github.com/ppqalt/desktop-foundation.git",
        "git@github.com:ppqalt/desktop-foundation.git",
        "ssh://git@github.com/ppqalt/desktop-foundation.git",
    ]
    .contains(&remote.as_str())
    {
        return Err(invalid(
            "origin must point to ppqalt/desktop-foundation on GitHub; no remote was changed",
        ));
    }
    text(&git(root, &["rev-parse", "HEAD"]))
}
// Diffs include staged and unstaged edits separately; their bytes never go to
// the terminal. Build helpers cannot silently race an edit in the live checkout.
fn diff(root: &Path, cached: bool) -> Result<Vec<u8>> {
    let mut args = vec![
        "--no-pager",
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--binary",
    ];
    if cached {
        args.push("--cached");
    }
    args.push("HEAD");
    Ok(process::checked_capture(&git(root, &args), SHORT, 16 * 1024 * 1024)?.stdout)
}
fn fingerprint(path: &Path) -> Result<(Vec<u8>, u32)> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok((fs::read_link(path)?.as_os_str().as_bytes().to_vec(), 0));
    }
    if !meta.is_file() {
        return Err(invalid(
            "Unsupported untracked directory/device; keep nested repositories outside this checkout or ignore them",
        ));
    }
    let mut hash = Sha256::new();
    let mut file = File::open(path)?;
    let mut buffer = [0_u8; 32768];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok((hash.finalize().to_vec(), meta.permissions().mode() & 0o777))
}
type LocalFiles = Vec<(PathBuf, (Vec<u8>, u32))>;
fn untracked(root: &Path) -> Result<LocalFiles> {
    let output = process::checked_capture(
        &git(root, &["ls-files", "--others", "--exclude-standard", "-z"]),
        SHORT,
        16 * 1024 * 1024,
    )?;
    output
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|bytes| {
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(bytes));
            if !path
                .components()
                .all(|p| matches!(p, std::path::Component::Normal(_)))
            {
                return Err(invalid("Invalid untracked project path"));
            }
            let signature = fingerprint(&root.join(&path))?;
            Ok((path, signature))
        })
        .collect()
}
fn overlay_files(root: &Path, target: &Path, files: &LocalFiles) -> Result<()> {
    for (relative, signature) in files {
        let source = root.join(relative);
        let destination = target.join(relative);
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(invalid(format!(
                "Upstream conflicts with the untracked file {}; original unchanged",
                relative.display()
            )));
        }
        let mut parent = target.to_path_buf();
        for part in relative.parent().unwrap().components() {
            parent.push(part);
            match fs::symlink_metadata(&parent) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => (),
                Ok(_) => {
                    return Err(invalid(format!(
                        "Upstream changed a parent directory of {}",
                        relative.display()
                    )));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&parent)?,
                Err(e) => return Err(e.into()),
            }
        }
        if signature.1 == 0 && fs::symlink_metadata(&source)?.file_type().is_symlink() {
            symlink(fs::read_link(source)?, &destination)?;
        } else {
            fs::copy(source, &destination)?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(signature.1))?;
        }
        if fingerprint(&destination)? != *signature {
            return Err(invalid(
                "A local file changed while preparing the build; retry",
            ));
        }
    }
    Ok(())
}

fn ancestor(root: &Path, first: &str, second: &str) -> Result<bool> {
    let output = process::run(
        &git(root, &["merge-base", "--is-ancestor", first, second]),
        None,
        SHORT,
        true,
    )?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(invalid("Cannot compare Git revisions")),
    }
}
struct Staging {
    root: PathBuf,
    path: PathBuf,
}
impl Drop for Staging {
    fn drop(&mut self) {
        if let Err(error) = process::checked(
            &git(
                &self.root,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    &self.path.to_string_lossy(),
                ],
            ),
            None,
            SHORT,
            true,
        ) {
            eprintln!(
                "Temporary build checkout retained at {}: {error}",
                self.path.display()
            );
        }
    }
}
fn step(log: &File, label: &str, command: &[String], timeout: Duration) -> Result<()> {
    writeln!(&*log, "\n{label}")?;
    process::checked_logged(command, timeout, log)
        .map_err(|error| invalid(format!("{label}: {error}")))
}
fn copy_binary(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(invalid("Build did not produce an executable backend"));
    }
    let parent = target.parent().unwrap();
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".df-update-{}-{}",
        std::process::id(),
        target.file_name().unwrap().to_string_lossy()
    ));
    // Only remove a temporary file that this invocation created.
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        std::io::copy(&mut File::open(source)?, &mut output)?;
        output.set_permissions(fs::Permissions::from_mode(0o755))?;
        output.sync_all()?;
        fs::rename(&temporary, target)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn active(unit: &str) -> bool {
    process::run(
        &[
            "systemctl".into(),
            "--user".into(),
            "is-active".into(),
            "--quiet".into(),
            unit.into(),
        ],
        None,
        SHORT,
        true,
    )
    .is_ok_and(|output| output.status.success())
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let dry = args == ["--dry-run"];
    if !args.is_empty() && !dry {
        return Err(invalid("Usage: df-update [--dry-run]"));
    }
    // SAFETY: geteuid has no pointer arguments or side effects.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid(
            "Run df-update as your normal desktop user, without sudo",
        ));
    }
    let root = root.canonicalize()?;
    let home = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation");
    let selected = installation(&root, &home)?;
    let before = checkout_revision(&root)?;
    if dry {
        println!(
            "Fetch, rebuild and redeploy {} profile {}; preserve settings.",
            selected.compositor, selected.profile
        );
        return Ok(());
    }
    let _lock = state::lock(&home.join("update.lock"), Duration::from_secs(1))?;
    let log_path = home.join("update.log");
    state::atomic(&log_path, b"", 0o600)?;
    let log = fs::OpenOptions::new()
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&log_path)?;
    println!("Fetching Desktop Foundation…");
    let mut phase = "fetch";
    let result = (|| {
        step(
            &log,
            "Fetching Desktop Foundation…",
            &git(
                &root,
                &[
                    "fetch",
                    "--no-tags",
                    "origin",
                    "+refs/heads/main:refs/remotes/origin/main",
                ],
            ),
            Duration::from_secs(180),
        )?;
        let upstream = text(&git(&root, &["rev-parse", "refs/remotes/origin/main"]))?;
        let diverged =
            !ancestor(&root, &before, &upstream)? && !ancestor(&root, &upstream, &before)?;
        let mut candidate = if ancestor(&root, &before, &upstream)? {
            upstream.clone()
        } else {
            before.clone()
        };
        let original_diff = diff(&root, false)?;
        let original_index = diff(&root, true)?;
        let index_before = text(&git(&root, &["write-tree"]))?;
        let local_files = untracked(&root)?;
        // stash create records tracked edits without changing the live worktree
        // or the user's stash stack. A private ref keeps this backup reachable.
        let local_commit = text(&git(
            &root,
            &["stash", "create", "Desktop Foundation local update backup"],
        ))?;
        let backup = if local_commit.is_empty() {
            None
        } else {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| invalid("Invalid system clock"))?
                .as_nanos();
            let reference = format!("refs/desktop-foundation/local-update-{stamp}");
            text(&git(&root, &["update-ref", &reference, &local_commit]))?;
            writeln!(&log, "Preserving local settings in {reference}")?;
            Some(reference)
        };
        phase = "build/validation";
        println!("Building and checking…");
        let cache = state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/update");
        state::private_directory(&cache)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("Invalid system clock"))?
            .as_nanos();
        let path = cache.join(format!("source-{}-{nonce}", std::process::id()));
        step(
            &log,
            "Preparing isolated build checkout…",
            &git(
                &root,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    &path.to_string_lossy(),
                    &candidate,
                ],
            ),
            SHORT,
        )?;
        let staging = Staging {
            root: root.clone(),
            path,
        };
        if diverged {
            step(
                &log,
                "Merging upstream in the isolated checkout…",
                &git(&staging.path, &["merge", "--no-edit", "--no-ff", &upstream]),
                SHORT,
            )?;
            candidate = text(&git(&staging.path, &["rev-parse", "HEAD"]))?;
        }
        if !local_commit.is_empty() {
            step(
                &log,
                "Checking local settings against the update…",
                &git(&staging.path, &["stash", "apply", "--index", &local_commit]),
                SHORT,
            )?;
        }
        overlay_files(&root, &staging.path, &local_files)?;
        let published_index = text(&git(&staging.path, &["write-tree"]))?;
        let published_diff = diff(&staging.path, false)?;
        for (crate_name, script) in [
            ("foundation", "build-backend"),
            ("nothing", "build-nothing"),
        ] {
            let target = cache.join(format!("target-{crate_name}"));
            state::private_directory(&target)?;
            symlink(
                target,
                staging.path.join(format!("native/{crate_name}/target")),
            )?;
            step(
                &log,
                &format!("Building {crate_name} backend…"),
                &[
                    "env".into(),
                    "--unset=CARGO_TARGET_DIR".into(),
                    staging
                        .path
                        .join(format!("scripts/{script}"))
                        .to_string_lossy()
                        .into_owned(),
                ],
                BUILD,
            )?;
        }
        // Verify every output before touching the live checkout or binaries.
        for binary in BINARIES {
            let meta = fs::symlink_metadata(staging.path.join(binary))?;
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
                return Err(invalid("A required backend was not built"));
            }
        }
        step(
            &log,
            "Validating updated configuration…",
            &[
                staging
                    .path
                    .join("scripts/preflight")
                    .to_string_lossy()
                    .into_owned(),
                "--update".into(),
                "--profile".into(),
                selected.profile.clone(),
                "--compositor".into(),
                selected.compositor.clone(),
            ],
            Duration::from_secs(120),
        )?;
        if !staging
            .path
            .join("profiles")
            .join(&selected.profile)
            .is_dir()
        {
            return Err(invalid(
                "The update removed the selected hardware profile; nothing deployed",
            ));
        }
        if diff(&staging.path, false)? != published_diff
            || text(&git(&staging.path, &["write-tree"]))? != published_index
            || local_files.iter().any(|(path, signature)| {
                fingerprint(&staging.path.join(path)).as_ref().ok() != Some(signature)
            })
        {
            return Err(invalid(
                "The build changed project source or local settings; nothing deployed",
            ));
        }
        if checkout_revision(&root)? != before
            || diff(&root, false)? != original_diff
            || diff(&root, true)? != original_index
            || untracked(&root)? != local_files
        {
            return Err(invalid(
                "Checkout changed during the build; nothing deployed",
            ));
        }
        installation(&root, &home)?;
        phase = "publication";
        println!("Applying update…");
        let previous = if home.join("update.json").exists() {
            let saved = read(&home.join("update.json"))?;
            if saved["revision"].as_str() == Some(&candidate) {
                saved["previousRevision"]
                    .as_str()
                    .unwrap_or(&before)
                    .to_owned()
            } else {
                before.clone()
            }
        } else {
            before.clone()
        };
        let journal = json!({"previousRevision":previous, "revision":candidate, "compositor":selected.compositor, "profile":selected.profile,"localChangesRef":backup});
        state::atomic(
            &home.join("update.json"),
            &serde_json::to_vec_pretty(&journal)?,
            0o600,
        )?;
        let publication = step(
            &log,
            "Publishing Desktop Foundation with local settings…",
            &git(
                &root,
                &["merge", "--ff-only", "--autostash", "--no-edit", &candidate],
            ),
            SHORT,
        );
        if publication.is_err() {
            // A rejected merge normally reapplies its autostash. Restore staging
            // only when both the old HEAD and all worktree bytes are unchanged.
            if checkout_revision(&root)? == before
                && diff(&root, false)? == original_diff
                && text(&git(&root, &["ls-files", "--unmerged"]))?.is_empty()
            {
                text(&git(&root, &["read-tree", &index_before]))?;
            }
            publication?;
        }
        if !text(&git(&root, &["ls-files", "--unmerged"]))?.is_empty() {
            return Err(invalid(
                "Local changes conflicted during publication; recover from localChangesRef in update.json",
            ));
        }
        // Restore staged additions too, before comparing the complete worktree.
        text(&git(&root, &["read-tree", &published_index]))?;
        if diff(&root, false)? != published_diff {
            return Err(invalid(
                "Local changes could not be restored exactly; no binaries deployed. Recover from localChangesRef in update.json",
            ));
        }
        for binary in BINARIES {
            copy_binary(&staging.path.join(binary), &root.join(binary))?;
        }
        phase = "deployment";
        step(
            &log,
            "Redeploying Desktop Foundation…",
            &[
                root.join("scripts/deploy").to_string_lossy().into_owned(),
                "--update".into(),
                "--compositor".into(),
                selected.compositor.clone(),
                "--profile".into(),
                selected.profile.clone(),
            ],
            Duration::from_secs(300),
        )?;
        phase = "component reload";
        let variable = if selected.compositor == "niri" {
            "NIRI_SOCKET"
        } else {
            "HYPRLAND_INSTANCE_SIGNATURE"
        };
        if env::var_os(variable).is_some_and(|value| !value.is_empty()) {
            let command: Vec<String> = if selected.compositor == "niri" {
                vec![
                    "niri".into(),
                    "msg".into(),
                    "action".into(),
                    "load-config-file".into(),
                ]
            } else {
                vec!["hyprctl".into(), "reload".into()]
            };
            step(&log, "Reloading compositor configuration…", &command, SHORT)?;
        }
        if active("desktop-foundation-shell.service") {
            step(
                &log,
                "Reloading the shell…",
                &[root
                    .join("scripts/shell-reload")
                    .to_string_lossy()
                    .into_owned()],
                Duration::from_secs(15),
            )?;
        }
        if active("desktop-foundation-monitor.service") {
            step(
                &log,
                "Refreshing the sidebar…",
                &[
                    root.join("scripts/monitor").to_string_lossy().into_owned(),
                    "refresh".into(),
                ],
                SHORT,
            )?;
        }
        fs::remove_file(home.join("update.json"))?;
        println!("Updated to {}.", &candidate[..12]);
        Ok(())
    })();
    result.map_err(|error| {
        invalid(format!(
            "Update stopped during {phase}: {error}.\nDetails: {}",
            log_path.display()
        ))
    })
}
