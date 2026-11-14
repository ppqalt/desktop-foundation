//! Native installer orchestration. Privileged/client helpers keep their own journals.
use crate::{Result, app_mods, application_roles, apps, deploy, invalid, process, state};
use serde_json::Value;
use std::io::Read;
use std::{fs, path::Path, process::Command, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub personal: bool,
    pub standard: bool,
    pub extras: Vec<String>,
    pub greeter: bool,
    pub auto_greeter: bool,
    pub patched_niri: bool,
    pub packages: bool,
    pub clean_boot: bool,
    pub eac_boot: bool,
    pub recovery_only_dkms: bool,
    pub recovery_keys_confirmed: bool,
    pub profile: String,
    pub performance_profile: Option<String>,
    pub dry_run: bool,
    pub check: bool,
}
impl Options {
    pub fn parse(root: &Path, args: &[String]) -> Result<Self> {
        let mut o = Self {
            personal: false,
            standard: false,
            extras: Vec::new(),
            greeter: true,
            auto_greeter: true,
            patched_niri: true,
            packages: true,
            clean_boot: false,
            eac_boot: false,
            recovery_only_dkms: false,
            recovery_keys_confirmed: false,
            profile: "default".into(),
            performance_profile: None,
            dry_run: false,
            check: false,
        };
        let mut layer = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                arg if arg.starts_with("--profile=") || arg.starts_with("--hardware-profile=") => {
                    o.profile = arg.split_once('=').expect("matched option").1.into();
                }
                "--core" | "--minimal" | "--personal" | "--everything" | "--standard" => {
                    let personal = matches!(args[i].as_str(), "--personal" | "--everything");
                    let selected = if personal {
                        "personal"
                    } else if args[i] == "--standard" {
                        "standard"
                    } else {
                        "minimal"
                    };
                    if layer.is_some_and(|old| old != selected) {
                        return Err(invalid("Choose one installation profile"));
                    }
                    layer = Some(selected);
                    o.personal = personal;
                    o.standard = selected == "standard";
                }
                "--extra" => {
                    i += 1;
                    let extra = args.get(i).ok_or_else(|| invalid("Missing extra name"))?;
                    if !["steam", "spicetify", "geode", "recording", "brave-nightly"]
                        .contains(&extra.as_str())
                        || o.extras.contains(extra)
                    {
                        return Err(invalid(
                            "Unknown or duplicate extra; choose steam, spicetify, geode, recording or brave-nightly",
                        ));
                    }
                    o.extras.push(extra.clone());
                }
                "--patched-niri" => o.patched_niri = true,
                "--stock-niri" => o.patched_niri = false,
                "--greeter" => {
                    o.greeter = true;
                    o.auto_greeter = false;
                }
                "--no-greeter" => {
                    o.greeter = false;
                    o.auto_greeter = false;
                }
                "--no-packages" => o.packages = false,
                "--clean-boot" => o.clean_boot = true,
                "--allow-recovery-only-dkms" => o.recovery_only_dkms = true,
                "--recovery-keys-confirmed" => o.recovery_keys_confirmed = true,
                "--performance-profile" => {
                    i += 1;
                    let value = args
                        .get(i)
                        .ok_or_else(|| invalid("Missing performance profile"))?;
                    if !["lucky38", "off"].contains(&value.as_str()) {
                        return Err(invalid("Performance profile must be lucky38 or off"));
                    }
                    o.performance_profile = Some(value.clone());
                }
                "--boot-profile" => {
                    i += 1;
                    o.eac_boot = match args.get(i).map(String::as_str) {
                        Some("eac") => true,
                        Some("existing") => false,
                        _ => return Err(invalid("Boot profile must be eac or existing")),
                    };
                }
                "--dry-run" | "--plan" => o.dry_run = true,
                "--check" => o.check = true,
                "--profile" | "--hardware-profile" => {
                    i += 1;
                    o.profile = args
                        .get(i)
                        .ok_or_else(|| invalid("Missing profile name"))?
                        .clone();
                }
                _ => return Err(invalid(format!("Unknown installer option: {}", args[i]))),
            }
            i += 1;
        }
        if o.eac_boot && o.clean_boot {
            return Err(invalid("EAC boot profile cannot use GRUB --clean-boot"));
        }
        if o.dry_run && o.check {
            return Err(invalid("Choose --dry-run or --check"));
        }
        if o.personal && !o.extras.is_empty() {
            return Err(invalid(
                "Legacy --personal already controls its application extras",
            ));
        }
        if o.profile.is_empty()
            || !o
                .profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !root.join("profiles").join(&o.profile).is_dir()
        {
            return Err(invalid("Unknown/invalid hardware profile"));
        }
        Ok(o)
    }
}

fn package(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
    {
        return Err(invalid("Invalid package name"));
    }
    Ok(())
}
pub fn manifest(root: &Path, group: &str) -> Result<Vec<String>> {
    if group.is_empty()
        || !group
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(invalid("Invalid package manifest group"));
    }
    let mut text = String::new();
    let directory = root.join("packages");
    let path = directory.join(format!("{group}.toml"));
    let (file, legacy) = match fs::File::open(&path) {
        Ok(file) => (file, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
            fs::File::open(directory.join(format!("{group}.txt")))?,
            true,
        ),
        Err(error) => return Err(error.into()),
    };
    file.take(64 * 1024 + 1).read_to_string(&mut text)?;
    if text.len() > 64 * 1024 {
        return Err(invalid("Package manifest exceeds 64 KiB"));
    }
    if legacy {
        return text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|name| {
                package(name)?;
                Ok(name.to_owned())
            })
            .collect();
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        packages: Vec<String>,
    }
    let parsed: Manifest = toml::from_str(&text)
        .map_err(|error| invalid(format!("Invalid package manifest {group}: {error}")))?;
    for name in &parsed.packages {
        package(name)?;
    }
    Ok(parsed.packages)
}

