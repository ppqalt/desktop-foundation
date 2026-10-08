//! An on-demand, real Kitty layer-shell panel running btop. No hidden sampler.
use crate::{Result, apps, invalid, process, state};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const UNIT: &str = "desktop-foundation-monitor.service";
const FRAME: Duration = Duration::from_millis(8);
const RPC_TIMEOUT: Duration = Duration::from_millis(250);
static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    width: u32,
    font_size: f32,
    animation_ms: u64,
    #[serde(default = "closing_animation_ms")]
    closing_animation_ms: u64,
    #[serde(default = "corner_radius")]
    corner_radius: u32,
    #[serde(default = "padding")]
    padding: u32,
    update_ms: u32,
}

fn closing_animation_ms() -> u64 {
    180
}

fn corner_radius() -> u32 {
    14
}

fn padding() -> u32 {
    12
}

struct Paths {
    directory: PathBuf,
    control: PathBuf,
    kitty: PathBuf,
}

impl Paths {
    fn new() -> Result<Self> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| invalid("The monitor requires XDG_RUNTIME_DIR"))?;
        let metadata = fs::symlink_metadata(&runtime)?;
        if !metadata.is_dir() || metadata.uid() != uid() || metadata.mode() & 0o077 != 0 {
            return Err(invalid(
                "The monitor requires a private, owned runtime directory",
            ));
        }
        let directory = runtime.join("desktop-foundation-monitor");
        if directory.exists() {
            let metadata = fs::symlink_metadata(&directory)?;
            if !metadata.is_dir() || metadata.uid() != uid() {
                return Err(invalid("The monitor runtime directory is not owned"));
            }
        }
        state::private_directory(&directory)?;
        Ok(Self {
            control: directory.join("control.sock"),
            kitty: directory.join("kitty.sock"),
            directory,
        })
    }
}

fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or retained pointers.
    unsafe { libc::geteuid() }
}

fn peer(stream: &UnixStream, pid: Option<u32>) -> Result<()> {
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: credentials is a writable ucred-sized buffer and stream is live.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if size as usize != std::mem::size_of::<libc::ucred>() {
        return Err(invalid("Invalid monitor socket credentials"));
    }
    // SAFETY: successful SO_PEERCRED initialized the supplied buffer.
    let credentials = unsafe { credentials.assume_init() };
    if credentials.uid != uid() || pid.is_some_and(|pid| credentials.pid as u32 != pid) {
        return Err(invalid("The monitor socket peer is not its owner"));
    }
    Ok(())
}

fn socket(path: &Path) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_socket() || metadata.uid() != uid() {
        return Err(invalid("The monitor socket is not an owned Unix socket"));
    }
    Ok(true)
}

fn stale(path: &Path) -> Result<()> {
    if socket(path)? {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn record(stream: &mut UnixStream, maximum: usize) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte)? {
            0 => return Err(invalid("The monitor connection closed before its response")),
            _ if byte[0] == b'\n' => return Ok(data),
            _ if data.len() >= maximum => return Err(invalid("Monitor message exceeds its limit")),
            _ => data.push(byte[0]),
        }
    }
}

