//! Journaled compositor package ownership. Never stops/restarts the compositor.
use crate::{Result, invalid, process, state, theme_runtime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Read,
    os::fd::AsRawFd,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
const LIMIT: u64 = 256 * 1024 * 1024;
const POLICY: &str = "desktop-foundation/niri-package-input.toml";
#[derive(Clone)]
pub struct Paths {
    pub directory: PathBuf,
    pub config: PathBuf,
    pub binary: PathBuf,
}
impl Paths {
    pub fn environment() -> Result<Self> {
        Ok(Self {
            directory: state::xdg("XDG_STATE_HOME", ".local/state")?
                .join("desktop-foundation/niri-package"),
            config: state::xdg("XDG_CONFIG_HOME", ".config")?,
            binary: "/usr/bin/niri".into(),
        })
    }
    fn journal(&self) -> PathBuf {
        self.directory.join("journal.json")
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub name: String,
    pub version: String,
    pub archive: PathBuf,
    pub sha256: String,
    pub binary_sha256: String,
    pub signature: Option<PathBuf>,
    pub signature_sha256: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Prepared,
    Installed,
    Restoring,
    Restored,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub version: u8,
    pub phase: Phase,
    pub recipe: String,
    pub original: Artifact,
    pub candidate: Artifact,
    pub previous: Option<Artifact>,
    pub previous_recipe: Option<String>,
    pub capability: Option<bool>,
    pub pending_capability: Option<bool>,
    #[serde(default)]
    pub activation_pending: bool,
}
pub trait Effects {
    fn capture(&mut self, args: &[String]) -> Result<Vec<u8>>;
    fn interactive(&mut self, args: &[String]) -> Result<()>;
    fn build(&mut self, root: &Path) -> Result<PathBuf>;
    fn render(&mut self, root: &Path, profile: &str) -> Result<()>;
}
pub struct Native {
    pub gui_auth: bool,
}
impl Effects for Native {
    fn capture(&mut self, args: &[String]) -> Result<Vec<u8>> {
        let output = if args.first().is_some_and(|name| name == "bsdtar") {
            process::checked_binary_capture(args, Duration::from_secs(60))?
        } else {
            process::checked_capture(args, Duration::from_secs(60), 64 * 1024)?
        };
        Ok(output.stdout)
    }
    fn interactive(&mut self, args: &[String]) -> Result<()> {
        let mut command = args.to_vec();
        if self.gui_auth && command.first().is_some_and(|s| s == "sudo") {
            command[0] = "pkexec".into();
            command[1] = "/usr/bin/pacman".into();
        }
        let status = Command::new(&command[0]).args(&command[1..]).status()?;
        if !status.success() {
            return Err(invalid(format!("Package operation failed: {status}")));
        }
        Ok(())
    }
    fn render(&mut self, root: &Path, profile: &str) -> Result<()> {
        theme_runtime::ensure(root, profile)
    }
    fn build(&mut self, root: &Path) -> Result<PathBuf> {
        let mut missing = Vec::new();
        for package in crate::install::manifest(root, "niri-build")? {
            if !process::run(
                &args(&["pacman", "-T", &package]),
                None,
                Duration::from_secs(10),
                true,
            )?
            .status
            .success()
            {
                missing.push(package);
            }
        }
        if !missing.is_empty() {
            let mut command = args(&["sudo", "pacman", "-Syu", "--needed"]);
            command.extend(missing);
            self.interactive(&command)?;
        }
        println!("Building the pinned Niri package (the current session stays running)…");
        let output = Command::new(root.join("scripts/build-niri-package"))
            .arg("--print-path")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()?;
        if !output.status.success() {
            return Err(invalid("Niri package build failed; nothing installed"));
        }
        let path =
            String::from_utf8(output.stdout).map_err(|_| invalid("Invalid Niri package path"))?;
        if path.lines().count() != 1 {
            return Err(invalid("Niri builder returned multiple paths"));
        }
        Ok(PathBuf::from(path.trim()))
    }
}
fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).into()).collect()
}
fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| invalid("Niri package path is not UTF-8"))
}
fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > limit {
        return Err(invalid(format!(
            "Invalid/oversized Niri recovery file: {}",
            path.display()
        )));
    }
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(invalid("Niri recovery file grew while reading"));
    }
    Ok(data)
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn hash(path: &Path) -> Result<String> {
    Ok(digest(&bytes(path, LIMIT)?))
}
fn recipe(root: &Path) -> Result<String> {
    let mut data = bytes(&root.join("patches/niri/PKGBUILD"), 65536)?;
    data.extend(bytes(
        &root.join("patches/niri/overview-ctrl-scroll.patch"),
        1024 * 1024,
    )?);
    Ok(digest(&data))
}
fn identity(e: &mut impl Effects, path: &Path) -> Result<(String, String)> {
    let out = String::from_utf8(e.capture(&args(&["pacman", "-Qp", text(path)?]))?)
        .map_err(|_| invalid("Invalid package identity"))?;
    let parts: Vec<_> = out.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(invalid("Ambiguous package identity"));
    }
    Ok((parts[0].into(), parts[1].into()))
}
fn expected(root: &Path) -> Result<String> {
    let source = String::from_utf8(bytes(&root.join("patches/niri/PKGBUILD"), 65536)?)
        .map_err(|_| invalid("Invalid Niri recipe"))?;
    let field = |key: &str| -> Result<&str> {
        let value = source
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .ok_or_else(|| invalid("Missing literal Niri package version"))?;
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
        {
            return Err(invalid("Niri recipe version must be literal"));
        }
        Ok(value)
    };
    let version = format!("{}-{}", field("pkgver=")?, field("pkgrel=")?);
    let epoch = source.lines().find_map(|line| line.strip_prefix("epoch="));
    if let Some(epoch) = epoch {
        if epoch.is_empty() || !epoch.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("Invalid Niri recipe epoch"));
        }
        if epoch != "0" {
            return Ok(format!("{epoch}:{version}"));
        }
    }
    Ok(version)
}
fn save(paths: &Paths, journal: &Journal) -> Result<()> {
    state::atomic(
        &paths.journal(),
        &serde_json::to_vec_pretty(journal)?,
        0o600,
    )
}
pub fn read(paths: &Paths) -> Result<Option<Journal>> {
    if !paths.journal().try_exists()? {
        return Ok(None);
    }
    let j: Journal = serde_json::from_slice(&bytes(&paths.journal(), 128 * 1024)?)?;
    if j.version != 1 {
        return Err(invalid("Unknown Niri package journal version"));
    }
    let valid_hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    let valid_artifact = |a: &Artifact| {
        ["niri", "niri-foundation"].contains(&a.name.as_str())
            && !a.version.is_empty()
            && a.version.len() <= 128
            && a.version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+:-".contains(&b))
            && valid_hash(&a.sha256)
            && valid_hash(&a.binary_sha256)
            && a.signature.is_some() == a.signature_sha256.is_some()
            && a.signature_sha256.as_deref().is_none_or(valid_hash)
    };
    if !valid_hash(&j.recipe)
        || !valid_artifact(&j.original)
        || !valid_artifact(&j.candidate)
        || j.original.name != "niri"
        || j.candidate.name != "niri-foundation"
        || j.original.signature.is_none()
        || j.previous.is_some() != j.previous_recipe.is_some()
        || j.previous
            .as_ref()
            .is_some_and(|a| !valid_artifact(a) || a.name != "niri-foundation")
        || j.previous_recipe.as_deref().is_some_and(|s| !valid_hash(s))
    {
        return Err(invalid(
            "Invalid Niri package recovery metadata; journal preserved",
        ));
    }
    Ok(Some(j))
}
fn verified(paths: &Paths, a: &Artifact, e: &mut impl Effects) -> Result<()> {
    if a.archive.parent() != Some(paths.directory.as_path())
        || hash(&a.archive)? != a.sha256
        || identity(e, &a.archive)? != (a.name.clone(), a.version.clone())
    {
        return Err(invalid(
            "Niri package recovery archive changed; preserved without installing",
        ));
    }
    if let Some(signature) = &a.signature {
        if signature.parent() != Some(paths.directory.as_path())
            || Some(hash(signature)?) != a.signature_sha256
        {
            return Err(invalid("Stock Niri signature changed"));
        }
        e.capture(&args(&[
            "pacman-key",
            "--verify",
            text(signature)?,
            text(&a.archive)?,
        ]))?;
    } else if a.name == "niri" {
        return Err(invalid(
            "Stock Niri recovery requires its verified signature",
        ));
    }
    Ok(())
}
fn retain(paths: &Paths, source: &Path, signed: bool, e: &mut impl Effects) -> Result<Artifact> {
    let contents = bytes(source, LIMIT)?;
    let sha = digest(&contents);
    let (name, version) = identity(e, source)?;
    if !["niri", "niri-foundation"].contains(&name.as_str()) {
        return Err(invalid("Unexpected compositor package"));
    }
    let binary = e.capture(&args(&["bsdtar", "-xOf", text(source)?, "usr/bin/niri"]))?;
    if binary.is_empty() {
        return Err(invalid("Niri package contains no compositor"));
    }
    let archive = paths.directory.join(format!("{name}-{sha}.pkg.tar.zst"));
    let (signature, signature_sha256) = if signed {
        let sig = PathBuf::from(format!("{}.sig", text(source)?));
        e.capture(&args(&[
            "pacman-key",
            "--verify",
            text(&sig)?,
            text(source)?,
        ]))?;
        let data = bytes(&sig, 64 * 1024)?;
        let target = PathBuf::from(format!("{}.sig", text(&archive)?));
        state::atomic(&target, &data, 0o600)?;
        (Some(target), Some(digest(&data)))
    } else {
        (None, None)
    };
    state::atomic(&archive, &contents, 0o600)?;
    Ok(Artifact {
        name,
        version,
        archive,
        sha256: sha,
        binary_sha256: digest(&binary),
        signature,
        signature_sha256,
    })
}
fn stock(paths: &Paths, e: &mut impl Effects) -> Result<Artifact> {
    let installed = String::from_utf8(e.capture(&args(&["pacman", "-Q", "niri"]))?)
        .map_err(|_| invalid("Invalid stock Niri version"))?;
    let installed = installed.trim();
    if !installed.starts_with("niri ") {
        return Err(invalid("Cannot identify stock Niri for recovery"));
    }
    let find = |e: &mut dyn Effects| -> Result<Option<PathBuf>> {
        let caches = String::from_utf8(e.capture(&args(&["pacman-conf", "CacheDir"]))?)
            .map_err(|_| invalid("Invalid pacman cache paths"))?;
        for dir in caches.lines() {
            let entries = match fs::read_dir(dir) {
                Ok(v) => v,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            for entry in entries {
                let path = entry?.path();
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with("niri-")
                    && name.contains(".pkg.tar.")
                    && !name.ends_with(".sig")
                    && path.with_file_name(format!("{name}.sig")).is_file()
                {
                    let out = e.capture(&args(&["pacman", "-Qp", text(&path)?]))?;
                    if String::from_utf8_lossy(&out).trim() == installed {
                        return Ok(Some(path));
                    }
                }
            }
        }
        Ok(None)
    };
    let source = if let Some(path) = find(e)? {
        path
    } else {
        let version = installed.strip_prefix("niri ").expect("validated");
        e.interactive(&args(&[
            "sudo",
            "pacman",
            "-Sw",
            "--noconfirm",
            &format!("niri={version}"),
        ]))?;
        find(e)?.ok_or_else(|| {
            invalid("The exact signed stock Niri archive is unavailable; no compositor replaced")
        })?
    };
    retain(paths, &source, true, e)
}
fn installed(paths: &Paths, a: &Artifact, e: &mut impl Effects) -> bool {
    hash(&paths.binary).is_ok_and(|sha| sha == a.binary_sha256)
        && e.capture(&args(&["pacman", "-Q", &a.name]))
            .is_ok_and(|out| {
                String::from_utf8_lossy(&out).trim() == format!("{} {}", a.name, a.version)
            })
}
fn validate_candidate(paths: &Paths, a: &Artifact, e: &mut impl Effects) -> Result<()> {
    let binary = e.capture(&args(&[
        "bsdtar",
        "-xOf",
        text(&a.archive)?,
        "usr/bin/niri",
    ]))?;
    if digest(&binary) != a.binary_sha256 {
        return Err(invalid("Niri package payload changed"));
    }
    let file = paths.directory.join("candidate-niri");
    state::atomic(&file, &binary, 0o700)?;
    let probe = paths.directory.join("capability.kdl");
    state::atomic(
        &probe,
        b"overview { ctrl-scroll-workspaces true; }\n",
        0o600,
    )?;
    e.capture(&args(&[text(&file)?, "validate", "-c", text(&probe)?]))?;
    let current = paths.config.join("niri/config.kdl");
    if current.try_exists()? {
        e.capture(&args(&[text(&file)?, "validate", "-c", text(&current)?]))?;
    }
    Ok(())
}
fn policy_bytes(value: bool) -> Vec<u8> {
    format!("# Managed by Desktop Foundation's Niri package transaction.\noverview_ctrl_scroll = {value}\n").into_bytes()
}
fn policy_owned(paths: &Paths, j: &Journal) -> Result<()> {
    let path = paths.config.join(POLICY);
    if path.try_exists()? {
        let data = bytes(&path, 4096)?;
        if ![j.capability, j.pending_capability]
            .into_iter()
            .flatten()
            .any(|value| data == policy_bytes(value))
        {
            return Err(invalid(
                "Managed Niri input policy changed externally; configuration preserved",
            ));
        }
    } else if j.capability.is_some() {
        return Err(invalid("Managed Niri input policy was removed externally"));
    }
    Ok(())
}
fn policy(paths: &Paths, j: &mut Journal, value: bool) -> Result<()> {
    policy_owned(paths, j)?;
    j.pending_capability = Some(value);
    save(paths, j)?;
    state::private_directory(&paths.config.join("desktop-foundation"))?;
    state::atomic(&paths.config.join(POLICY), &policy_bytes(value), 0o600)?;
    j.capability = Some(value);
    j.pending_capability = None;
    save(paths, j)
}
pub fn install(
    root: &Path,
    paths: &Paths,
    provided: Option<&Path>,
    e: &mut impl Effects,
) -> Result<()> {
    state::private_directory(&paths.directory)?;
    let _lock = state::lock(
        &paths.directory.join("operation.lock"),
        Duration::from_secs(5),
    )?;
    let wanted = recipe(root)?;
    let mut old = read(paths)?;
    if let Some(j) = &mut old {
        verified(paths, &j.original, e)?;
        policy_owned(paths, j)?;
        if j.phase == Phase::Restoring {
            return Err(invalid(
                "Finish niri-package restore before installing another package",
            ));
        }
        if j.phase == Phase::Prepared && installed(paths, &j.candidate, e) {
            verified(paths, &j.candidate, e)?;
            j.phase = Phase::Installed;
            save(paths, j)?;
        }
        if j.phase == Phase::Installed {
            verified(paths, &j.candidate, e)?;
            if !installed(paths, &j.candidate, e) {
                return Err(invalid(
                    "Managed Niri package changed externally; use niri-package restore before reinstalling",
                ));
            }
            if j.recipe == wanted && provided.is_none() {
                println!("Patched Niri is current; the active session was not restarted.");
                return Ok(());
            }
        } else {
            let previous = if j.phase == Phase::Restored {
                &j.original
            } else {
                j.previous.as_ref().unwrap_or(&j.original)
            };
            verified(paths, previous, e)?;
            if !installed(paths, previous, e) {
                return Err(invalid(
                    "Interrupted Niri transaction no longer matches its recorded package; restore first",
                ));
            }
        }
    } else if paths.config.join(POLICY).try_exists()? {
        return Err(invalid(
            "Niri package input policy already exists without an ownership journal",
        ));
    }
    let original = if let Some(j) = &old {
        j.original.clone()
    } else {
        stock(paths, e)?
    };
    let candidate = if let Some(path) = provided {
        retain(paths, path, false, e)?
    } else if let Some(cached) = old.as_ref().filter(|j| j.recipe == wanted) {
        verified(paths, &cached.candidate, e)?;
        cached.candidate.clone()
    } else {
        let built = e.build(root)?;
        retain(paths, &built, false, e)?
    };
    if candidate.name != "niri-foundation" || candidate.version != expected(root)? {
        return Err(invalid("Built Niri package does not match this recipe"));
    }
    validate_candidate(paths, &candidate, e)?;
    let mut j = Journal {
        version: 1,
        phase: Phase::Prepared,
        recipe: wanted,
        original,
        candidate,
        previous: old.as_ref().and_then(|j| {
            if j.phase == Phase::Installed {
                Some(j.candidate.clone())
            } else if j.phase == Phase::Prepared {
                j.previous.clone()
            } else {
                None
            }
        }),
        previous_recipe: old.as_ref().and_then(|j| {
            if j.phase == Phase::Installed {
                Some(j.recipe.clone())
            } else if j.phase == Phase::Prepared {
                j.previous_recipe.clone()
            } else {
                None
            }
        }),
        capability: old.as_ref().and_then(|j| j.capability),
        pending_capability: old.as_ref().and_then(|j| j.pending_capability),
        activation_pending: old.as_ref().is_some_and(|j| j.activation_pending),
    };
    save(paths, &j)?;
    let result = e
        .interactive(&args(&[
            "sudo",
            "pacman",
            "-U",
            text(&j.candidate.archive)?,
        ]))
        .and_then(|()| {
            if installed(paths, &j.candidate, e) {
                Ok(())
            } else {
                Err(invalid(
                    "Installed Niri payload did not match the verified package",
                ))
            }
        });
    if let Err(error) = result {
        let recovery = j.previous.as_ref().unwrap_or(&j.original).clone();
        verified(paths, &recovery, e)?;
        if !installed(paths, &recovery, e) {
            e.interactive(&args(&["sudo","pacman","-U",text(&recovery.archive)?])).map_err(|rollback|invalid(format!("{error}; package recovery also failed: {rollback}. Recovery archives and journal retained")))?;
            if !installed(paths, &recovery, e) {
                return Err(invalid(format!(
                    "{error}; recovery payload verification failed; journal retained"
                )));
            }
        }
        if let Some(previous) = j.previous.take() {
            j.candidate = previous;
            j.recipe = j.previous_recipe.take().expect("previous recipe");
            j.phase = Phase::Installed;
        } else {
            j.phase = Phase::Restored;
        }
        save(paths, &j)?;
        return Err(error);
    }
    j.phase = Phase::Installed;
    save(paths, &j)?;
    println!(
        "Patched Niri installed; the running graphical session is preserved. Log out and back in to activate Ctrl+Scroll overview zoom."
    );
    Ok(())
}
fn profile(paths: &Paths) -> Result<String> {
    let path = paths
        .directory
        .parent()
        .expect("component parent")
        .join("theme/profile.json");
    let value: serde_json::Value = serde_json::from_slice(&bytes(&path, 65536)?)?;
    value["profile"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| invalid("No installed Niri hardware profile"))
}
fn owner(root: &Path, paths: &Paths) -> Result<()> {
    let path = paths
        .directory
        .parent()
        .expect("component parent")
        .join("manifest.json");
    let value: serde_json::Value = serde_json::from_slice(&bytes(&path, 1024 * 1024)?)?;
    if value["root"].as_str() != root.to_str() {
        return Err(invalid(
            "Only the deployment owner can activate Niri input settings",
        ));
    }
    Ok(())
}
fn running_hash() -> Result<Option<String>> {
    let Some(socket) = env::var_os("NIRI_SOCKET") else {
        return Ok(None);
    };
    let stream = UnixStream::connect(socket)?;
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: exact initialized output buffers and the owned connected descriptor.
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: geteuid has no arguments.
    if cred.pid <= 0 || cred.uid != unsafe { libc::geteuid() } {
        return Err(invalid("Niri socket is owned by another user"));
    }
    // /proc/PID/exe deliberately follows the kernel's executable link, including
    // an old inode retained by a running compositor after package replacement.
    let mut data = Vec::new();
    fs::File::open(format!("/proc/{}/exe", cred.pid))?
        .take(LIMIT + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > LIMIT {
        return Err(invalid("Niri executable exceeds size limit"));
    }
    Ok(Some(digest(&data)))
}
pub fn activate_with(
    root: &Path,
    paths: &Paths,
    running: Option<&str>,
    e: &mut impl Effects,
) -> Result<()> {
    if !paths.journal().try_exists()? {
        return Ok(());
    }
    let _lock = state::lock(
        &paths.directory.join("operation.lock"),
        Duration::from_secs(5),
    )?;
    let Some(mut j) = read(paths)? else {
        return Ok(());
    };
    if j.phase != Phase::Installed || running != Some(&j.candidate.binary_sha256) {
        return Ok(());
    }
    owner(root, paths)?;
    if j.capability == Some(true) && !j.activation_pending {
        policy_owned(paths, &j)?;
        return Ok(());
    }
    j.activation_pending = true;
    save(paths, &j)?;
    policy(paths, &mut j, true)?;
    e.render(root, &profile(paths)?)?;
    e.capture(&args(&["niri", "msg", "action", "load-config-file"]))?;
    j.activation_pending = false;
    save(paths, &j)?;
    println!("Patched Niri session verified; overview Ctrl+Scroll integration activated.");
    Ok(())
}
pub fn activate(root: &Path) -> Result<()> {
    let paths = Paths::environment()?;
    if !paths.journal().try_exists()? {
        return Ok(());
    }
    let running = running_hash()?;
    activate_with(
        root,
        &paths,
        running.as_deref(),
        &mut Native { gui_auth: false },
    )
}
pub fn restore(root: &Path, paths: &Paths, e: &mut impl Effects) -> Result<()> {
    if !paths.journal().try_exists()? {
        println!("No managed Niri package to restore.");
        return Ok(());
    }
    let _lock = state::lock(
        &paths.directory.join("operation.lock"),
        Duration::from_secs(5),
    )?;
    let Some(mut j) = read(paths)? else {
        return Ok(());
    };
    verified(paths, &j.original, e)?;
    policy_owned(paths, &j)?;
    if j.phase == Phase::Restored && installed(paths, &j.original, e) {
        println!("Stock Niri is already restored.");
        return Ok(());
    }
    // Disable only the owned capability layer; user input files remain intact.
    j.phase = Phase::Restoring;
    j.activation_pending = false;
    save(paths, &j)?;
    policy(paths, &mut j, false)?;
    let manifest = paths
        .directory
        .parent()
        .expect("component parent")
        .join("manifest.json");
    if manifest.exists() {
        owner(root, paths)?;
        e.render(root, &profile(paths)?)?;
    }
    e.interactive(&args(&["sudo", "pacman", "-U", text(&j.original.archive)?]))?;
    if !installed(paths, &j.original, e) {
        return Err(invalid(
            "Stock Niri restore did not verify; journal retained",
        ));
    }
    j.phase = Phase::Restored;
    save(paths, &j)?;
    println!(
        "Verified stock Niri restored. The current compositor remains running; the next login uses stock Niri."
    );
    Ok(())
}
pub fn ensure_managed(root: &Path) -> Result<()> {
    let paths = Paths::environment()?;
    if read(&paths)?.is_some_and(|j| j.phase == Phase::Installed) {
        install(root, &paths, None, &mut Native { gui_auth: false })?;
    }
    Ok(())
}
pub fn run(root: &Path, arguments: &[String]) -> Result<()> {
    // SAFETY: geteuid takes no arguments.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Run Niri package management as your desktop user"));
    }
    let mut args = arguments.to_vec();
    let gui = args.iter().any(|a| a == "--gui-auth");
    args.retain(|a| a != "--gui-auth");
    let paths = Paths::environment()?;
    let mut native = Native { gui_auth: gui };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["status"] => {
            println!("{}", serde_json::to_string(&read(&paths)?)?);
            Ok(())
        }
        ["install"] => install(root, &paths, None, &mut native),
        ["install", "--package", file] => install(root, &paths, Some(Path::new(file)), &mut native),
        ["restore"] => restore(root, &paths, &mut native),
        ["activate"] => activate(root),
        _ => Err(invalid(
            "Expected niri-package status|install [--package FILE] [--gui-auth]|restore [--gui-auth]|activate",
        )),
    }
}
