//! An on-demand launcher for an existing local Ollama/Odysseus installation.
use crate::{Result, apps, invalid, process, state};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const MAX_CONFIG: u64 = 64 * 1024;
const MAX_MODEL_INVENTORY: u64 = 1024 * 1024;
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    compose_directory: Option<String>,
    url: Option<String>,
    ollama_url: Option<String>,
    startup_timeout_seconds: Option<u64>,
}

struct Endpoint {
    url: String,
    authority: String,
    addresses: Vec<SocketAddr>,
}

struct Config {
    directory: PathBuf,
    compose_file: Option<PathBuf>,
    browser: Endpoint,
    ollama: Endpoint,
    timeout: Duration,
}

fn read_settings(path: &Path) -> Result<Settings> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_CONFIG {
        return Err(invalid(
            "Local AI configuration must be a regular file below 64 KiB",
        ));
    }
    let mut text = String::new();
    Read::by_ref(&mut file)
        .take(MAX_CONFIG + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_CONFIG {
        return Err(invalid("Local AI configuration exceeds 64 KiB"));
    }
    toml::from_str(&text)
        .map_err(|error| invalid(format!("Invalid local AI configuration: {error}")))
}

fn endpoint(url: String) -> Result<Endpoint> {
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| invalid("Local AI URLs must use http:// on a loopback address"))?
        .trim_end_matches('/');
    let (host, port) = if let Some(port) = authority.strip_prefix("[::1]:") {
        ("::1", port)
    } else {
        authority
            .rsplit_once(':')
            .ok_or_else(|| invalid("Local AI URLs must include an explicit port"))?
    };
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| invalid("Local AI URL has an invalid port"))?;
    let addresses = match host {
        "localhost" => vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
        ],
        "127.0.0.1" => vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)],
        "::1" => vec![SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)],
        _ => {
            return Err(invalid(
                "Local AI URLs must use localhost, 127.0.0.1 or [::1]",
            ));
        }
    };
    Ok(Endpoint {
        url: format!("http://{authority}/"),
        authority: authority.into(),
        addresses,
    })
}

fn configuration(root: &Path) -> Result<Config> {
    let mut settings = read_settings(&root.join("config/local-ai.toml"))?;
    let override_path =
        state::xdg("XDG_CONFIG_HOME", ".config")?.join("desktop-foundation/local-ai.toml");
    if override_path.exists() {
        let custom = read_settings(&override_path)?;
        settings.compose_directory = custom.compose_directory.or(settings.compose_directory);
        settings.url = custom.url.or(settings.url);
        settings.ollama_url = custom.ollama_url.or(settings.ollama_url);
        settings.startup_timeout_seconds = custom
            .startup_timeout_seconds
            .or(settings.startup_timeout_seconds);
    }
    let directory = settings
        .compose_directory
        .ok_or_else(|| invalid("Local AI compose_directory is missing"))?;
    if directory.is_empty() || directory.contains('\0') {
        return Err(invalid(
            "Local AI compose_directory must be an absolute path or ~/path",
        ));
    }
    let directory = match directory.strip_prefix("~/") {
        Some(suffix) => state::home()?.join(suffix),
        None => PathBuf::from(directory),
    };
    if !directory.is_absolute() {
        return Err(invalid(
            "Local AI compose_directory must be an absolute path or ~/path",
        ));
    }
    let compose_file = [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ]
    .into_iter()
    .map(|name| directory.join(name))
    .find(|path| path.is_file());
    let timeout = settings.startup_timeout_seconds.unwrap_or(90);
    if !(10..=120).contains(&timeout) {
        return Err(invalid(
            "Local AI startup_timeout_seconds must be between 10 and 120",
        ));
    }
    Ok(Config {
        directory,
        compose_file,
        browser: endpoint(
            settings
                .url
                .ok_or_else(|| invalid("Local AI URL is missing"))?,
        )?,
        ollama: endpoint(
            settings
                .ollama_url
                .ok_or_else(|| invalid("Local AI ollama_url is missing"))?,
        )?,
        timeout: Duration::from_secs(timeout),
    })
}

fn compose(config: &Config, arguments: &[&str]) -> Result<Vec<String>> {
    let file = config.compose_file.as_ref().ok_or_else(|| invalid(
        "Odysseus Compose project is missing; set compose_directory in ~/.config/desktop-foundation/local-ai.toml",
    ))?;
    let mut command = vec![
        "docker".into(),
        "compose".into(),
        "--project-directory".into(),
        path_text(&config.directory)?,
        "--file".into(),
        path_text(file)?,
    ];
    command.extend(arguments.iter().map(|value| (*value).into()));
    Ok(command)
}

fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Local AI paths must be UTF-8"))
}

