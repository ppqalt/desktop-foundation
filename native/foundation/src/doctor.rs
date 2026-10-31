//! Read-only installation/session diagnostics, with inert effects in fixtures.
use crate::{
    Result, app_mods, application_roles, apps, deploy, diagnostics, install, invalid, process,
    theme,
};
use serde_json::Value;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone, Debug)]
pub struct Reply {
    pub code: i32,
    pub text: String,
}
impl Reply {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}
pub trait Runtime {
    fn query(&self, args: &[String]) -> Reply;
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn exists(&self, path: &Path) -> bool;
    fn executable(&self, name: &str) -> bool;
    fn env(&self, key: &str) -> Option<String>;
    fn protected_helper(&self, path: &Path) -> bool;
    fn role_check(&self, root: &Path, personal: bool) -> Result<Value>;
    fn validate_role(&self, name: &str, app: &Value) -> Result<()>;
    fn conflicts(&self, root: &Path, paths: &deploy::Paths) -> Result<Vec<String>>;
    fn mods(&self, root: &Path, config: &Path) -> Result<Vec<String>>;
    fn hash(&self, path: &Path) -> Result<String>;
    fn spotify(&self, root: &Path) -> Result<()>;
}
pub struct Native;
impl Runtime for Native {
    fn query(&self, args: &[String]) -> Reply {
        let journal = args.first().is_some_and(|s| s == "journalctl");
        let timeout = if args.first().is_some_and(|s| s == "quickshell") {
            3
        } else if journal {
            20
        } else {
            10
        };
        match process::checked_capture(
            args,
            Duration::from_secs(timeout),
            if journal { 16 * 1024 * 1024 } else { 64 * 1024 },
        ) {
            Ok(output) => Reply {
                code: 0,
                text: String::from_utf8_lossy(&output.stdout).trim().into(),
            },
            Err(error) => Reply {
                code: 1,
                text: error.to_string(),
            },
        }
    }
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Diagnostic input is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(invalid("Diagnostic input exceeds 4 MiB"));
        }
        Ok(bytes)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn executable(&self, name: &str) -> bool {
        apps::executable(name)
    }
    fn env(&self, key: &str) -> Option<String> {
        env::var(key).ok().filter(|s| !s.is_empty())
    }
    fn protected_helper(&self, path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok_and(|m| {
            m.is_file()
                && m.uid() == 0
                && m.mode() & 0o6022 == 0
                && m.permissions().mode() & 0o111 != 0
        })
    }
    fn role_check(&self, root: &Path, personal: bool) -> Result<Value> {
        application_roles::check(root, personal)
    }
    fn validate_role(&self, name: &str, app: &Value) -> Result<()> {
        application_roles::validate(name, app).map(|_| ())
    }
    fn conflicts(&self, root: &Path, paths: &deploy::Paths) -> Result<Vec<String>> {
        let mut errors = deploy::inspect(root, paths)?;
        errors.extend(application_roles::conflicts()?);
        Ok(errors)
    }
    fn mods(&self, root: &Path, config: &Path) -> Result<Vec<String>> {
        app_mods::configured_at(root, config)
    }
    fn hash(&self, path: &Path) -> Result<String> {
        theme::wallpaper_hash(path)
    }
    fn spotify(&self, root: &Path) -> Result<()> {
        crate::spotify_theme::check(root).map(|_| ())
    }
}
pub fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).into()).collect()
}
pub fn query(r: &impl Runtime, args: &[&str]) -> Reply {
    r.query(&words(args))
}
pub fn text(r: &impl Runtime, path: &Path) -> Result<String> {
    String::from_utf8(r.read(path)?)
        .map_err(|_| invalid(format!("Invalid UTF-8: {}", path.display())))
}
fn json_file(r: &impl Runtime, path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&r.read(path)?)?)
}
#[derive(Default)]
pub struct Report {
    pub lines: Vec<String>,
    pub failures: Vec<String>,
}
impl Report {
    fn check(&mut self, ok: bool, message: impl Into<String>, warning: bool) {
        let m = message.into();
        self.lines.push(format!(
            "{}: {m}",
            if ok {
                "PASS"
            } else if warning {
                "WARN"
            } else {
                "FAIL"
            }
        ));
        if !ok && !warning {
            self.failures.push(m);
        }
    }
    fn note(&mut self, message: impl Into<String>) {
        self.lines.push(message.into());
    }
    fn result<T>(&mut self, value: Result<T>, name: &str) -> Option<T> {
        match value {
            Ok(v) => Some(v),
            Err(e) => {
                self.check(false, format!("{name}: {e}"), false);
                None
            }
        }
    }
    fn append(&mut self, other: Report) {
        self.lines.extend(other.lines);
        self.failures.extend(other.failures);
    }
}
#[derive(Default, Debug)]
pub struct Options {
    pub installation: bool,
    pub personal: bool,
    pub core: bool,
    pub no_greeter: bool,
    pub json: bool,
    pub startup: bool,
    pub benchmark: bool,
    pub save: Option<String>,
    pub compare: bool,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut o = Self::default();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--installation" => o.installation = true,
                "--personal" => o.personal = true,
                "--core" => o.core = true,
                "--no-greeter" => o.no_greeter = true,
                "--json" => o.json = true,
                "--startup" => o.startup = true,
                "--benchmark" => o.benchmark = true,
                "--compare" => o.compare = true,
                "--save" => {
                    i += 1;
                    o.save = Some(
                        args.get(i)
                            .ok_or_else(|| invalid("--save requires a label"))?
                            .clone(),
                    );
                }
                value if value.starts_with("--save=") => o.save = Some(value[7..].into()),
                _ => return Err(invalid(format!("Unknown doctor option: {}", args[i]))),
            }
            i += 1;
        }
        if o.personal && o.core {
            return Err(invalid("Choose --personal or --core"));
        }
        if o.json && args != ["--json"] {
            return Err(invalid("--json must be used alone"));
        }
        if !o.startup && (o.benchmark || o.compare || o.save.is_some()) {
            return Err(invalid(
                "--benchmark, --save and --compare require --startup",
            ));
        }
        Ok(o)
    }
}
pub fn shell_instance_count(reply: &str) -> Result<usize> {
    let lines: Vec<_> = reply.trim().lines().collect();
    if lines.is_empty()
        || lines.len() == 2
            && lines[0].starts_with("No running instances for \"")
            && lines[0].ends_with('"')
            && lines[1] == "Use --all to list all instances."
    {
        return Ok(0);
    }
    serde_json::from_str::<Value>(reply)?
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| invalid("Quickshell did not return an instance list"))
}
fn pam(report: &mut Report, r: &impl Runtime) {
    if let Some(bytes) = report.result(
        r.read(Path::new("/etc/pam.d/greetd")),
        "greetd PAM configuration unreadable",
    ) {
        report.check(
            crate::system_setup::pam_ready(&bytes),
            "greetd ordered oo7 password/readiness hooks; see docs/keyring.md",
            false,
        );
    }
}
pub fn session(root: &Path, paths: &deploy::Paths, r: &impl Runtime) -> Report {
    let mut out = Report::default();
    for name in [
        "quickshell",
        "kitty",
        "wl-copy",
        "wl-paste",
        "grim",
        "slurp",
        "jq",
        "dbus-update-activation-environment",
        "niri",
        "xwayland-satellite",
    ] {
        out.check(
            r.executable(name),
            format!("Executable {name}; missing tools: scripts/bootstrap"),
            false,
        );
    }
    for key in [
        "WAYLAND_DISPLAY",
        "NIRI_SOCKET",
        "XDG_CURRENT_DESKTOP",
        "XDG_RUNTIME_DIR",
    ] {
        out.check(
            r.env(key).is_some(),
            format!("Session environment {key}"),
            false,
        );
    }
    for unit in [
        "desktop-foundation-clipboard-persist",
        "pipewire",
        "pipewire-pulse",
        "wireplumber",
        "oo7-daemon",
        "xdg-desktop-portal",
        "xdg-desktop-portal-gnome",
        "xdg-desktop-portal-gtk",
        "hyprpolkitagent",
        "desktop-foundation-shell",
        "desktop-foundation-clipboard",
    ] {
        let mut reply = query(
            r,
            &[
                "systemctl",
                "--user",
                "is-active",
                &format!("{unit}.service"),
            ],
        );
        if !reply.ok() && unit == "pipewire-pulse" {
            reply = query(
                r,
                &["systemctl", "--user", "is-active", "pipewire-pulse.socket"],
            );
        }
        out.check(
            reply.ok(),
            format!(
                "{unit}: {}; inspect journalctl --user -u {unit}.service",
                reply.text
            ),
            false,
        );
    }
    let shell = root.join("shell").to_string_lossy().into_owned();
    let reply = query(r, &["quickshell", "list", "--json", "--path", &shell]);
    if reply.ok() {
        match shell_instance_count(&reply.text) {
            Ok(n) => out.check(
                n == 1,
                format!("Shell instances: {n}; expected one for this checkout"),
                false,
            ),
            Err(e) => out.check(
                false,
                format!("Unable to parse Quickshell instance list: {e}"),
                false,
            ),
        }
    } else {
        out.check(
            false,
            format!("Unable to query Quickshell instances: {}", reply.text),
            false,
        );
    }
    for unit in [
        "desktop-foundation-wallpaper",
        "desktop-foundation-clipboard-persist",
    ] {
        let reply = query(
            r,
            &[
                "systemctl",
                "--user",
                "show",
                &format!("{unit}.service"),
                "--property=FragmentPath",
                "--value",
            ],
        );
        out.check(
            reply.ok() && Path::new(&reply.text).starts_with(paths.config.join("systemd/user")),
            format!("{unit}: persistent deployment unit"),
            false,
        );
        out.check(
            query(
                r,
                &[
                    "systemctl",
                    "--user",
                    "is-enabled",
                    &format!("{unit}.service"),
                ],
            )
            .ok(),
            format!("{unit}: Niri session activation"),
            false,
        );
    }
    if query(r, &["systemctl", "is-active", "greetd.service"]).ok() {
        pam(&mut out, r);
    }
    for name in ["swaybg", "wl-clip-persist", "oo7-daemon"] {
        let reply = query(r, &["pgrep", "-x", name]);
        let count = if reply.ok() {
            reply.text.lines().count()
        } else {
            0
        };
        out.check(count == 1, format!("{name} instances: {count}"), false);
    }
    out.check(
        !query(r, &["pgrep", "-x", "gnome-keyring-d"]).ok(),
        "No competing GNOME Keyring daemon",
        false,
    );
    let failed = query(
        r,
        &["systemctl", "--user", "--failed", "--no-legend", "--plain"],
    );
    out.check(
        failed.ok() && failed.text.is_empty(),
        format!("No failed user services: {}", failed.text),
        false,
    );
    for unit in [
        "desktop-foundation-wallpaper.service",
        "desktop-foundation-notifications.service",
    ] {
        let reply = query(r, &["systemctl", "--user", "is-active", unit]);
        out.check(reply.ok(), format!("{unit}: {}", reply.text), false);
    }
    let dark = query(
        r,
        &[
            "gsettings",
            "get",
            "org.gnome.desktop.interface",
            "color-scheme",
        ],
    );
    out.check(
        dark.ok() && dark.text == "'prefer-dark'",
        "System dark preference; restore with scripts/dark-mode",
        false,
    );
    out.check(
        query(r, &["niri", "msg", "-j", "outputs"]).ok(),
        "Niri IPC",
        false,
    );
    let config = paths
        .config
        .join("niri/config.kdl")
        .to_string_lossy()
        .into_owned();
    let valid = query(r, &["niri", "validate", "-c", &config]);
    out.check(
        valid.ok(),
        format!("Niri config validation: {}", valid.text),
        false,
    );
    let reply = query(
        r,
        &[
            "quickshell",
            "ipc",
            "--path",
            &shell,
            "call",
            "foundation",
            "status",
        ],
    );
    if reply.ok() {
        match serde_json::from_str::<Value>(&reply.text) {
            Ok(v) => {
                out.check(
                    v["ready"] == true
                        && v["backend"] == "niri"
                        && v["backendDiagnostics"]["configFailed"] == false,
                    "Niri adapter ready without config failure",
                    false,
                );
                out.check(
                    v["backendDiagnostics"]["keyboardLayouts"]["names"]
                        .as_array()
                        .is_some_and(|a| {
                            a.iter()
                                .any(|v| v.as_str().is_some_and(|s| s.contains("Finnish")))
                        }),
                    "Niri reports Finnish keyboard layout",
                    false,
                );
            }
            Err(e) => out.check(
                false,
                format!("Niri shell diagnostics unavailable: {e}"),
                false,
            ),
        }
    } else {
        out.check(
            false,
            format!("Niri shell diagnostics unavailable: {}", reply.text),
            false,
        );
    }
    out
}
fn secret_service(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut section = false;
    text.lines().any(|line| {
        let line = line.trim();
        if line.starts_with('[') {
            section = line == "[D-BUS Service]";
            return false;
        }
        section
            && line.split_once('=').is_some_and(|(k, v)| {
                k.trim() == "SystemdService" && v.trim() == "dbus-org.freedesktop.secrets.service"
            })
    })
}
pub fn installation(root: &Path, paths: &deploy::Paths, o: &Options, r: &impl Runtime) -> Report {
    let mut out = Report::default();
    for group in [
        Some("core"),
        (!o.no_greeter).then_some("greeter"),
        o.personal.then_some("personal"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(packages) = out.result(install::manifest(root, group), "Package manifest") {
            for name in packages {
                out.check(
                    query(r, &["pacman", "-T", &name]).ok(),
                    format!("Package {name}"),
                    false,
                );
            }
        }
    }
    for unit in [
        Some("bluetooth"),
        Some("apparmor"),
        (!o.no_greeter).then_some("greetd"),
    ]
    .into_iter()
    .flatten()
    {
        out.check(
            query(r, &["systemctl", "is-enabled", &format!("{unit}.service")]).ok(),
            format!("{unit} enabled"),
            false,
        );
    }
    let apparmor = Path::new("/sys/module/apparmor/parameters/enabled");
    out.check(r.exists(apparmor), "Kernel has AppArmor support", false);
    out.check(
        text(r, apparmor).is_ok_and(|v| v.trim() == "Y"),
        "AppArmor kernel enforcement; see docs/APPARMOR.md",
        o.installation,
    );
    if !o.installation {
        out.check(
            query(r, &["systemctl", "is-active", "apparmor.service"]).ok(),
            "AppArmor profile service active",
            false,
        );
    }
    let failed = query(r, &["systemctl", "--failed", "--no-legend", "--plain"]);
    out.check(
        failed.ok() && failed.text.is_empty(),
        "No failed system services",
        false,
    );
    for path in ["/usr/lib/oo7-daemon", "/usr/lib/security/pam_oo7.so"] {
        out.check(r.exists(Path::new(path)), path, false);
    }
    let oo7 = query(
        r,
        &["systemctl", "--user", "is-enabled", "oo7-daemon.service"],
    );
    out.check(
        oo7.ok(),
        "oo7 login activation",
        o.installation && r.env("XDG_RUNTIME_DIR").is_none(),
    );
    for unit in [
        "gnome-keyring-daemon.service",
        "gnome-keyring-daemon.socket",
    ] {
        let v = query(
            r,
            &[
                "systemctl",
                "--user",
                "show",
                unit,
                "--property=LoadState",
                "--value",
            ],
        );
        out.check(
            v.ok() && ["masked", "not-found"].contains(&v.text.as_str()),
            format!("{unit}: inactive provider"),
            o.installation && !v.ok(),
        );
    }
    let activation = paths
        .data
        .join("dbus-1/services/org.freedesktop.secrets.service");
    let activation = if r.exists(&activation) {
        activation
    } else {
        PathBuf::from("/usr/share/dbus-1/services/org.freedesktop.secrets.service")
    };
    if let Some(bytes) = out.result(r.read(&activation), "Secret Service activation unreadable") {
        out.check(
            secret_service(&bytes),
            "Secret Service uses packaged oo7 alias",
            false,
        );
    }
    if !o.no_greeter {
        pam(&mut out, r);
        out.check(
            r.protected_helper(Path::new("/usr/local/libexec/desktop-foundation-oo7-ready")),
            "oo7 readiness executable is root-owned and protected",
            false,
        );
    }
    for name in [
        "niri/config.kdl",
        "quickshell/desktop-foundation",
        "kitty",
        "fish",
        "fastfetch",
    ] {
        out.check(
            r.exists(&paths.config.join(name)),
            format!("Deployed {name}"),
            false,
        );
    }
    let config = paths
        .config
        .join("niri/config.kdl")
        .to_string_lossy()
        .into_owned();
    let valid = query(r, &["niri", "validate", "-c", &config]);
    out.check(
        valid.ok(),
        format!("Niri configuration capabilities: {}", valid.text),
        false,
    );
    for name in [
        "native/nothing/target/release/foundation-nothing",
        "native/foundation/target/release/desktop-foundationctl",
    ] {
        out.check(r.exists(&root.join(name)), format!("Backend {name}"), false);
    }
    out.check(
        r.exists(&paths.data.join("wallpapers/wallhaven-135w7w.png")),
        "Stable wallpaper",
        false,
    );
    for family in [
        "Google Sans Flex",
        "Google Sans Code",
        "GoogleSansCode Nerd Font Mono",
    ] {
        let v = query(r, &["fc-match", "-f", "%{family}", family]);
        out.check(
            v.ok() && v.text.contains(family),
            format!("Font {family}"),
            false,
        );
    }
    if !o.no_greeter {
        let v = query(r, &["tuigreet", "--background", "matrix", "--dump-config"]);
        out.check(
            v.ok() && v.text.contains("kind = \"matrix\""),
            "Matrix-capable tuigreet",
            false,
        );
    }
    if o.personal {
        let v = r.query(&[
            root.join("scripts/foundation")
                .to_string_lossy()
                .into_owned(),
            "browser-theme".into(),
            "status".into(),
        ]);
        out.check(
            v.ok() && serde_json::from_str::<Value>(&v.text).is_ok_and(|v| v["enabled"] == true),
            "Brave managed theme policy adapter",
            false,
        );
        out.check(
            r.executable("brave-origin-nightly"),
            "Personal executable brave-origin-nightly",
            false,
        );
        if let Some(mods) = out.result(r.mods(root, &paths.config), "Application modifications")
            && mods.iter().any(|s| s == "spicetify")
        {
            for name in ["spotify", "spicetify"] {
                out.check(
                    r.exists(&paths.data.join("desktop-foundation/apps").join(name)),
                    format!("Pinned application {name}"),
                    false,
                );
            }
            out.check(
                r.exists(
                    &paths
                        .config
                        .join("spicetify/CustomApps/marketplace/manifest.json"),
                ),
                "Marketplace installed",
                false,
            );
            out.check(
                r.exists(&paths.config.join("spotify/prefs")),
                "Spotify initialized; first login then normal quit/reopen",
                true,
            );
            let check = r.spotify(root);
            out.check(
                check.is_ok(),
                format!(
                    "Spotify/Spicetify configuration: {}",
                    check
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "valid".into())
                ),
                false,
            );
        }
        if let Some(personal) = out.result(
            json_file(r, &root.join("apps/personal.json")),
            "Personal application manifest",
        ) {
            for name in ["chatgpt", "vesktop", "steam"] {
                let result = r.validate_role(name, &personal[name]);
                out.check(
                    result.is_ok(),
                    format!(
                        "Application {name}: {}",
                        result
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "valid".into())
                    ),
                    false,
                );
                if name != "steam" {
                    if let Some(package) = personal[name]["package"].as_str() {
                        out.check(
                            query(r, &["pacman", "-Q", package]).ok(),
                            format!("Package {package}"),
                            false,
                        );
                    } else {
                        out.check(false, format!("Missing package for {name}"), false);
                    }
                }
            }
        }
    }
    let journal = paths.state.join("application-roles.json");
    let selected_personal = if r.exists(&journal) {
        let recorded = out.result(json_file(r, &journal), "Application role journal");
        o.personal || recorded.is_some_and(|v| v["profile"] == "personal")
    } else {
        o.personal
    };
    if let Some(roles) = out.result(
        r.role_check(root, selected_personal),
        "Application defaults",
    ) {
        out.check(
            roles["messages"].as_array().is_some() && roles["failures"].as_array().is_some(),
            "Application defaults response schema",
            false,
        );
        if let Some(messages) = roles["messages"].as_array() {
            for line in messages.iter().filter_map(Value::as_str) {
                out.note(line);
            }
        }
        if let Some(errors) = roles["failures"].as_array() {
            for line in errors.iter().filter_map(Value::as_str) {
                out.check(false, format!("Application defaults: {line}"), false);
            }
        }
    }
    if let Some(conflicts) = out.result(r.conflicts(root, paths), "Deployment ownership") {
        out.check(
            conflicts.is_empty(),
            format!(
                "Deployment ownership{}",
                if conflicts.is_empty() {
                    String::new()
                } else {
                    format!(": {}", conflicts.join(", "))
                }
            ),
            false,
        );
    }
    let theme = (|| -> Result<()> {
        let current = paths.state.join("theme/current");
        let metadata = json_file(r, &current.join("metadata.json"))?;
        let wallpaper: toml::Value = toml::from_str(&text(r, &current.join("wallpaper.toml"))?)
            .map_err(|e| invalid(e.to_string()))?;
        let image = wallpaper
            .get("image")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| invalid("Runtime wallpaper image missing"))?;
        out.check(
            metadata["wallpaperHash"].as_str() == Some(&r.hash(Path::new(image))?),
            "Runtime wallpaper content hash matches revision",
            false,
        );
        for name in [
            "semantic.json",
            "overview.png",
            "niri.kdl",
            "terminal/kitty/kitty.conf",
            "brave/manifest.json",
            "spotify/color.ini",
        ] {
            out.check(
                r.exists(&current.join(name)),
                format!("Theme output {name}"),
                false,
            );
        }
        Ok(())
    })();
    if let Err(e) = theme {
        out.check(false, format!("Runtime theme: {e}"), false);
    }
    for unit in [
        "desktop-foundation-wallpaper",
        "desktop-foundation-clipboard-persist",
    ] {
        out.check(
            r.exists(
                &paths
                    .config
                    .join("systemd/user/niri.service.wants")
                    .join(format!("{unit}.service")),
            ),
            format!("{unit}: session activation"),
            false,
        );
    }
    let paired = query(r, &["bluetoothctl", "devices", "Paired"]);
    out.check(
        paired.ok() && !paired.text.is_empty(),
        "Bluetooth pairing present; pair through Blueman if empty",
        true,
    );
    let failed = query(
        r,
        &["systemctl", "--user", "--failed", "--no-legend", "--plain"],
    );
    out.check(
        failed.ok() && failed.text.is_empty(),
        "No failed user services",
        o.installation && !failed.ok(),
    );
    for unit in [
        "pipewire.socket",
        "pipewire-pulse.socket",
        "wireplumber.service",
        "xdg-desktop-portal.service",
        "xdg-desktop-portal-gnome.service",
        "xdg-desktop-portal-gtk.service",
        "hyprpolkitagent.service",
    ] {
        let v = query(
            r,
            &[
                "systemctl",
                "--user",
                "show",
                unit,
                "--property=LoadState",
                "--value",
            ],
        );
        out.check(
            v.ok() && v.text == "loaded",
            format!("Session dependency {unit}"),
            o.installation && !v.ok(),
        );
    }
    if !o.installation && r.env("NIRI_SOCKET").is_some() {
        out.append(session(root, paths, r));
    }
    out
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    if args == ["--help"] || args == ["-h"] {
        println!(
            "Usage: doctor [--installation] [--personal|--core] [--no-greeter]\n       doctor --json\n       doctor --startup [--benchmark] [--save LABEL] [--compare]"
        );
        return Ok(());
    }
    let options = Options::parse(args)?;
    if options.json {
        println!("{}", serde_json::to_string(&diagnostics::report(root)?)?);
        return Ok(());
    }
    // SAFETY: geteuid has no arguments or pointers. Operational checks are user-scoped.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Run diagnostics as your normal desktop user"));
    }
    if options.startup {
        return crate::startup_report::run(root, &options, &Native);
    }
    let paths = deploy::Paths::environment()?;
    let report = installation(root, &paths, &options, &Native);
    for line in report.lines {
        println!("{line}");
    }
    if report.failures.is_empty() {
        println!("HEALTHY");
        Ok(())
    } else {
        Err(invalid(format!(
            "{} diagnostic issue(s)",
            report.failures.len()
        )))
    }
}
pub fn acceptance(root: &Path) -> Result<()> {
    let r = Native;
    println!(
        "Boot ID: {}",
        text(&r, Path::new("/proc/sys/kernel/random/boot_id"))?.trim()
    );
    println!(
        "Session: {}",
        r.env("XDG_SESSION_ID").unwrap_or_else(|| "unknown".into())
    );
    let result = run(root, &[]);
    for args in [
        words(&["systemctl", "is-active", "greetd.service"]),
        words(&["systemctl", "is-enabled", "cosmic-greeter.service"]),
        words(&["pgrep", "-a", "-x", "xwayland-satell"]),
        words(&["systemctl", "--user", "--failed", "--no-pager"]),
    ] {
        let reply = r.query(&args);
        println!("{}: {}", args.join(" "), reply.text);
    }
    result
}
