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
    pub packages: bool,
    pub clean_boot: bool,
    pub profile: String,
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
            packages: true,
            clean_boot: false,
            profile: "default".into(),
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
                "--core" | "--minimal" | "--personal" | "--standard" => {
                    let personal = args[i] == "--personal";
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
                    if !["steam", "spicetify", "geode"].contains(&extra.as_str())
                        || o.extras.contains(extra)
                    {
                        return Err(invalid(
                            "Unknown or duplicate extra; choose steam, spicetify or geode",
                        ));
                    }
                    o.extras.push(extra.clone());
                }
                "--greeter" => o.greeter = true,
                "--no-greeter" => o.greeter = false,
                "--no-packages" => o.packages = false,
                "--clean-boot" => o.clean_boot = true,
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
    fn executable(&self, name: &str) -> bool;
    fn validate_app(&self, name: &str, app: &Value) -> Result<()>;
    fn desktop_exists(&self, name: &str) -> Result<bool>;
    fn uid(&self) -> u32;
    fn conflicts(&self, root: &Path, paths: &deploy::Paths, personal: bool) -> Result<Vec<String>>;
    fn apparmor_enabled(&self) -> bool;
    fn clean_boot_supported(&self) -> bool;
}
pub struct Native;
impl Runtime for Native {
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
    if r.query(&words(&["pacman", "-Si", package]))? {
        r.command(&words(&["sudo", "pacman", "-Syu", "--needed", package]))?;
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
fn doctor(r: &mut impl Runtime, root: &Path, o: &Options) -> Result<()> {
    let mut args = vec![
        "--installation",
        if o.personal { "--personal" } else { "--core" },
    ];
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
    if o.check {
        doctor(r, root, o)?;
        if o.standard {
            let roles = application_roles::roles(root, true)?;
            let apps: Value = crate::theme_render::read(&root.join("apps/personal.json"))?;
            for (name, app) in [
                ("browser", &roles["browser"]),
                ("chatgpt", &apps["chatgpt"]),
                ("vesktop", &apps["vesktop"]),
            ] {
                r.validate_app(name, app)?;
            }
            if !r.executable("paru") {
                return Err(invalid("Standard profile requires paru"));
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
    let roles = application_roles::roles(root, o.personal || o.standard)?;
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
        o.greeter.then_some("greeter"),
        o.personal.then_some("personal"),
        (o.personal || o.clean_boot).then_some("firmware"),
    ]
    .into_iter()
    .flatten()
    {
        for name in manifest(root, group)? {
            packages.push((group, name));
        }
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
        if o.standard {
            println!("Standard: Brave Origin Nightly, paru, ChatGPT, Vesktop");
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
        println!("Services: Bluetooth/AppArmor enabled; display manager is not restarted.");
        println!(
            "Boot: {}",
            if o.clean_boot {
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
    if o.clean_boot && !r.clean_boot_supported() {
        return Err(invalid(
            "--clean-boot requires an existing mkinitcpio/GRUB system; dracut is never converted",
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
        if o.standard {
            for (name, app) in [
                ("browser", &roles["browser"]),
                ("chatgpt", &personal["chatgpt"]),
                ("vesktop", &personal["vesktop"]),
            ] {
                r.validate_app(name, app)?;
            }
            if !r.executable("paru") {
                return Err(invalid("Standard requires paru when --no-packages is used"));
            }
        }
        if o.extras.iter().any(|e| e == "steam" || e == "geode") {
            r.validate_app("steam", &personal["steam"])?;
        }
    }
    r.command(&words(&["sudo", "-v"]))?;
    if o.packages {
        let mut bootstrap = Vec::new();
        if o.greeter {
            bootstrap.push("--greeter");
        }
        if o.personal || o.clean_boot {
            bootstrap.push("--firmware");
        }
        call(r, root, "bootstrap", &bootstrap)?;
    }
    call(r, root, "build-nothing", &[])?;
    call(r, root, "build-backend", &[])?;
    call(r, root, "preflight", &["--profile", &o.profile])?;
    call(
        r,
        root,
        "deploy",
        &["--compositor", "niri", "--profile", &o.profile],
    )?;
    call(r, root, "preferences", &["install", "--theme-only"])?;
    r.command(&words(&[
        "sudo",
        "systemctl",
        "enable",
        "bluetooth.service",
        "apparmor.service",
    ]))?;
    if o.clean_boot {
        sudo_script(r, root, "boot-setup")?;
    } else if !r.apparmor_enabled() {
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
        provision(r, root, "browser", &roles["browser"], true)?;
        for name in ["chatgpt", "vesktop"] {
            provision(r, root, name, &personal[name], true)?;
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
    doctor(r, root, o)?;
    println!("Install complete. Log into Niri and run scripts/doctor. Restore: scripts/uninstall.");
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
}

fn selection(root: &Path, state_dir: &Path, args: &[String]) -> Result<Options> {
    // Validate explicit options before considering saved configuration.
    Options::parse(root, args)?;
    let path = state_dir.join("installation.toml");
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Options::parse(root, args),
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
    let explicit_layer = args
        .iter()
        .any(|s| ["--minimal", "--core", "--standard", "--personal"].contains(&s.as_str()));
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
        "version = 1\nlayer = \"{layer}\"\nextras = [{extras}]\nhardware_profile = \"{}\"\ngreeter = {}\n",
        o.profile, o.greeter
    );
    state::atomic(&dir.join("installation.toml"), document.as_bytes(), 0o600)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let root = root.canonicalize()?;
    let paths = deploy::Paths::environment()?;
    let options = selection(&root, &paths.state, args)?;
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
}