fn request(paths: &Paths, action: &str) -> Result<Option<Value>> {
    if !socket(&paths.control)? {
        return Ok(None);
    }
    let mut stream = match UnixStream::connect(&paths.control) {
        Ok(stream) => stream,
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    peer(&stream, None)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(serde_json::to_string(action)?.as_bytes())?;
    stream.write_all(b"\n")?;
    let response: Value = serde_json::from_slice(&record(&mut stream, 4096)?)?;
    if response.get("ok") != Some(&json!(true)) {
        return Err(invalid("The monitor rejected its control request"));
    }
    Ok(Some(response))
}

pub fn toggle(_: &Path) -> Result<()> {
    let paths = Paths::new()?;
    let _request = state::lock(
        &paths.directory.join("request.lock"),
        Duration::from_secs(6),
    )?;
    if request(&paths, "toggle")?.is_some() {
        return Ok(());
    }
    process::checked(
        &[
            "systemctl".into(),
            "--user".into(),
            "start".into(),
            UNIT.into(),
        ],
        None,
        Duration::from_secs(5),
        true,
    )?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if request(&paths, "status")?.is_some() {
            return Ok(()); // A new service starts opening; do not toggle it closed.
        }
        if Instant::now() >= deadline {
            return Err(invalid(
                "The monitor did not become ready; inspect its user service",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn close(_: &Path) -> Result<()> {
    let paths = Paths::new()?;
    if request(&paths, "close")?.is_none() {
        // This also safely cancels a service that has not bound its socket yet.
        process::checked(
            &[
                "systemctl".into(),
                "--user".into(),
                "stop".into(),
                UNIT.into(),
            ],
            None,
            Duration::from_secs(4),
            true,
        )?;
    }
    Ok(())
}

pub fn status(_: &Path) -> Result<Value> {
    Ok(request(&Paths::new()?, "status")?.unwrap_or_else(|| json!({"ok":true,"state":"closed"})))
}

pub fn refresh(_: &Path) -> Result<()> {
    let _ = request(&Paths::new()?, "refresh")?;
    Ok(())
}

fn settings(root: &Path) -> Result<Settings> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(root.join("config/monitor.toml"))?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 4096 {
        return Err(invalid(
            "Monitor settings must be a regular file below 4 KiB",
        ));
    }
    let mut text = String::new();
    Read::by_ref(&mut file)
        .take(4097)
        .read_to_string(&mut text)?;
    if text.len() > 4096 {
        return Err(invalid("Monitor settings exceed 4 KiB"));
    }
    let settings: Settings = toml::from_str(&text)
        .map_err(|error| invalid(format!("Invalid monitor settings: {error}")))?;
    if !(480..=1600).contains(&settings.width)
        || !settings.font_size.is_finite()
        || !(8.0..=18.0).contains(&settings.font_size)
        || !(80..=400).contains(&settings.animation_ms)
        || !(80..=400).contains(&settings.closing_animation_ms)
        || settings.corner_radius > 32
        || settings.padding > 24
        || !(1000..=10000).contains(&settings.update_ms)
    {
        return Err(invalid(
            "Monitor size, font, corners, padding, animation or update interval is out of range",
        ));
    }
    Ok(settings)
}

fn niri(action: &str) -> Result<Value> {
    let output = process::checked(
        &["niri".into(), "msg".into(), "-j".into(), action.into()],
        None,
        Duration::from_secs(2),
        true,
    )?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn config_path(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .ok_or_else(|| invalid("Monitor paths must be UTF-8"))?;
    if path.contains(['\n', '\r', '\0', '$']) || path.trim() != path {
        return Err(invalid(
            "Monitor paths contain unsupported configuration characters",
        ));
    }
    // Kitty's include value is the entire remainder of the line, including
    // spaces. It does not strip shell quotes around an absolute path.
    Ok(path.to_owned())
}

fn shader(paths: &Paths, settings: &Settings, scale: f64) -> Result<String> {
    let pipeline = paths.directory.join("rounding.pipeline");
    Ok(if settings.corner_radius == 0 {
        String::new()
    } else {
        if !apps::executable("slangc") {
            return Err(invalid(
                "Rounded monitor corners require shader-slang; install it through scripts/bootstrap",
            ));
        }
        state::atomic(
            &paths.directory.join("monitor-rounding.slang"),
            include_bytes!("../../../terminal/monitor/monitor-rounding.slang"),
            0o600,
        )?;
        state::atomic(
            &pipeline,
            format!(
                "startgroup\n    animation_step 0\n    var float RADIUS = {}\n    shaders monitor-rounding\nendgroup\n",
                f64::from(settings.corner_radius) * scale,
            ).as_bytes(),
            0o600,
        )?;
        // Unlike include, custom_shaders uses shell-style tokenization.
        format!(
            "custom_shaders {}\n",
            shell_words::quote(&config_path(&pipeline)?)
        )
    })
}

fn kitty_environment(root: &Path, command: &mut Command) -> Result<()> {
    let cache = state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/monitor/kitty");
    state::private_directory(&cache)?;
    let compiler = format!(
        "{} --root {} monitor shader-compiler",
        shell_words::quote(&config_path(&std::env::current_exe()?)?),
        shell_words::quote(&config_path(root)?),
    );
    command
        .env("SLANGC", compiler)
        .env("KITTY_CACHE_DIRECTORY", cache);
    Ok(())
}

/// Prepare the shader headlessly at deployment; no panel or sampler is started.
pub fn prepare(root: &Path) -> Result<()> {
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        return Ok(()); // An offline installation can compile on its first opening.
    }
    let settings = settings(root)?;
    if settings.corner_radius == 0 {
        return Ok(());
    }
    let paths = Paths::new()?;
    // Share toggle's lock order so deployment cannot race a normal panel start.
    let _request = state::lock(
        &paths.directory.join("request.lock"),
        Duration::from_secs(6),
    )?;
    if request(&paths, "status")?.is_some() {
        return Ok(()); // The visible monitor already owns its prepared pipeline.
    }
    let _owner = match state::lock(
        &paths.directory.join("owner.lock"),
        Duration::from_millis(100),
    ) {
        Ok(owner) => owner,
        Err(crate::Error::Invalid(_)) => return Ok(()), // A direct service start owns it.
        Err(error) => return Err(error),
    };
    let scale = if std::env::var_os("NIRI_SOCKET").is_some() {
        niri("focused-output")?["logical"]["scale"]
            .as_f64()
            .filter(|scale| scale.is_finite() && (0.5..=8.0).contains(scale))
            .unwrap_or(1.0)
    } else {
        1.0
    };
    shader(&paths, &settings, scale)?;
    let mut environment = Command::new("kitty");
    kitty_environment(root, &mut environment)?;
    let mut args = vec![
        "env".into(),
        "-u".into(),
        "DISPLAY".into(),
        "-u".into(),
        "WAYLAND_DISPLAY".into(),
    ];
    for (key, value) in environment.get_envs() {
        if let Some(value) = value {
            args.push(format!(
                "{}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
    }
    args.extend([
        "kitty".into(), "+runpy".into(),
        "import sys; from kitty.shaders.slang import parse_pipeline,build_custom_shader_pipeline_glsl; build_custom_shader_pipeline_glsl(parse_pipeline(sys.argv[1]))".into(),
        config_path(&paths.directory.join("rounding.pipeline"))?,
    ]);
    process::checked(&args, None, Duration::from_secs(5), true)?;
    Ok(())
}

fn config(paths: &Paths, settings: &Settings, scale: f64) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let theme =
        state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/theme/current");
    let colors = theme.join("terminal/kitty/colors.conf");
    let themes = theme.join("btop");
    if !colors.is_file() || !themes.join("desktop-foundation.theme").is_file() {
        return Err(invalid(
            "The monitor theme is unavailable; deploy the current desktop theme",
        ));
    }
    let shaders = shader(paths, settings, scale)?;
    let kitty = paths.directory.join("kitty.conf");
    state::atomic(
        &kitty,
        format!(
            "include {}\nlinux_display_server wayland\nfont_family GoogleSansCode Nerd Font Mono\nfont_size {}\nbackground_opacity 0.90\nwindow_padding_width {}\nwindow_margin_width 0\nwindow_border_width 0px\nscrollback_lines 0\ncursor_blink_interval 0\nconfirm_os_window_close -1\nclose_on_child_death yes\nclear_all_shortcuts yes\nallow_remote_control password\nremote_control_password \"\" resize-os-window close-window load-config signal-child\n{}",
            config_path(&colors)?, settings.font_size,
            // Kitty padding uses points; Wayland uses 96 DPI at scale one.
            f64::from(settings.padding) * 0.75, shaders,
        ).as_bytes(),
        0o600,
    )?;
    let state = state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/monitor");
    state::private_directory(&state)?;
    let btop = state.join("btop.conf");
    // btop saves its own layout changes here; neither Git nor normal btop settings
    // are writable through this panel. Initial defaults are installed only once.
    if !btop.exists() {
        state::atomic(
            &btop,
            format!("color_theme = \"desktop-foundation\"\ntheme_background = false\ntruecolor = true\nrounded_corners = true\nterminal_sync = true\nshown_boxes = \"cpu mem net proc\"\nupdate_ms = {}\n", settings.update_ms).as_bytes(),
            0o600,
        )?;
    } else if !fs::symlink_metadata(&btop)?.is_file() {
        return Err(invalid(
            "The monitor btop configuration must be a regular file",
        ));
    }
    Ok((kitty, btop, themes))
}

struct OwnedPanel(Child);
impl Drop for OwnedPanel {
    fn drop(&mut self) {
        // The user unit also owns every descendant in its cgroup. KillMode=
        // control-group provides final cleanup if Kitty or btop fails to exit.
        // Allow a native btop quit/TERM to finish saving its isolated layout.
        let deadline = Instant::now() + Duration::from_millis(250);
        while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Cleanup<'a>(&'a Paths);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = stale(&self.0.control);
        let _ = stale(&self.0.kitty);
    }
}

fn rpc(paths: &Paths, pid: u32, command: &str, payload: Value) -> Result<()> {
    if !socket(&paths.kitty)? {
        return Err(invalid("The monitor terminal is not ready"));
    }
    let mut stream = UnixStream::connect(&paths.kitty)?;
    peer(&stream, Some(pid))?;
    stream.set_read_timeout(Some(RPC_TIMEOUT))?;
    stream.set_write_timeout(Some(RPC_TIMEOUT))?;
    stream.write_all(b"\x1bP@kitty-cmd")?;
    serde_json::to_writer(
        &mut stream,
        &json!({
            "cmd":command,"version":[0,49,2],"no_response":false,"payload":payload,
        }),
    )?;
    stream.write_all(b"\x1b\\")?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = Vec::new();
    Read::by_ref(&mut stream)
        .take(16385)
        .read_to_end(&mut response)?;
    if response.len() > 16384 {
        return Err(invalid("The monitor terminal response exceeds its limit"));
    }
    let response = response
        .strip_prefix(b"\x1bP@kitty-cmd")
        .and_then(|value| value.strip_suffix(b"\x1b\\"))
        .ok_or_else(|| invalid("Invalid monitor terminal response"))?;
    let response: Value = serde_json::from_slice(response)?;
    if response.get("ok") != Some(&json!(true)) {
        return Err(invalid(format!(
            "Monitor terminal: {}",
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("command failed")
        )));
    }
    Ok(())
}

fn place(paths: &Paths, pid: u32, x: i32, focus: bool) -> Result<()> {
    rpc(
        paths,
        pid,
        "resize-os-window",
        json!({
            "match":"all","action":"os-panel","incremental":true,
            "os_panel":[format!("margin-left={x}"), format!("focus-policy={}", if focus { "exclusive" } else { "not-allowed" })],
        }),
    )
}

struct Motion {
    x: f64,
    velocity: f64,
    from: f64,
    initial_velocity: f64,
    to: f64,
    shown: f64,
    hidden: f64,
    started: Instant,
    duration: Duration,
    opening_duration: Duration,
    closing_duration: Duration,
    opening: bool,
}
impl Motion {
    fn sample(&mut self) -> bool {
        let seconds = self.duration.as_secs_f64();
        let t = (self.started.elapsed().as_secs_f64() / seconds).min(1.0);
        if t >= 1.0 {
            self.x = self.to;
            self.velocity = 0.0;
            return true;
        }
        // Cubic Hermite interpolation starts from the current position and
        // velocity, and arrives at rest. Reversals brake smoothly rather than
        // restarting an ease-out curve with a discontinuous velocity.
        let distance = self.to - self.from;
        let x = self.from
            + distance * (3.0 * t * t - 2.0 * t * t * t)
            + self.initial_velocity * seconds * (t - 2.0 * t * t + t * t * t);
        self.x = x.clamp(self.shown, self.hidden);
        self.velocity = if self.x == x {
            distance * (6.0 * t - 6.0 * t * t) / seconds
                + self.initial_velocity * (1.0 - 4.0 * t + 3.0 * t * t)
        } else {
            // Repeated reversals can carry momentum toward an output edge.
            // Keep the entire motion inside its two endpoint positions.
            0.0
        };
        false
    }
    fn target(&mut self, opening: bool, shown: i32, hidden: i32) {
        self.sample();
        self.from = self.x;
        self.initial_velocity = self.velocity;
        self.to = f64::from(if opening { shown } else { hidden });
        self.duration = if opening {
            self.opening_duration
        } else {
            self.closing_duration
        };
        let distance = self.to - self.from;
        let speed = self.initial_velocity.abs();
        if speed > 0.0 {
            let room = if self.initial_velocity > 0.0 {
                self.hidden - self.from
            } else {
                self.from - self.shown
            };
            if room <= 0.0 || distance == 0.0 {
                // An outward velocity at an endpoint cannot be retained.
                self.initial_velocity = 0.0;
                self.velocity = 0.0;
            } else {
                let maximum_seconds = if self.initial_velocity * distance < 0.0 {
                    // The wrong-way Hermite term peaks at 4/27 * speed * T.
                    27.0 / 4.0 * room / speed
                } else {
                    // A normalized initial slope at most 3 stays monotonic.
                    3.0 * distance.abs() / speed
                };
                let duration =
                    Duration::from_secs_f64(self.duration.as_secs_f64().min(maximum_seconds));
                if duration.is_zero() {
                    // Sub-nanosecond braking cannot be represented by Instant.
                    self.initial_velocity = 0.0;
                    self.velocity = 0.0;
                } else {
                    self.duration = duration;
                }
            }
        }
        self.started = Instant::now();
        self.opening = opening;
    }
    fn state(&self, settled: bool) -> &'static str {
        match (settled, self.opening) {
            (true, true) => "open",
            (true, false) => "closed",
            (false, true) => "opening",
            (false, false) => "closing",
        }
    }
}

pub fn serve(root: &Path) -> Result<()> {
    let initializing = Instant::now();
    let cgroup = fs::read_to_string("/proc/self/cgroup")?;
    if std::env::var_os("INVOCATION_ID").is_none()
        || !cgroup.lines().any(|line| {
            line.rsplit_once(':')
                .is_some_and(|(_, path)| path.ends_with(&format!("/{UNIT}")))
        })
    {
        return Err(invalid(
            "Start the monitor through its user service so child cleanup is owned",
        ));
    }
    let paths = Paths::new()?;
    let _owner = state::lock(
        &paths.directory.join("owner.lock"),
        Duration::from_millis(100),
    )?;
    stale(&paths.control)?;
    stale(&paths.kitty)?;
    let _cleanup = Cleanup(&paths);
    let settings = settings(root)?;
    let output = niri("focused-output")?;
    let name = output
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("No focused output for the monitor"))?;
    let geometry = output
        .get("logical")
        .ok_or_else(|| invalid("The monitor output has no logical geometry"))?;
    let width = geometry
        .get("width")
        .and_then(Value::as_u64)
        .filter(|width| (520..=32768).contains(width))
        .ok_or_else(|| invalid("The output is too narrow for the monitor"))? as u32;
    let height = geometry
        .get("height")
        .and_then(Value::as_u64)
        .filter(|height| (480..=32768).contains(height))
        .ok_or_else(|| invalid("The output is too short for the monitor"))? as u32;
    let pane_width = settings.width.min(width - 26);
    let scale = geometry
        .get("scale")
        .and_then(Value::as_f64)
        .filter(|scale| scale.is_finite() && (0.5..=8.0).contains(scale))
        .ok_or_else(|| invalid("The monitor output has an invalid scale"))?;
    // Kitty adds scaled edge spacing and one sizing pixel to pixel requests.
    // Keep the outer surface full-height and the TUI clear of rounded corners.
    let spacing = 2.0 * f64::from(settings.padding) * scale;
    let content_size = |outer: u32| (f64::from(outer) - spacing - 1.0).ceil().max(1.0) as u32;
    let hidden = width as i32 + 12;
    let shown = (width - pane_width) as i32;
    let (kitty_config, btop_config, themes) = config(&paths, &settings, scale)?;
    let listener = UnixListener::bind(&paths.control)?;
    fs::set_permissions(&paths.control, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let mut events = UnixStream::connect(
        std::env::var_os("NIRI_SOCKET")
            .ok_or_else(|| invalid("The monitor requires a Niri session"))?,
    )?;
    peer(&events, None)?;
    events.write_all(b"\"EventStream\"\n")?;
    events.set_nonblocking(true)?;
    let mut event_buffer = Vec::new();
    STOP.store(false, Ordering::Relaxed);
    // SAFETY: the handlers perform only an atomic store and retain no pointers.
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, stop as *const () as libc::sighandler_t);
    }
    let mut kitty = Command::new("kitty");
    kitty_environment(root, &mut kitty)?;
    let mut panel = OwnedPanel(
        kitty
            .args([
                "+kitten",
                "panel",
                "--edge=none",
                "--layer=overlay",
                "--exclusive-zone=-1",
                "--focus-policy=not-allowed",
                "--app-id=desktop-foundation:monitor",
            ])
            .arg(format!("--columns={}px", content_size(pane_width)))
            .arg(format!("--lines={}px", content_size(height)))
            .arg("--margin-top=0")
            .arg(format!("--margin-left={hidden}"))
            .arg(format!("--output-name={name}"))
            .arg(format!("--listen-on=unix:{}", paths.kitty.display()))
            .arg("--config")
            .arg(&kitty_config)
            .arg("btop")
            .arg("--config")
            .arg(&btop_config)
            .arg("--themes-dir")
            .arg(&themes)
            .arg("--no-tty")
            .arg("--update")
            .arg(settings.update_ms.to_string())
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    // SAFETY: pidfd_open receives the PID of our own newly spawned child.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, panel.0.id(), 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful pidfd_open returned a fresh descriptor.
    let pidfd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    let mut motion = Motion {
        x: f64::from(hidden),
        velocity: 0.0,
        from: f64::from(hidden),
        initial_velocity: 0.0,
        to: f64::from(shown),
        shown: f64::from(shown),
        hidden: f64::from(hidden),
        started: Instant::now(),
        duration: Duration::from_millis(settings.animation_ms),
        opening_duration: Duration::from_millis(settings.animation_ms),
        closing_duration: Duration::from_millis(settings.closing_animation_ms),
        opening: true,
    };
    let startup = Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    let mut settled = false;
    let mut last_x = hidden;
    loop {
        if STOP.load(Ordering::Relaxed) {
            return Ok(());
        }
        let timeout = if ready && settled {
            -1
        } else {
            FRAME.as_millis() as i32
        };
        let mut descriptors = [
            libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: events.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: descriptors is a writable array of pollfd records.
        let result = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                descriptors.len() as libc::nfds_t,
                timeout,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if descriptors[1].revents != 0 || panel.0.try_wait()?.is_some() {
            return Ok(());
        }
        if descriptors[2].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            return Ok(());
        }
        if descriptors[2].revents & libc::POLLIN != 0 {
            let mut buffer = [0_u8; 8192];
            loop {
                match events.read(&mut buffer) {
                    Ok(0) => return Ok(()),
                    Ok(count) => event_buffer.extend_from_slice(&buffer[..count]),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error.into()),
                }
                if event_buffer.len() > 2 * 1024 * 1024 {
                    return Err(invalid("Niri event exceeded its limit"));
                }
            }
            while let Some(end) = event_buffer.iter().position(|byte| *byte == b'\n') {
                // All other events are discarded without reading window titles or
                // browser contents. Workspace changes include output removal.
                if event_buffer[..end].starts_with(b"{\"WorkspacesChanged\":") {
                    let outputs = niri("outputs")?;
                    if outputs.get(name).and_then(|output| output.get("logical")) != Some(geometry)
                    {
                        return Ok(()); // Reopen on the new geometry/output.
                    }
                }
                event_buffer.drain(..=end);
            }
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            for _ in 0..8 {
                let (mut client, _) = match listener.accept() {
                    Ok(client) => client,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error.into()),
                };
                if peer(&client, None).is_err() {
                    continue;
                }
                client.set_read_timeout(Some(Duration::from_millis(30)))?;
                client.set_write_timeout(Some(Duration::from_millis(30)))?;
                let action = record(&mut client, 64)
                    .ok()
                    .and_then(|data| serde_json::from_slice::<String>(&data).ok());
                let accepted = match action.as_deref() {
                    Some("toggle") | Some("close") => {
                        let opening = action.as_deref() == Some("toggle") && !motion.opening;
                        if ready {
                            motion.target(opening, shown, hidden);
                        } else {
                            motion.opening = opening;
                            motion.to = f64::from(if opening { shown } else { hidden });
                        }
                        settled = false;
                        if ready {
                            place(&paths, panel.0.id(), motion.x.round() as i32, false)?;
                            last_x = motion.x.round() as i32;
                        }
                        true
                    }
                    Some("status") => true,
                    Some("refresh") if ready => {
                        rpc(
                            &paths,
                            panel.0.id(),
                            "load-config",
                            json!({"paths":[],"override":[],"ignore_overrides":false}),
                        )?;
                        rpc(
                            &paths,
                            panel.0.id(),
                            "signal-child",
                            json!({"match":"all","signals":["SIGUSR2"]}),
                        )?;
                        true
                    }
                    Some("refresh") => true,
                    _ => false,
                };
                let response = json!({"ok":accepted,"state":motion.state(settled),"output":name});
                let _ = writeln!(client, "{response}");
            }
        }
        if !ready {
            if !motion.opening {
                return Ok(());
            }
            if socket(&paths.kitty)? {
                place(&paths, panel.0.id(), hidden, false)?;
                ready = true;
                eprintln!(
                    "Monitor terminal ready in {} ms",
                    initializing.elapsed().as_millis()
                );
                motion.started = Instant::now();
            } else if Instant::now() >= startup {
                return Err(invalid(
                    "The monitor terminal did not initialize within five seconds",
                ));
            }
            continue;
        }
        if !settled {
            settled = motion.sample();
            let x = motion.x.round() as i32;
            if x != last_x || settled {
                place(&paths, panel.0.id(), x, settled && motion.opening)?;
                last_x = x;
            }
            if settled && !motion.opening {
                // Let btop clean up before the service reaps its cgroup.
                let _ = rpc(
                    &paths,
                    panel.0.id(),
                    "signal-child",
                    json!({"match":"all","signals":["SIGTERM"]}),
                );
                let _ = rpc(
                    &paths,
                    panel.0.id(),
                    "close-window",
                    json!({"match":"all","ignore_no_match":true}),
                );
                return Ok(());
            }
        }
    }
}