/// Inspect configuration and executable availability without contacting services.
pub fn plan(root: &Path) -> Result<Value> {
    let config = configuration(root)?;
    Ok(json!({
        "composeDirectory": config.directory,
        "composeFile": config.compose_file,
        "configured": config.compose_file.is_some(),
        "url": config.browser.url,
        "ollamaUrl": config.ollama.url,
        "startupTimeoutSeconds": config.timeout.as_secs(),
        "tools": { "docker": apps::executable("docker"), "systemctl": apps::executable("systemctl"), "curl": apps::executable("curl") },
        "browserCommand": apps::command(root, "browser").ok(),
        "containerAction": "start-existing",
    }))
}

fn service_state(name: &str) -> Result<String> {
    let output = process::run(
        &[
            "systemctl".into(),
            "show".into(),
            "--property=LoadState".into(),
            "--property=ActiveState".into(),
            name.into(),
        ],
        None,
        QUERY_TIMEOUT,
        true,
    )?;
    if !output.status.success() {
        return Err(invalid(format!(
            "Cannot inspect {name}; check systemd service availability"
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| invalid("Invalid systemd service response"))?;
    if !text.lines().any(|line| line == "LoadState=loaded") {
        return Ok("not-installed".into());
    }
    text.lines()
        .find_map(|line| line.strip_prefix("ActiveState=").map(str::to_owned))
        .ok_or_else(|| invalid("Systemd did not report a service state"))
}

fn containers(config: &Config) -> Result<Vec<Value>> {
    let output = process::run(
        &compose(config, &["ps", "--all", "--format", "json"])?,
        None,
        QUERY_TIMEOUT,
        true,
    )?;
    if !output.status.success() {
        return Err(invalid(
            "Cannot inspect Odysseus containers; check access to Docker",
        ));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| invalid("Invalid Docker response"))?;
    let rows: Vec<Value> = if text.trim().starts_with('[') {
        serde_json::from_str(&text)?
    } else {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<std::result::Result<_, _>>()?
    };
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "service": row.get("Service").and_then(Value::as_str).unwrap_or("unknown"),
                "state": row.get("State").and_then(Value::as_str).unwrap_or("unknown"),
                "health": row.get("Health").and_then(Value::as_str).unwrap_or(""),
            })
        })
        .collect())
}

/// Read only service/container lifecycle metadata. No HTTP or model queries.
pub fn status(root: &Path) -> Result<Value> {
    let config = configuration(root)?;
    let docker = service_state("docker.service")?;
    let ollama = service_state("ollama.service")?;
    let containers = if docker == "active" && config.compose_file.is_some() {
        containers(&config)?
    } else {
        Vec::new()
    };
    Ok(
        json!({ "url": config.browser.url, "docker": docker, "ollama": ollama,
        "containers": containers }),
    )
}

fn ensure_service(name: &str) -> Result<()> {
    match service_state(name)?.as_str() {
        "active" => return Ok(()),
        "not-installed" => return Err(invalid(format!("Install {name} before starting local AI"))),
        _ => {}
    }
    // systemctl uses the session's Polkit agent when authorization is needed.
    let output = process::run(
        &["systemctl".into(), "start".into(), name.into()],
        None,
        Duration::from_secs(120),
        true,
    )?;
    if !output.status.success() || service_state(name)? != "active" {
        return Err(invalid(format!(
            "Could not start {name}; authorize it or start the service in your terminal"
        )));
    }
    Ok(())
}

fn require_existing_services(config: &Config, rows: &[Value]) -> Result<()> {
    let output = process::run(
        &compose(config, &["config", "--services"])?,
        None,
        QUERY_TIMEOUT,
        true,
    )?;
    if !output.status.success() {
        return Err(invalid(
            "Odysseus Compose configuration is unavailable or invalid",
        ));
    }
    let services = String::from_utf8(output.stdout)
        .map_err(|_| invalid("Invalid Compose service response"))?;
    let mut count = 0;
    for service in services
        .lines()
        .map(str::trim)
        .filter(|service| !service.is_empty())
    {
        count += 1;
        if !rows
            .iter()
            .any(|row| row["service"].as_str() == Some(service))
        {
            return Err(invalid(
                "Odysseus containers have not all been created; complete the upstream Compose setup first",
            ));
        }
    }
    if count == 0 {
        return Err(invalid("Odysseus Compose project has no services"));
    }
    Ok(())
}

