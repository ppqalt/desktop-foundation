//! Atomic immutable theme revisions, explicit finite reloads and retained rollback.
use crate::{
    Result, backdrop, browser_policy, cache, invalid, monitor, preferences, process, shell,
    spotify_theme, state, theme, theme_render as render,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const RELOAD: Duration = Duration::from_secs(15);
#[derive(Deserialize, Serialize)]
pub struct Wallpaper {
    pub image: PathBuf,
    #[serde(default = "default_mode")]
    pub mode: String,
}
fn default_mode() -> String {
    "fill".into()
}
pub fn directory() -> Result<PathBuf> {
    Ok(state::xdg("XDG_STATE_HOME", ".local/state")?.join("desktop-foundation/theme"))
}
fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let suffix = Sha256::digest(
        format!(
            "{}:{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
        .as_bytes(),
    );
    format!("{nanos}-{}", &format!("{suffix:x}")[..8])
}
pub fn current(home: &Path) -> Result<Option<PathBuf>> {
    match fs::read_link(home.join("current")) {
        Ok(path) => {
            let path = if path.is_absolute() {
                path
            } else {
                home.join(path)
            };
            Ok(Some(path.canonicalize()?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn expand(path: &Path) -> Result<PathBuf> {
    if path == Path::new("~") {
        return state::home();
    }
    if let Ok(rest) = path.strip_prefix("~/") {
        return Ok(state::home()?.join(rest));
    }
    Ok(path.into())
}
pub fn config(root: &Path, home: &Path) -> Result<Wallpaper> {
    let active = home.join("current/wallpaper.toml");
    let runtime = active.is_file();
    let path = if runtime {
        active
    } else {
        root.join("compositor/niri/wallpaper.toml")
    };
    let mut config: Wallpaper =
        toml::from_str(&fs::read_to_string(path)?).map_err(|e| invalid(e.to_string()))?;
    if !["fill", "fit", "center", "tile"].contains(&config.mode.as_str()) {
        return Err(invalid("Invalid wallpaper mode"));
    }
    config.image = expand(&config.image)?;
    if !config.image.is_file() && !runtime {
        config.image = root.join("wallpapers").join(
            config
                .image
                .file_name()
                .ok_or_else(|| invalid("Missing wallpaper filename"))?,
        );
    }
    Ok(config)
}
fn profile(home: &Path) -> Result<String> {
    let path = home.join("profile.json");
    if path.is_file() {
        Ok(render::read(&path)?["profile"]
            .as_str()
            .ok_or_else(|| invalid("Missing theme profile"))?
            .into())
    } else {
        Ok("default".into())
    }
}
fn check_owner(root: &Path, target: &Path) -> Result<()> {
    if render::read(&target.join("metadata.json"))?["root"] != root.to_string_lossy().as_ref() {
        return Err(invalid("Another checkout owns runtime theme state"));
    }
    Ok(())
}
fn link(home: &Path, name: &str, target: &Path) -> Result<()> {
    if !["current", "previous"].contains(&name) {
        return Err(invalid("Invalid theme link"));
    }
    let temporary = home.join(format!(".{name}-{}", nonce()));
    symlink(target, &temporary)?;
    if let Err(error) = fs::rename(&temporary, home.join(name)) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    File::open(home)?.sync_all()?;
    Ok(())
}
fn sync_tree(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            sync_tree(&path)?;
        } else {
            File::open(path)?.sync_all()?;
        }
    }
    File::open(path)?.sync_all()?;
    Ok(())
}
pub fn stage(
    root: &Path,
    home: &Path,
    p: &Value,
    image: &Path,
    mode: &str,
    reset: bool,
    selected: &str,
) -> Result<PathBuf> {
    if !["fill", "fit", "center", "tile"].contains(&mode) {
        return Err(invalid("Invalid wallpaper scaling mode"));
    }
    let image = expand(image)?.canonicalize()?;
    let revisions = home.join("revisions");
    state::private_directory(&revisions)?;
    let target = revisions.join(nonce());
    fs::create_dir(&target)?;
    let result = (|| {
        let extension = image
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("image")
            .to_ascii_lowercase();
        if !extension.bytes().all(|c| c.is_ascii_alphanumeric()) {
            return Err(invalid("Invalid wallpaper extension"));
        }
        let copied = target.join(format!("wallpaper.{extension}"));
        fs::copy(&image, &copied)?;
        let digest = backdrop::hash(&copied)?;
        if backdrop::hash(&image)? != digest {
            return Err(invalid("Wallpaper changed during staging"));
        }
        let overview = backdrop::prepare(&copied, mode)?;
        fs::copy(overview, target.join("overview.png"))?;
        render::render(root, &target, p, selected, reset)?;
        render::write(
            &target.join("wallpaper.toml"),
            &format!(
                "image = {}\nmode = {}\n",
                serde_json::to_string(&copied)?,
                serde_json::to_string(mode)?
            ),
        )?;
        render::write_json(
            &target.join("backdrop.json"),
            &json!({"image":state::file_uri(&target.join("overview.png"))?,"mode":mode}),
        )?;
        render::write_json(
            &target.join("metadata.json"),
            &json!({"root":root,"input":image,"wallpaperHash":digest,"profile":selected,"reset":reset,"renderer":"rust-v1"}),
        )?;
        process::checked(
            &[
                "niri".into(),
                "validate".into(),
                "-c".into(),
                target.join("niri.kdl").to_string_lossy().into_owned(),
            ],
            None,
            RELOAD,
            true,
        )?;
        process::checked(
            &[
                "fish".into(),
                "--no-config".into(),
                "-n".into(),
                target
                    .join("terminal/fish/theme.fish")
                    .to_string_lossy()
                    .into_owned(),
            ],
            None,
            RELOAD,
            true,
        )?;
        sync_tree(&target)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&target);
        return Err(error);
    }
    Ok(target)
}
fn command(args: &[&str]) -> Result<()> {
    process::checked(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        None,
        RELOAD,
        true,
    )?;
    Ok(())
}
fn reload(root: &Path, home: &Path, wallpaper: bool) -> Result<()> {
    let p = render::read(&home.join("current/semantic.json"))?;
    let policy = browser_policy::refresh(root, render::role(&p, "elevated")?)?;
    if env::var_os("NIRI_SOCKET").is_some_and(|v| !v.is_empty()) {
        if wallpaper {
            command(&[
                "systemctl",
                "--user",
                "reset-failed",
                "desktop-foundation-wallpaper.service",
            ])?;
            command(&[
                "systemctl",
                "--user",
                "restart",
                "desktop-foundation-wallpaper.service",
            ])?;
        }
        command(&["niri", "msg", "action", "load-config-file"])?;
        shell::call(root, &["reloadTheme".into()])?;
        monitor::refresh(root)?;
    }
    if crate::apps::executable("makoctl")
        && let Err(e) = command(&["makoctl", "reload"])
    {
        eprintln!("Notification colors apply at next start: {e}");
    }
    // Only signal this user's Kitty processes, as in the prior implementation.
    for proc in fs::read_dir("/proc")? {
        let proc = proc?;
        let name = proc.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|p| *p > 0)
        else {
            continue;
        };
        if proc
            .metadata()
            .is_ok_and(|m| m.uid() == unsafe { libc::getuid() })
            && fs::read_to_string(proc.path().join("comm")).is_ok_and(|s| s.trim() == "kitty")
        {
            unsafe {
                libc::kill(pid, libc::SIGUSR1);
            }
        }
    }
    preferences::gtk_refresh(&preferences::directory()?)?;
    spotify_theme::run(root, "refresh")?;
    if policy["enabled"] != true && home.join("brave-devtools.json").is_file() {
        process::checked(
            &[root
                .join("scripts/brave-theme-reload")
                .to_string_lossy()
                .into_owned()],
            None,
            Duration::from_secs(30),
            true,
        )?;
    }
    Ok(())
}
/// Publication is factored from reload so failure/recovery can be verified
/// without contacting any real compositor, browser or service.
pub fn publish_with(
    root: &Path,
    home: &Path,
    target: &Path,
    mut refresh: impl FnMut() -> Result<()>,
) -> Result<()> {
    check_owner(root, target)?;
    let old = current(home)?;
    if let Some(old) = &old {
        check_owner(root, old)?;
    }
    let manifest = fs::read_to_string(target.join("brave/manifest.json"))?;
    let stable = home.join("brave/manifest.json");
    state::private_directory(stable.parent().expect("manifest parent"))?;
    link(home, "current", target)?;
    if let Err(error) = render::write(&stable, &manifest).and_then(|()| refresh()) {
        if let Some(old) = &old {
            link(home, "current", old)?;
            render::write(
                &stable,
                &fs::read_to_string(old.join("brave/manifest.json"))?,
            )?;
            if let Err(error) = refresh() {
                eprintln!("Rollback published; reload needs attention: {error}");
            }
        } else {
            fs::remove_file(home.join("current"))?;
            if stable.exists() {
                fs::remove_file(&stable)?;
            }
        }
        return Err(error);
    }
    if let Some(old) = old {
        link(home, "previous", &old)?;
    }
    Ok(())
}
fn publish(root: &Path, home: &Path, target: &Path, wallpaper: bool, live: bool) -> Result<()> {
    publish_with(root, home, target, || {
        if live {
            reload(root, home, wallpaper)
        } else {
            Ok(())
        }
    })
}
fn retention(root: &Path) {
    if let Err(error) =
        cache::CachePaths::from_environment().and_then(|paths| cache::prune(root, &paths))
    {
        eprintln!("Theme cache maintenance deferred: {error}");
    }
}
pub fn ensure(root: &Path, selected: &str) -> Result<()> {
    let home = directory()?;
    state::private_directory(home.parent().expect("theme parent"))?;
    {
        let _lock = state::lock(
            &home.parent().expect("theme parent").join("theme.lock"),
            Duration::from_secs(120),
        )?;
        state::private_directory(&home)?;
        let c = config(root, &home)?;
        let old = current(&home)?;
        let (p, reset) = if let Some(old) = old {
            check_owner(root, &old)?;
            (
                render::read(&old.join("semantic.json"))?,
                render::read(&old.join("metadata.json"))?["reset"] == true,
            )
        } else {
            (render::read(&root.join("theme/generated.json"))?, false)
        };
        let target = stage(root, &home, &p, &c.image, &c.mode, reset, selected)?;
        publish(root, &home, &target, false, false)?;
        render::write_json(&home.join("profile.json"), &json!({"profile":selected}))?;
    }
    retention(root);
    Ok(())
}
fn equal_palette(a: &Value, b: &Value) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    if let Some(a) = a.as_object_mut() {
        a.remove("source");
    }
    if let Some(b) = b.as_object_mut() {
        b.remove("source");
    }
    a == b
}
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err(invalid(
            "Expected generate|apply|reset|wallpaper|rollback|promote|ensure",
        ));
    };
    if action == "ensure" {
        if args.len() > 2 {
            return Err(invalid("Expected ensure [PROFILE]"));
        }
        return ensure(root, args.get(1).map(String::as_str).unwrap_or("default"));
    }
    if ![
        "generate",
        "apply",
        "reset",
        "wallpaper",
        "rollback",
        "promote",
    ]
    .contains(&action)
    {
        return Err(invalid("Unknown theme action"));
    }
    let mut image = None;
    let mut mode = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--mode" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| invalid("Missing wallpaper mode"))?;
            if mode.is_some() || !["fill", "fit", "center", "tile"].contains(&value.as_str()) {
                return Err(invalid("Invalid wallpaper mode"));
            }
            mode = Some(value.clone());
        } else if args[index].starts_with('-') || image.is_some() {
            return Err(invalid("Unexpected theme argument"));
        } else {
            image = Some(PathBuf::from(&args[index]));
        }
        index += 1;
    }
    if action == "wallpaper" && image.is_none() {
        return Err(invalid("wallpaper-set requires an image path"));
    }
    if ["rollback", "promote"].contains(&action) && (image.is_some() || mode.is_some()) {
        return Err(invalid("Unexpected theme argument"));
    }
    let started = Instant::now();
    let home = directory()?;
    state::private_directory(home.parent().expect("theme parent"))?;
    let mut cached = false;
    {
        let _lock = state::lock(
            &home.parent().expect("theme parent").join("theme.lock"),
            Duration::from_secs(120),
        )?;
        state::private_directory(&home)?;
        if action == "rollback" {
            let target = home
                .join("previous")
                .canonicalize()
                .map_err(|_| invalid("No previous runtime theme exists"))?;
            return publish(root, &home, &target, true, true);
        }
        if action == "promote" {
            let target = current(&home)?.ok_or_else(|| invalid("No runtime palette"))?;
            let mut p = render::read(&target.join("semantic.json"))?;
            p["source"] = json!("promoted-runtime-palette");
            return render::write_json(&root.join("theme/generated.json"), &p);
        }
        let c = config(root, &home)?;
        let chosen = expand(image.as_deref().unwrap_or(&c.image))?.canonicalize()?;
        let p = if action == "reset" {
            render::read(&root.join("theme/fallback/semantic.json"))?
        } else {
            let generated = theme::generate(root, &chosen)?;
            cached = generated.cached;
            generated.palette
        };
        if action == "generate" {
            println!("{}", serde_json::to_string_pretty(&p)?);
            return Ok(());
        }
        let wallpaper = action == "wallpaper";
        // theme-apply IMAGE derives that image's colors while retaining the selected
        // wallpaper, matching the existing command's behavior.
        let selected_image = if wallpaper { chosen } else { c.image.clone() };
        let selected_mode = if wallpaper {
            mode.as_deref().unwrap_or(&c.mode)
        } else {
            &c.mode
        };
        let old = current(&home)?;
        let unchanged = if let Some(old) = &old {
            check_owner(root, old)?;
            equal_palette(&render::read(&old.join("semantic.json"))?, &p)
                && render::read(&old.join("metadata.json"))?["wallpaperHash"]
                    == backdrop::hash(&selected_image)?
                && selected_mode == c.mode
                && render::read(&old.join("metadata.json"))?["renderer"] == "rust-v1"
        } else {
            false
        };
        if unchanged {
            browser_policy::refresh(root, render::role(&p, "elevated")?)?;
        } else {
            let target = stage(
                root,
                &home,
                &p,
                &selected_image,
                selected_mode,
                action == "reset",
                &profile(&home)?,
            )?;
            if wallpaper
                && render::read(&target.join("metadata.json"))?["wallpaperHash"]
                    != p["wallpaperHash"]
            {
                fs::remove_dir_all(&target)?;
                return Err(invalid("Wallpaper changed after palette generation"));
            }
            publish(root, &home, &target, wallpaper, true)?;
        }
    }
    retention(root);
    eprintln!(
        "{action}: {:.3}s; palette cache hit={cached}",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