/// Substitute only system effects in isolated tests; production has no test env bypass.
pub trait Runtime {
    fn query(&mut self, args: &[String]) -> Result<bool>;
    fn command(&mut self, args: &[String]) -> Result<()>;
    /// true = setup ready; false = a safe, durable user-action checkpoint.
    fn checkpoint_command(&mut self, args: &[String]) -> Result<bool> {
        self.command(args)?;
        Ok(true)
    }
    fn executable(&self, name: &str) -> bool;
    fn validate_app(&self, name: &str, app: &Value) -> Result<()>;
    fn desktop_exists(&self, name: &str) -> Result<bool>;
    fn uid(&self) -> u32;
    fn conflicts(&self, root: &Path, paths: &deploy::Paths, personal: bool) -> Result<Vec<String>>;
    fn eac_kernel(&self) -> bool {
        false
    }
    fn lucky38_hardware(&self) -> bool {
        false
    }
    fn apparmor_enabled(&self) -> bool;
    fn clean_boot_supported(&self) -> bool;
    fn display_manager(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
pub struct Native;
impl Runtime for Native {
    fn lucky38_hardware(&self) -> bool {
        crate::performance::detect_in(Path::new("/"))
    }
    fn checkpoint_command(&mut self, args: &[String]) -> Result<bool> {
        let program = args.first().ok_or_else(|| invalid("Empty setup command"))?;
        let status = Command::new(program).args(&args[1..]).status()?;
        match status.code() {
            Some(0) => Ok(true),
            Some(75) => Ok(false),
            _ => Err(invalid(format!(
                "Setup failed ({status}): {program}; saved journals retained"
            ))),
        }
    }
    fn eac_kernel(&self) -> bool {
        fs::read_to_string("/proc/sys/kernel/osrelease")
            .is_ok_and(|s| s.trim().ends_with("-cachyos-eac"))
    }
    fn display_manager(&self) -> Result<Option<String>> {
        system_display_manager()
    }
    fn query(&mut self, args: &[String]) -> Result<bool> {
        Ok(process::run(args, None, Duration::from_secs(10), true)?
            .status
            .success())
    }
    fn command(&mut self, args: &[String]) -> Result<()> {
        println!("+ {}", args.join(" "));
        // Authentication, package recipe review and package downloads own their
        // terminal. Do not capture stdin or apply a selector-style timeout here.
        let program = args
            .first()
            .ok_or_else(|| invalid("Empty installer command"))?;
        let status = Command::new(program).args(&args[1..]).status()?;
        if !status.success() {
            return Err(invalid(format!("Command failed ({status}): {}", args[0])));
        }
        Ok(())
    }
    fn executable(&self, name: &str) -> bool {
        apps::executable(name)
    }
    fn validate_app(&self, name: &str, app: &Value) -> Result<()> {
        application_roles::validate(name, app).map(|_| ())
    }
    fn desktop_exists(&self, name: &str) -> Result<bool> {
        Ok(application_roles::desktop_path(name)?.is_some())
    }
    fn uid(&self) -> u32 {
        unsafe { libc::geteuid() }
    } // SAFETY: no arguments or pointers.
    fn conflicts(&self, root: &Path, paths: &deploy::Paths, personal: bool) -> Result<Vec<String>> {
        let mut errors = deploy::inspect(root, paths)?;
        if personal {
            errors.extend(application_roles::conflicts()?);
        }
        Ok(errors)
    }
    fn apparmor_enabled(&self) -> bool {
        fs::read_to_string("/sys/module/apparmor/parameters/enabled").is_ok_and(|s| s.trim() == "Y")
    }
    fn clean_boot_supported(&self) -> bool {
        !apps::executable("dracut") || Path::new("/etc/mkinitcpio.conf").is_file()
    }
}
fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).into()).collect()
}
fn script(root: &Path, name: &str, args: &[&str]) -> Vec<String> {
    let mut command = vec![
        root.join("scripts")
            .join(name)
            .to_string_lossy()
            .into_owned(),
    ];
    command.extend(words(args));
    command
}
fn call(r: &mut impl Runtime, root: &Path, name: &str, args: &[&str]) -> Result<()> {
    r.command(&script(root, name, args))
}
fn sudo_script(r: &mut impl Runtime, root: &Path, name: &str) -> Result<()> {
    let mut args = vec!["sudo".into()];
    args.extend(script(root, name, &["install"]));
    r.command(&args)
}
fn installed(r: &mut impl Runtime, name: &str) -> Result<bool> {
    Ok(r.executable("pacman") && r.query(&words(&["pacman", "-T", name]))?)
}
fn missing_packages(r: &mut impl Runtime, packages: &[String]) -> Result<()> {
    let mut missing = Vec::new();
    for name in packages {
        if !installed(r, name)? {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        let mut args = words(&["sudo", "pacman", "-Syu", "--needed"]);
        args.extend(missing);
        r.command(&args)?;
    }
    Ok(())
}
fn app_record(name: &str, app: &Value) -> Result<()> {
    package(
        app["package"]
            .as_str()
            .ok_or_else(|| invalid(format!("Missing package for {name}")))?,
    )?;
    let executable = app["executable"]
        .as_str()
        .ok_or_else(|| invalid("Missing application executable"))?;
    if executable.is_empty()
        || executable.starts_with('-')
        || executable.contains('/')
        || executable
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(invalid("Invalid application executable"));
    }
    let desktop = app["desktop"]
        .as_str()
        .ok_or_else(|| invalid("Missing application desktop entry"))?;
    if desktop.contains('/')
        || desktop.chars().any(char::is_control)
        || !desktop.ends_with(".desktop")
    {
        return Err(invalid("Invalid application desktop entry"));
    }
    Ok(())
}
pub fn provision(
    r: &mut impl Runtime,
    root: &Path,
    name: &str,
    app: &Value,
    aur: bool,
) -> Result<()> {
    app_record(name, app)?;
    let package = app["package"].as_str().expect("validated package");
    if installed(r, package)? {
        r.validate_app(name, app)?;
        println!("Adopted installed {package}.");
        return Ok(());
    }
    if r.executable(app["executable"].as_str().expect("validated executable"))
        || r.desktop_exists(app["desktop"].as_str().expect("validated desktop"))?
    {
        return Err(invalid(format!(
            "Unrelated existing application variant: {name}; resolve it explicitly"
        )));
    }
    let repository_package = if package == "brave-origin-bin" {
        "cachyos/brave-origin-bin"
    } else {
        package
    };
    if r.query(&words(&["pacman", "-Si", repository_package]))? {
        r.command(&words(&[
            "sudo",
            "pacman",
            "-Syu",
            "--needed",
            repository_package,
        ]))?;
    } else if aur {
        call(r, root, "paru-bootstrap", &[])?;
        r.command(&words(&["paru", "-S", "--needed", package]))?;
    } else {
        return Err(invalid(format!(
            "{package} is unavailable in configured signed repositories; no repository added"
        )));
    }
    r.validate_app(name, app)
}
/// Detect the configured manager without contacting/restarting a running session.
pub fn configured_display_manager(alias: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(alias) {
        Ok(meta) if meta.file_type().is_symlink() => Ok(Some(
            fs::read_link(alias)?
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| {
                    name.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"@._-".contains(&c))
                })
                .unwrap_or("display-manager.service")
                .to_string(),
        )),
        Ok(meta) if meta.is_file() => Ok(Some("display-manager.service".into())),
        Ok(_) => Err(invalid(
            "Display-manager configuration is not a service file or link",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub fn system_display_manager() -> Result<Option<String>> {
    detect_display_manager(
        Path::new("/etc/systemd/system/display-manager.service"),
        |check, manager| {
            Ok(process::run(
                &words(&["/usr/bin/systemctl", check, "--quiet", manager]),
                None,
                Duration::from_secs(5),
                true,
            )?
            .status
            .success())
        },
    )
}
fn detect_display_manager(
    alias: &Path,
    mut probe: impl FnMut(&str, &str) -> Result<bool>,
) -> Result<Option<String>> {
    if let Some(manager) = configured_display_manager(alias)? {
        return Ok(Some(manager));
    }
    // Prefer running managers over merely enabled ones, including alias-free setups.
    for check in ["is-active", "is-enabled"] {
        for manager in [
            "sddm.service",
            "gdm.service",
            "lightdm.service",
            "lxdm.service",
            "greetd.service",
        ] {
            if probe(check, manager)? {
                return Ok(Some(manager.into()));
            }
        }
    }
    Ok(None)
}

fn resolve_greeter(r: &impl Runtime, options: &Options) -> Result<Options> {
    let mut resolved = options.clone();
    if !options.auto_greeter && !options.greeter {
        return Ok(resolved);
    }
    let manager = r.display_manager()?;
    if options.auto_greeter {
        resolved.greeter = manager.is_none();
        resolved.auto_greeter = false;
        if let Some(manager) = manager {
            println!("Preserving existing login manager: {manager}");
        }
    } else if let Some(manager) = manager
        && manager != "greetd.service"
    {
        return Err(invalid(format!(
            "Existing login manager {manager}; use --no-greeter to preserve it. No login files changed."
        )));
    }
    Ok(resolved)
}
const RECORDING_TOOLS: [&str; 3] = ["gpu-screen-recorder", "ffmpeg", "ffprobe"];
fn check_recording_tools(r: &impl Runtime) -> Result<()> {
    let missing: Vec<_> = RECORDING_TOOLS
        .into_iter()
        .filter(|tool| !r.executable(tool))
        .collect();
    if !missing.is_empty() {
        return Err(invalid(format!(
            "Recording requires {} (recording packages: gpu-screen-recorder and ffmpeg)",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn doctor(r: &mut impl Runtime, root: &Path, o: &Options) -> Result<()> {
    let mut args = vec![
        "--installation",
        if o.personal {
            "--personal"
        } else if o.standard {
            "--standard"
        } else {
            "--core"
        },
    ];
    if o.eac_boot {
        args.push("--eac-profile");
    }
    if !o.greeter {
        args.push("--no-greeter");
    }
    call(r, root, "doctor", &args)
}

pub fn execute(
    root: &Path,
    paths: &deploy::Paths,
    o: &Options,
    r: &mut impl Runtime,
) -> Result<()> {
    let mut resolved = resolve_greeter(r, o)?;
    if resolved.performance_profile.is_none() {
        resolved.performance_profile = match crate::performance::selection(&paths.state)?.as_deref()
        {
            Some("off") => Some("off".into()),
            _ if r.lucky38_hardware() => Some("lucky38".into()),
            _ => None,
        };
    }
    let o = &resolved;
    let recording_selected = o.personal || o.standard || o.extras.iter().any(|e| e == "recording");
    if (o.check || !o.packages) && recording_selected {
        check_recording_tools(r)?;
    }
    if (o.check || !o.packages) && o.extras.iter().any(|extra| extra == "brave-nightly") {
        let nightly = application_roles::roles(root, true)?;
        r.validate_app("browser", &nightly["browser"])?;
    }
    if o.check {
        doctor(r, root, o)?;
        if o.standard {
            if !r.executable("paru") {
                return Err(invalid("Standard requires paru"));
            }
            let roles = application_roles::roles(root, false)?;
            let apps: Value = crate::theme_render::read(&root.join("apps/personal.json"))?;
            for (name, app) in [
                ("browser", &roles["browser"]),
                ("chatgpt", &apps["chatgpt"]),
                ("vesktop", &apps["vesktop"]),
            ] {
                r.validate_app(name, app)?;
            }
        }
        if o.extras.iter().any(|e| e == "steam" || e == "geode") {
            let apps: Value = crate::theme_render::read(&root.join("apps/personal.json"))?;
            r.validate_app("steam", &apps["steam"])?;
        }
        for extra in &o.extras {
            if extra == "spicetify" || extra == "geode" {
                call(
                    r,
                    root,
                    "install-app-mods",
                    &["check", extra, "--skip-unavailable"],
                )?;
            }
        }
        return Ok(());
    }
    let roles = application_roles::roles(root, o.personal)?;
    let mods = if o.personal {
        app_mods::configured_at(root, &paths.config)?
    } else {
        Vec::new()
    };
    let personal: Value = if o.personal || o.standard || !o.extras.is_empty() {
        crate::theme_render::read(&root.join("apps/personal.json"))?
    } else {
        Value::Null
    };
    if o.personal {
        for (name, app) in [
            ("browser", &roles["browser"]),
            ("chatgpt", &personal["chatgpt"]),
            ("steam", &personal["steam"]),
            ("vesktop", &personal["vesktop"]),
        ] {
            app_record(name, app)?;
        }
    }
    if o.standard {
        for (name, app) in [
            ("browser", &roles["browser"]),
            ("chatgpt", &personal["chatgpt"]),
            ("vesktop", &personal["vesktop"]),
        ] {
            app_record(name, app)?;
        }
    }
    if o.extras.iter().any(|e| e == "steam" || e == "geode") {
        app_record("steam", &personal["steam"])?;
    }
    let mut packages = Vec::new();
    for group in [
        Some("core"),
        (o.patched_niri && o.packages).then_some("niri-build"),
        o.greeter.then_some("greeter"),
        o.personal.then_some("personal"),
        o.standard.then_some("standard"),
        (o.personal || o.standard || o.clean_boot).then_some("firmware"),
        recording_selected.then_some("recording"),
    ]
    .into_iter()
    .flatten()
    {
        for name in manifest(root, group)? {
            packages.push((group, name));
        }
    }
    if o.extras.iter().any(|extra| extra == "brave-nightly") {
        app_record("browser", &application_roles::roles(root, true)?["browser"])?;
    }
    let conflicts = r.conflicts(root, paths, o.personal)?;
    if o.dry_run {
        println!(
            "Layer: {}",
            if o.personal {
                "core + personal"
            } else if o.standard {
                "standard"
            } else {
                "core desktop"
            }
        );
        for (group, name) in &packages {
            println!(
                "{group}: {name}: {}",
                if installed(r, name)? {
                    "already installed"
                } else {
                    "missing"
                }
            );
        }
        println!(
            "Niri: {}",
            if !o.packages {
                "preserve installed compositor (--no-packages)"
            } else if o.patched_niri {
                "build/verify niri-foundation, preserve signed stock rollback; activate on a fresh login"
            } else {
                "restore recorded stock package if Foundation owns a replacement"
            }
        );
        if o.standard {
            println!(
                "Standard: paru, Brave Origin/settings, ChatGPT, Vesktop, GNOME utilities, recording and firmware"
            );
        }
        if let Some(profile) = &o.performance_profile {
            println!(
                "Performance profile: {profile} (hardware/saved/explicit selection; reversible; no firmware/GPU tuning)"
            );
        }
        if !o.extras.is_empty() {
            println!("Extras: {}", o.extras.join(", "));
        }
        if o.personal {
            for (name, app) in [
                ("browser", &roles["browser"]),
                ("chatgpt", &personal["chatgpt"]),
                ("steam", &personal["steam"]),
                ("vesktop", &personal["vesktop"]),
            ] {
                println!(
                    "personal: {}: {name}",
                    app["package"].as_str().expect("validated")
                );
            }
            if mods.iter().any(|s| s == "spicetify") {
                println!("Spotify: checksum-pinned tools/Marketplace");
            }
            println!("Application modifications: {}", mods.join(", "));
        }
        if o.personal {
            println!(
                "Roles: {}",
                roles
                    .as_object()
                    .expect("validated roles")
                    .iter()
                    .map(|(name, app)| format!("{name}={}", app["desktop"].as_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        } else {
            println!("Application defaults: unchanged");
        }
        println!(
            "Services: Bluetooth enabled; AppArmor follows the running kernel policy; display manager is not restarted."
        );
        println!(
            "Boot: {}",
            if o.eac_boot {
                "resumable upstream EAC + independently signed Limine/LTS recovery"
            } else if r.eac_kernel() {
                "preserve EAC LSM policy and dedicated upstream-signed loader"
            } else if o.clean_boot {
                "explicit reversible GRUB/mkinitcpio setup"
            } else {
                "preserve boot stack; configure AppArmor if needed"
            }
        );
        for conflict in &conflicts {
            println!("CONFLICT: {conflict}");
        }
        call(
            r,
            root,
            "deploy",
            &["install", "--profile", &o.profile, "--dry-run"],
        )?;
        if !conflicts.is_empty() {
            return Err(invalid(
                "Deployment/MIME ownership conflicts; nothing changed",
            ));
        }
        return Ok(());
    }
    if r.uid() == 0 {
        return Err(invalid(
            "Run as your normal login user; native/AUR builds must not run as root",
        ));
    }
    if !r.executable("pacman") {
        return Err(invalid("Installer package management requires pacman"));
    }
    if !conflicts.is_empty() {
        return Err(invalid(format!(
            "Externally changed deployed paths: {}",
            conflicts.join(", ")
        )));
    }
    if o.clean_boot && (r.eac_kernel() || !r.clean_boot_supported()) {
        return Err(invalid(
            "--clean-boot requires conventional mkinitcpio/GRUB; EAC policy and dracut are never converted",
        ));
    }
    if o.personal && !o.packages {
        for (name, app) in [
            ("browser", &roles["browser"]),
            ("chatgpt", &personal["chatgpt"]),
            ("steam", &personal["steam"]),
            ("vesktop", &personal["vesktop"]),
        ] {
            r.validate_app(name, app)?;
        }
    }
    if !o.packages {
        let mut missing = Vec::new();
        for (_, name) in &packages {
            if !installed(r, name)? {
                missing.push(name.as_str());
            }
        }
        if !missing.is_empty() {
            return Err(invalid(format!(
                "Missing selected-profile packages with --no-packages: {}; nothing deployed",
                missing.join(", ")
            )));
        }
        if o.standard {
            if !r.executable("paru") {
                return Err(invalid("Standard requires paru when --no-packages is used"));
            }
            for (name, app) in [
                ("browser", &roles["browser"]),
                ("chatgpt", &personal["chatgpt"]),
                ("vesktop", &personal["vesktop"]),
            ] {
                r.validate_app(name, app)?;
            }
        }
        if o.extras.iter().any(|e| e == "steam" || e == "geode") {
            r.validate_app("steam", &personal["steam"])?;
        }
    }
    // Detect unsupported activation before authentication, builds or deployment.
    // The privileged writer repeats the same read-only prerequisites later.
    if !o.eac_boot && !r.eac_kernel() && !o.clean_boot && !r.apparmor_enabled() {
        call(r, root, "apparmor-setup", &["check"])?;
    }
    r.command(&words(&["sudo", "-v"]))?;
    if o.packages {
        let mut bootstrap = Vec::new();
        if o.greeter {
            bootstrap.push("--greeter");
        }
        if o.personal || o.standard || o.clean_boot {
            bootstrap.push("--firmware");
        }
        call(r, root, "bootstrap", &bootstrap)?;
        if o.standard {
            missing_packages(r, &manifest(root, "standard")?)?;
        }
        if recording_selected {
            missing_packages(r, &manifest(root, "recording")?)?;
        }
    }
    if recording_selected {
        check_recording_tools(r)?;
    }
    call(r, root, "build-nothing", &[])?;
    call(r, root, "build-backend", &[])?;
    if o.packages {
        if o.patched_niri {
            missing_packages(r, &manifest(root, "niri-build")?)?;
            call(r, root, "niri-package", &["install"])?;
        } else {
            call(r, root, "niri-package", &["restore"])?;
        }
    }
    call(r, root, "preflight", &["--profile", &o.profile])?;
    call(
        r,
        root,
        "deploy",
        &["--compositor", "niri", "--profile", &o.profile],
    )?;
    call(r, root, "preferences", &["install", "--theme-only"])?;
    let externally_managed_network = r.query(&words(&[
        "systemctl",
        "is-active",
        "--quiet",
        "systemd-networkd.service",
    ]))? || r.query(&words(&[
        "systemctl",
        "is-active",
        "--quiet",
        "connman.service",
    ]))?;
    let mut services = words(&["sudo", "systemctl", "enable", "--now", "bluetooth.service"]);
    if !externally_managed_network {
        services.push("NetworkManager.service".into());
    } else {
        println!(
            "Preserving external network manager; NetworkManager controls remain unavailable until explicitly configured."
        );
    }
    if !o.eac_boot && !r.eac_kernel() {
        r.command(&words(&["sudo", "systemctl", "enable", "apparmor.service"]))?;
    }
    r.command(&services)?;
    if o.clean_boot {
        sudo_script(r, root, "boot-setup")?;
    } else if !o.eac_boot && !r.eac_kernel() && !r.apparmor_enabled() {
        sudo_script(r, root, "apparmor-setup")?;
    }
    if o.greeter {
        sudo_script(r, root, "system-setup")?;
    }
    if o.clean_boot && o.greeter {
        sudo_script(r, root, "greeter-console-setup")?;
    }
    if o.clean_boot {
        sudo_script(r, root, "boot-optimize")?;
    }
    if o.personal {
        if o.packages {
            missing_packages(r, &manifest(root, "personal")?)?;
            provision(r, root, "browser", &roles["browser"], true)?;
            for name in ["chatgpt", "steam", "vesktop"] {
                provision(r, root, name, &personal[name], name == "vesktop")?;
            }
            if mods.iter().any(|s| s == "geode") {
                missing_packages(r, &words(&["protontricks", "libarchive"]))?;
            }
        }
        call(
            r,
            root,
            "install-app-mods",
            &[
                if o.packages { "install" } else { "check" },
                "--skip-unavailable",
            ],
        )?;
        call(r, root, "applications", &["apply", "--personal"])?;
        sudo_script(r, root, "browser-policy-setup")?;
        call(r, root, "theme-apply", &[])?;
    }
    if o.standard && o.packages {
        if !r.executable("paru") {
            call(r, root, "paru-bootstrap", &[])?;
        }
        provision(r, root, "browser", &roles["browser"], false)?;
        for name in ["chatgpt", "vesktop"] {
            provision(r, root, name, &personal[name], true)?;
        }
    }
    if o.standard {
        sudo_script(r, root, "browser-policy-setup")?;
        call(r, root, "theme-apply", &[])?;
    }
    if o.standard {
        if r.checkpoint_command(&script(root, "brave-config", &["migrate"]))? {
            call(
                r,
                root,
                "brave-config",
                &["apply", "--install", "--channel", "stable"],
            )?;
            call(r, root, "applications", &["browser-default"])?;
        } else {
            println!(
                "Stable Brave migration pending; Nightly profile and existing MIME choices preserved."
            );
        }
    } else if o.personal {
        call(
            r,
            root,
            "brave-config",
            &["apply", "--install", "--channel", "nightly"],
        )?;
    }
    if o.extras.iter().any(|extra| extra == "brave-nightly") {
        let nightly = application_roles::roles(root, true)?;
        if o.packages {
            provision(r, root, "browser", &nightly["browser"], true)?;
        } else {
            r.validate_app("browser", &nightly["browser"])?;
        }
    }
    if !o.extras.is_empty() {
        if o.packages && o.extras.iter().any(|e| e == "steam" || e == "geode") {
            provision(r, root, "steam", &personal["steam"], false)?;
        }
        if o.packages && o.extras.iter().any(|e| e == "geode") {
            missing_packages(r, &words(&["protontricks", "libarchive"]))?;
        }
        for extra in &o.extras {
            if extra == "spicetify" || extra == "geode" {
                call(
                    r,
                    root,
                    "install-app-mods",
                    &[
                        if o.packages { "install" } else { "check" },
                        extra,
                        "--skip-unavailable",
                    ],
                )?;
            }
        }
    }
    if o.eac_boot {
        let mut boot_args = vec!["resume", "--desktop"];
        if !o.packages {
            boot_args.push("--no-packages");
        }
        if o.recovery_only_dkms {
            boot_args.push("--allow-recovery-only-dkms");
        }
        if o.recovery_keys_confirmed {
            boot_args.push("--recovery-keys-confirmed");
        }
        // Desktop files are already installed. Only safe, durable firmware
        // checkpoints defer boot readiness; real boot defects still fail.
        if r.checkpoint_command(&script(root, "boot-integrate", &boot_args))? {
            call(r, root, "boot-integrate", &["finalize"])?;
        } else {
            println!(
                "Boot integration pending: firmware/reboot verification required. Desktop deployment is installed; rerun the same install command to resume."
            );
        }
    }
    if let Some(profile) = &o.performance_profile {
        call(
            r,
            root,
            "performance-profile",
            if profile == "off" {
                vec!["disable"]
            } else {
                vec!["enable", "lucky38"]
            }
            .as_slice(),
        )?;
    }
    doctor(r, root, o)?;
    if o.personal || o.extras.iter().any(|e| e == "spicetify" || e == "geode") {
        println!(
            "Application add-ons: first-use/game-dependent setup may remain pending; inspect scripts/install-app-mods plan."
        );
    }
    println!(
        "Desktop installation complete. Choose Niri at your next login. Restore: scripts/uninstall."
    );
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSelection {
    version: u8,
    layer: String,
    extras: Vec<String>,
    hardware_profile: String,
    greeter: bool,
    #[serde(default = "patched_default")]
    patched_niri: bool,
    #[serde(default = "existing_boot_profile")]
    boot_profile: String,
    #[serde(default)]
    recovery_only_dkms: bool,
}
fn existing_boot_profile() -> String {
    "existing".into()
}
fn patched_default() -> bool {
    true
}

fn selection(root: &Path, state_dir: &Path, args: &[String]) -> Result<Options> {
    let path = state_dir.join("installation.toml");
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut defaults = Vec::new();
            if !state_dir.join("manifest.json").exists()
                && !args.iter().any(|s| s == "--boot-profile")
            {
                defaults.extend(["--boot-profile".into(), "eac".into()]);
            }
            defaults.extend_from_slice(args);
            return Options::parse(root, &defaults);
        }
        Err(e) => return Err(e.into()),
    };
    let mut text = String::new();
    file.take(16385).read_to_string(&mut text)?;
    if text.len() > 16384 {
        return Err(invalid("Saved installation selection is too large"));
    }
    let saved: SavedSelection = toml::from_str(&text)
        .map_err(|e| invalid(format!("Invalid saved installation selection: {e}")))?;
    if saved.version != 1 || !["minimal", "standard", "personal"].contains(&saved.layer.as_str()) {
        return Err(invalid("Unsupported saved installation selection"));
    }
    let mut effective = Vec::new();
    if !args.iter().any(|s| s == "--boot-profile") {
        effective.extend(["--boot-profile".into(), saved.boot_profile]);
    }
    if saved.recovery_only_dkms && !args.iter().any(|s| s == "--allow-recovery-only-dkms") {
        effective.push("--allow-recovery-only-dkms".into());
    }
    let explicit_layer = args.iter().any(|s| {
        [
            "--minimal",
            "--core",
            "--standard",
            "--personal",
            "--everything",
        ]
        .contains(&s.as_str())
    });
    if !explicit_layer {
        effective.push(format!("--{}", saved.layer));
    }
    if !explicit_layer && !args.iter().any(|s| s == "--extra") {
        for extra in saved.extras {
            effective.extend(["--extra".into(), extra]);
        }
    }
    if !args.iter().any(|s| {
        s == "--profile"
            || s == "--hardware-profile"
            || s.starts_with("--profile=")
            || s.starts_with("--hardware-profile=")
    }) {
        effective.extend(["--profile".into(), saved.hardware_profile]);
    }
    if !args.iter().any(|s| s == "--greeter" || s == "--no-greeter") {
        effective.push(
            if saved.greeter {
                "--greeter"
            } else {
                "--no-greeter"
            }
            .into(),
        );
    }
    if !args
        .iter()
        .any(|s| s == "--patched-niri" || s == "--stock-niri")
        && !saved.patched_niri
    {
        effective.push("--stock-niri".into());
    }
    effective.extend_from_slice(args);
    Options::parse(root, &effective)
}

fn save_selection(dir: &Path, o: &Options) -> Result<()> {
    let layer = if o.personal {
        "personal"
    } else if o.standard {
        "standard"
    } else {
        "minimal"
    };
    // Values here have already been restricted to safe identifiers by Options::parse.
    let extras = o
        .extras
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let document = format!(
        "version = 1\nlayer = \"{layer}\"\nextras = [{extras}]\nhardware_profile = \"{}\"\ngreeter = {}\npatched_niri = {}\nboot_profile = \"{}\"\nrecovery_only_dkms = {}\n",
        o.profile,
        o.greeter,
        o.patched_niri,
        if o.eac_boot { "eac" } else { "existing" },
        o.recovery_only_dkms
    );
    state::atomic(&dir.join("installation.toml"), document.as_bytes(), 0o600)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let root = root.canonicalize()?;
    let paths = deploy::Paths::environment()?;
    let options = resolve_greeter(&Native, &selection(&root, &paths.state, args)?)?;
    // Dry-run/check never create a state directory or acquire a mutation lock.
    if options.dry_run || options.check {
        return execute(&root, &paths, &options, &mut Native);
    }
    let mut runtime = Native;
    if runtime.uid() == 0 {
        return Err(invalid("Run the installer as your normal login user"));
    }
    // Preflight ownership before creating even an installer lock directory.
    let conflicts = runtime.conflicts(&root, &paths, options.personal)?;
    if !conflicts.is_empty() {
        return Err(invalid(format!(
            "Installation conflicts: {}",
            conflicts.join(", ")
        )));
    }
    state::private_directory(&paths.state)?;
    let _lock = state::lock(&paths.state.join("install.lock"), Duration::from_secs(30))?;
    // Another installer may have completed while we waited for the lock.
    // Re-read its selection and ownership state before changing anything.
    let options = resolve_greeter(&runtime, &selection(&root, &paths.state, args)?)?;
    let conflicts = runtime.conflicts(&root, &paths, options.personal)?;
    if !conflicts.is_empty() {
        return Err(invalid(format!(
            "Installation conflicts: {}",
            conflicts.join(", ")
        )));
    }
    execute(&root, &paths, &options, &mut runtime).map_err(|e| {
        invalid(format!(
            "Installation stopped: {e}. Backups retained; resolve the error and rerun."
        ))
    })?;
    save_selection(&paths.state, &options)?;
    Ok(())
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn alias_free_manager_is_preserved_and_active_beats_enabled() {
        let missing = std::env::temp_dir().join(format!("df-no-dm-alias-{}", std::process::id()));
        let found = detect_display_manager(&missing, |check, manager| {
            Ok((check == "is-active" && manager == "gdm.service")
                || (check == "is-enabled" && manager == "sddm.service"))
        })
        .unwrap();
        assert_eq!(found.as_deref(), Some("gdm.service"));
        assert_eq!(
            detect_display_manager(&missing, |check, manager| Ok(
                check == "is-enabled" && manager == "lightdm.service"
            ))
            .unwrap()
            .as_deref(),
            Some("lightdm.service")
        );
    }

    #[test]
    fn fresh_boot_default_and_existing_selection_are_distinct() {
        let base = std::env::temp_dir().join(format!("df-boot-selection-{}", std::process::id()));
        fs::create_dir_all(base.join("profiles/default")).unwrap();
        let dir = base.join("state");
        fs::create_dir_all(&dir).unwrap();
        assert!(selection(&base, &dir, &[]).unwrap().eac_boot);
        fs::write(dir.join("manifest.json"), "{}").unwrap();
        assert!(!selection(&base, &dir, &[]).unwrap().eac_boot);
        let selected = Options::parse(
            &base,
            &[
                "--boot-profile".into(),
                "eac".into(),
                "--allow-recovery-only-dkms".into(),
            ],
        )
        .unwrap();
        save_selection(&dir, &selected).unwrap();
        let reused = selection(&base, &dir, &[]).unwrap();
        assert!(reused.eac_boot && reused.recovery_only_dkms);
        assert!(
            !selection(&base, &dir, &["--boot-profile".into(), "existing".into()])
                .unwrap()
                .eac_boot
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn automatically_preserved_manager_choice_survives_the_next_install() {
        struct ExistingManager;
        impl Runtime for ExistingManager {
            fn display_manager(&self) -> Result<Option<String>> {
                Ok(Some("sddm.service".into()))
            }
            fn query(&mut self, _: &[String]) -> Result<bool> {
                unreachable!()
            }
            fn command(&mut self, _: &[String]) -> Result<()> {
                unreachable!()
            }
            fn executable(&self, _: &str) -> bool {
                unreachable!()
            }
            fn validate_app(&self, _: &str, _: &Value) -> Result<()> {
                unreachable!()
            }
            fn desktop_exists(&self, _: &str) -> Result<bool> {
                unreachable!()
            }
            fn uid(&self) -> u32 {
                unreachable!()
            }
            fn conflicts(&self, _: &Path, _: &deploy::Paths, _: bool) -> Result<Vec<String>> {
                unreachable!()
            }
            fn apparmor_enabled(&self) -> bool {
                unreachable!()
            }
            fn clean_boot_supported(&self) -> bool {
                unreachable!()
            }
        }
        let root =
            std::env::temp_dir().join(format!("df-install-auto-greeter-{}", std::process::id()));
        fs::create_dir_all(root.join("profiles/default")).unwrap();
        fs::create_dir_all(root.join("state")).unwrap();
        let selected =
            resolve_greeter(&ExistingManager, &Options::parse(&root, &[]).unwrap()).unwrap();
        assert!(!selected.greeter && !selected.auto_greeter);
        save_selection(&root.join("state"), &selected).unwrap();
        let next = selection(&root, &root.join("state"), &[]).unwrap();
        assert!(!next.greeter && !next.auto_greeter);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recording_extra_is_optional_and_preserved() {
        let dir =
            std::env::temp_dir().join(format!("df-recording-selection-{}", std::process::id()));
        fs::create_dir_all(dir.join("profiles/default")).unwrap();
        fs::create_dir_all(dir.join("state")).unwrap();
        let minimal = Options::parse(&dir, &[]).unwrap();
        assert!(minimal.extras.is_empty());
        let options = Options::parse(&dir, &["--extra".into(), "recording".into()]).unwrap();
        save_selection(&dir.join("state"), &options).unwrap();
        assert_eq!(
            selection(&dir, &dir.join("state"), &[]).unwrap().extras,
            ["recording"]
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn saved_selection_is_reused_and_explicit_profile_overrides_it() {
        let dir = std::env::temp_dir().join(format!("df-install-selection-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("profiles/default")).unwrap();
        fs::create_dir_all(dir.join("state")).unwrap();
        let args: Vec<String> = ["--standard", "--extra", "geode", "--no-greeter"]
            .iter()
            .map(|s| (*s).into())
            .collect();
        let initial = Options::parse(&dir, &args).unwrap();
        save_selection(&dir.join("state"), &initial).unwrap();
        let reused = selection(&dir, &dir.join("state"), &["--dry-run".into()]).unwrap();
        assert!(reused.standard && reused.dry_run && !reused.greeter);
        assert_eq!(reused.extras, ["geode"]);
        let minimal = selection(&dir, &dir.join("state"), &["--minimal".into()]).unwrap();
        assert!(!minimal.standard && minimal.extras.is_empty());
        fs::write(dir.join("state/installation.toml"), "version = 99\nlayer = 'minimal'\nextras = []\nhardware_profile = 'default'\ngreeter = true\n").unwrap();
        assert!(selection(&dir, &dir.join("state"), &[]).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn compositor_selection_survives_reinstall_and_can_be_explicitly_changed() {
        let dir =
            std::env::temp_dir().join(format!("df-install-niri-choice-{}", std::process::id()));
        fs::create_dir_all(dir.join("profiles/default")).unwrap();
        fs::create_dir_all(dir.join("state")).unwrap();
        let initial = Options::parse(&dir, &["--stock-niri".into()]).unwrap();
        save_selection(&dir.join("state"), &initial).unwrap();
        assert!(
            !selection(&dir, &dir.join("state"), &[])
                .unwrap()
                .patched_niri
        );
        assert!(
            selection(&dir, &dir.join("state"), &["--patched-niri".into()])
                .unwrap()
                .patched_niri
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saved_custom_profile_does_not_require_missing_default_profile() {
        let dir =
            std::env::temp_dir().join(format!("df-install-custom-profile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("profiles/custom")).unwrap();
        fs::create_dir_all(dir.join("state")).unwrap();
        let initial = Options::parse(&dir, &["--profile".into(), "custom".into()]).unwrap();
        save_selection(&dir.join("state"), &initial).unwrap();
        assert_eq!(
            selection(&dir, &dir.join("state"), &["--dry-run".into()])
                .unwrap()
                .profile,
            "custom"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn waiting_installer_reads_the_selection_published_by_the_lock_owner() {
        let dir =
            std::env::temp_dir().join(format!("df-install-lock-selection-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("profiles/default")).unwrap();
        fs::create_dir_all(dir.join("state")).unwrap();
        let state_dir = dir.join("state");
        let minimal = Options::parse(&dir, &[]).unwrap();
        save_selection(&state_dir, &minimal).unwrap();
        let guard = state::lock(&state_dir.join("install.lock"), Duration::from_secs(2)).unwrap();
        let worker_root = dir.clone();
        let worker_state = state_dir.clone();
        let worker = std::thread::spawn(move || {
            let _guard =
                state::lock(&worker_state.join("install.lock"), Duration::from_secs(2)).unwrap();
            selection(&worker_root, &worker_state, &[]).unwrap()
        });
        let standard = Options::parse(&dir, &["--standard".into()]).unwrap();
        save_selection(&state_dir, &standard).unwrap();
        drop(guard);
        assert!(worker.join().unwrap().standard);
        fs::remove_dir_all(dir).unwrap();
    }
}