fn health(endpoint: &Endpoint, path: &str, deadline: Instant) -> Result<bool> {
    for address in &endpoint.addresses {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(2));
        if remaining.is_zero() {
            return Ok(false);
        }
        let Ok(mut stream) = TcpStream::connect_timeout(address, remaining) else {
            continue;
        };
        let attempt_deadline = deadline.min(Instant::now() + Duration::from_secs(2));
        let remaining = attempt_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        stream.set_write_timeout(Some(remaining))?;
        if write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            endpoint.authority
        )
        .is_err()
        {
            continue;
        }
        // Stop after the first HTTP status line. No response body is retrieved.
        let mut line = Vec::new();
        let mut byte = [0_u8; 1];
        while line.len() < 128 {
            let remaining = attempt_deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            stream.set_read_timeout(Some(remaining))?;
            if stream.read(&mut byte).unwrap_or(0) != 1 {
                break;
            }
            line.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let complete = line.last() == Some(&b'\n');
        let line = String::from_utf8_lossy(&line);
        let mut fields = line.split_whitespace();
        if complete
            && matches!(fields.next(), Some("HTTP/1.0" | "HTTP/1.1"))
            && fields.next().is_some_and(|status| status == "200")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn wait_ready(config: &Config) -> Result<()> {
    let deadline = Instant::now() + config.timeout;
    loop {
        let ollama = health(&config.ollama, "/api/version", deadline)?;
        let browser = health(&config.browser, "/api/health", deadline)?;
        if ollama && browser {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(invalid(
                "Ollama or Odysseus did not become ready; inspect their service status",
            ));
        }
        thread::sleep(remaining.min(Duration::from_millis(250)));
    }
}

#[derive(Deserialize)]
struct ModelInventory {
    models: Vec<ModelIdentity>,
}

#[derive(Deserialize)]
struct ModelIdentity {
    name: String,
}

fn require_installed_model(endpoint: &Endpoint) -> Result<()> {
    let mut command = vec![
        "curl".into(),
        "--disable".into(),
        "--silent".into(),
        "--show-error".into(),
        "--fail".into(),
        "--noproxy".into(),
        "*".into(),
        "--proto".into(),
        "=http".into(),
        "--connect-timeout".into(),
        "2".into(),
        "--max-time".into(),
        "5".into(),
        "--max-filesize".into(),
        MAX_MODEL_INVENTORY.to_string(),
    ];
    if let Some(port) = endpoint.authority.strip_prefix("localhost:") {
        // Avoid DNS and proxy configuration: both candidates stay loopback.
        command.extend([
            "--resolve".into(),
            format!("localhost:{port}:127.0.0.1,[::1]"),
        ]);
    }
    command.extend(["--url".into(), format!("{}api/tags", endpoint.url)]);
    let output = process::checked_capture(
        &command,
        QUERY_TIMEOUT + Duration::from_secs(1),
        MAX_MODEL_INVENTORY,
    )
    .map_err(|_| {
        invalid("Cannot read the active Ollama model inventory; check Ollama and install curl")
    })?;
    let inventory: ModelInventory = serde_json::from_slice(&output.stdout)
        .map_err(|_| invalid("The active Ollama service returned an invalid model inventory"))?;
    if inventory.models.is_empty() {
        return Err(invalid(
            "No installed models in this Ollama service; import or pull a model before opening Odysseus. A model in ~/.ollama/models may belong to a different Ollama instance.",
        ));
    }
    if inventory
        .models
        .iter()
        .any(|model| model.name.trim().is_empty())
    {
        return Err(invalid(
            "The active Ollama service returned an invalid model identity",
        ));
    }
    Ok(())
}

fn start_inner(root: &Path) -> Result<()> {
    let config = configuration(root)?;
    let mut browser = apps::command(root, "browser")?;
    compose(&config, &[])?;
    if !apps::executable("docker") || !apps::executable("systemctl") || !apps::executable("curl") {
        return Err(invalid(
            "Install Docker, Docker Compose, Ollama and curl before starting local AI",
        ));
    }
    let runtime = state::xdg("XDG_RUNTIME_DIR", ".cache")?;
    let _lock = state::lock(
        &runtime.join("desktop-foundation-local-ai.lock"),
        Duration::from_secs(1),
    )?;
    ensure_service("ollama.service")?;
    ensure_service("docker.service")?;
    let rows = containers(&config)?;
    require_existing_services(&config, &rows)?;
    if rows.iter().any(|row| {
        row["state"] != "running"
            || matches!(row["health"].as_str(), Some("starting" | "unhealthy"))
    }) {
        let seconds = config.timeout.as_secs().to_string();
        let output = process::run(
            &compose(&config, &["start", "--wait", "--wait-timeout", &seconds])?,
            None,
            config.timeout + Duration::from_secs(5),
            true,
        )?;
        if !output.status.success() {
            return Err(invalid(
                "Odysseus containers did not start; inspect their lifecycle status",
            ));
        }
    }
    wait_ready(&config)?;
    require_installed_model(&config.ollama)?;
    drop(_lock);
    browser.extend(["--new-window".into(), config.browser.url]);
    process::replace(&browser)
}

/// Start existing dependencies, wait for public liveness and hand off to browser.
pub fn start(root: &Path) -> Result<()> {
    let result = start_inner(root);
    if let Err(error) = &result {
        let _ = process::run(
            &[
                "notify-send".into(),
                "--app-name=Desktop Foundation".into(),
                "--urgency=normal".into(),
                "Local AI could not start".into(),
                error.to_string(),
            ],
            None,
            Duration::from_secs(2),
            false,
        );
    }
    result
}
