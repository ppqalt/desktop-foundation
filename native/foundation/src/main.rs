use desktop_foundationctl::{
    Result, actions, apps, bluetooth, browser_policy, cache, clipboard, clipboard_transport,
    diagnostics, geode, invalid, local_ai, monitor, monitor_shader, packages, preferences, process,
    screenshot, session, shell, theme, volume,
};
use std::{
    io::{self, Read},
    path::PathBuf,
};

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| invalid("Command arguments must be valid UTF-8"))
        })
        .collect::<Result<_>>()?;
    let root = if args.first().is_some_and(|s| s == "--root") {
        if args.len() < 3 {
            return Err(invalid("--root requires a checkout and command"));
        }
        let root = PathBuf::from(&args[1]);
        if !root.is_absolute() {
            return Err(invalid("--root must be an absolute checkout path"));
        }
        args.drain(..2);
        root
    } else {
        let executable = std::env::current_exe()?;
        let checkout = executable
            .ancestors()
            .nth(5)
            .ok_or_else(|| invalid("Cannot locate backend checkout"))?;
        if checkout.join("config/application-roles.json").is_file() {
            checkout.to_path_buf()
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
        }
    };
    // Compiler arguments, including --help, belong to Slang rather than our CLI.
    if args.first().map(String::as_str) == Some("monitor")
        && args.get(1).map(String::as_str) == Some("shader-compiler")
    {
        return monitor_shader::compiler(&args[2..]);
    }
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!(
            "Foundation commands:\n  clipboard [--state DIRECTORY] init|watch|store text|store image|store --mime MIME\n  clipboard [--state DIRECTORY] copy|copy-plain|pin|unpin|delete ID\n  clipboard [--state DIRECTORY] clear|clear-all\n  apps launch [terminal|browser|files|pdf|image|text] [--check]\n  volume up|down\n  bluetooth power on|off\n  bluetooth codecs DEVICE_PATH\n  bluetooth connect|disconnect|reconnect DEVICE_PATH\n  bluetooth codec DEVICE_PATH --codec sbc|sbc_xq|aac|ldac\n  screenshot [--backend niri|hyprland] region|window|output\n  power suspend|logout|reboot|poweroff [--check]\n  actions list|plan ID|invoke ID\n  cache plan|prune\n  theme generate IMAGE\n  theme derive --source SOURCE --hash HASH (material JSON on stdin)\n  theme gtk-accent (palette JSON on stdin)\n  preferences [--state DIRECTORY] set TYPE KEY VALUE|check|prepare-restore|restore|gtk-refresh\n  wallpaper start\n  notifications start|check-owner\n  system packages [--config FILE]\n  shell call METHOD [ARGS...]\n  local-ai start|status|plan\n  geode install|status|restore [--game-dir PATH]\n  build-info\n  doctor --json\n  monitor toggle|close|status|refresh\n  browser-theme status|apply COLOR|remove"
        );
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("geode") => print_json(&geode::run(&root, &args[1..])?),
        Some("monitor") if args.len() == 2 => match args[1].as_str() {
            "toggle" => monitor::toggle(&root),
            "close" => monitor::close(&root),
            "serve" => monitor::serve(&root),
            "status" => print_json(&monitor::status(&root)?),
            "refresh" => monitor::refresh(&root),
            "prepare" => monitor::prepare(&root),
            _ => Err(invalid(
                "Expected monitor toggle, close, status, refresh or prepare",
            )),
        },
        Some("browser-theme") => match args.as_slice() {
            [_, action] if action == "status" => print_json(&browser_policy::status()?),
            [_, action, color] if action == "apply" => {
                print_json(&browser_policy::refresh(&root, color)?)
            }
            [_, action] if action == "remove" => print_json(&browser_policy::remove(&root)?),
            _ => Err(invalid(
                "Expected browser-theme status, apply COLOR or remove",
            )),
        },
        Some("build-info") if args.len() == 1 => print_json(&diagnostics::build_info()),
        Some("doctor") if args.as_slice() == ["doctor", "--json"] => {
            print_json(&diagnostics::report(&root)?)
        }
        Some("local-ai") if args.len() == 2 => match args[1].as_str() {
            "plan" => print_json(&local_ai::plan(&root)?),
            "status" => print_json(&local_ai::status(&root)?),
            "start" => local_ai::start(&root),
            _ => Err(invalid("Expected local-ai start, status or plan")),
        },
        Some("clipboard") => run_clipboard(args[1..].to_vec()),
        Some("apps") if args.get(1).map(String::as_str) == Some("launch") => {
            let check = args.iter().any(|s| s == "--check");
            // The legacy launch command accepted --personal but used the
            // selected journal profile. Preserve that routing contract.
            args.retain(|s| s != "--check" && s != "--personal");
            if !(2..=3).contains(&args.len()) {
                return Err(invalid("Expected apps launch [ROLE] [--check]"));
            }
            let role = args.get(2).map(String::as_str).unwrap_or("terminal");
            if check {
                print_json(&apps::command(&root, role)?)
            } else {
                apps::launch(&root, role)
            }
        }
        Some("cache") if args.len() == 2 => {
            let paths = cache::CachePaths::from_environment()?;
            match args[1].as_str() {
                "plan" => print_json(&cache::plan(&root, &paths)?),
                "prune" => print_json(&cache::prune(&root, &paths)?),
                _ => Err(invalid("Expected cache plan or cache prune")),
            }
        }
        Some("theme") => match args.as_slice() {
            [_, action, image] if action == "generate" => {
                print_json(&theme::generate(&root, &PathBuf::from(image))?)
            }
            [_, action, source_option, source, hash_option, digest]
                if action == "derive" && source_option == "--source" && hash_option == "--hash" =>
            {
                let mut input = Vec::new();
                io::stdin().take(64 * 1024 + 1).read_to_end(&mut input)?;
                if input.len() > 64 * 1024 {
                    return Err(invalid("Material palette JSON is too large"));
                }
                let material = serde_json::from_slice(&input)?;
                print_json(&theme::derive(&root, &material, source, digest)?)
            }
            [_, action] if action == "gtk-accent" => {
                let mut input = Vec::new();
                io::stdin().take(64 * 1024 + 1).read_to_end(&mut input)?;
                if input.len() > 64 * 1024 {
                    return Err(invalid("Palette JSON is too large"));
                }
                let palette = serde_json::from_slice(&input)?;
                print_json(&serde_json::json!({"accent": theme::gtk_accent(&palette)?}))
            }
            _ => Err(invalid(
                "Expected theme generate IMAGE, theme derive --source SOURCE --hash HASH or theme gtk-accent",
            )),
        },
        Some("preferences") => run_preferences(args[1..].to_vec()),
        Some("system") if args.get(1).map(String::as_str) == Some("packages") => {
            let config = match args.as_slice() {
                [_, _] => root.join("terminal/fastfetch/packages.jsonc"),
                [_, _, option, file] if option == "--config" => std::env::current_dir()?.join(file),
                _ => return Err(invalid("Expected system packages [--config FILE]")),
            };
            if !config.is_absolute() || !config.is_file() {
                return Err(invalid("Package config must be an existing absolute file"));
            }
            println!("{}", packages::display(&config)?);
            Ok(())
        }
        Some("shell") if args.get(1).map(String::as_str) == Some("call") => {
            shell::call(&root, &args[2..])
        }
        Some("wallpaper") if args.len() == 2 && args[1] == "start" => session::wallpaper(&root),
        Some("notifications") if args.len() == 2 => match args[1].as_str() {
            "start" => session::notifications_start(),
            "check-owner" => session::notifications_available(),
            _ => Err(invalid("Expected notifications start or check-owner")),
        },
        Some("volume") if args.len() == 2 => volume::adjust(&root, &args[1]),
        Some("screenshot") => {
            let mut backend = std::env::var("DF_COMPOSITOR").unwrap_or_else(|_| "niri".into());
            let mut action = None;
            let mut options = args[1..].iter();
            while let Some(option) = options.next() {
                if option == "--backend" {
                    backend = options
                        .next()
                        .ok_or_else(|| invalid("--backend requires niri or hyprland"))?
                        .clone();
                } else if action.replace(option.as_str()).is_some() {
                    return Err(invalid(
                        "Expected screenshot [--backend niri|hyprland] region|window|output",
                    ));
                }
            }
            let action = action.ok_or_else(|| invalid("Screenshot action required"))?;
            if !matches!(backend.as_str(), "niri" | "hyprland")
                || !matches!(action, "region" | "window" | "output")
            {
                return Err(invalid(
                    "Expected screenshot [--backend niri|hyprland] region|window|output",
                ));
            }
            screenshot::capture(&root, &backend, action)
        }
        Some("bluetooth") => {
            let result = match args[1..].as_ref() {
                [action, mode] if action == "power" && matches!(mode.as_str(), "on" | "off") => {
                    bluetooth::power(mode == "on")
                }
                [action, path] if action == "codecs" => bluetooth::codecs(path),
                [action, path]
                    if matches!(action.as_str(), "connect" | "disconnect" | "reconnect") =>
                {
                    bluetooth::action(action, path, None)
                }
                [action, path, option, codec] if action == "codec" && option == "--codec" => {
                    bluetooth::action(action, path, Some(codec))
                }
                [action, path, option] if action == "codec" && option.starts_with("--codec=") => {
                    bluetooth::action(action, path, option.strip_prefix("--codec="))
                }
                _ => Err(invalid(
                    "Expected bluetooth power on|off, codecs|connect|disconnect|reconnect DEVICE_PATH or codec DEVICE_PATH --codec sbc|sbc_xq|aac|ldac",
                )),
            };
            match result {
                Ok(value) => print_json(&value),
                Err(error) => {
                    print_json(&serde_json::json!({"success": false, "error": error.to_string()}))?;
                    Err(error)
                }
            }
        }
        Some("power") => {
            let check = args.iter().any(|s| s == "--check");
            args.retain(|s| s != "--check");
            if args.len() != 2 {
                return Err(invalid("Expected power ACTION [--check]"));
            }
            let command = actions::power_command(&root, &args[1])?;
            if check {
                print_json(&command)
            } else {
                process::replace(&command)
            }
        }
        Some("actions") if args.len() == 2 && args[1] == "list" => {
            print_json(&actions::list(&root))
        }
        Some("actions") if args.len() == 3 && args[1] == "plan" => {
            print_json(&actions::plan(&root, &args[2])?)
        }
        Some("actions") if args.len() == 3 && args[1] == "invoke" => {
            actions::invoke(&root, &args[2])
        }
        _ => Err(invalid(
            "Expected clipboard, apps launch, volume, bluetooth, screenshot, power, theme, preferences, wallpaper start, notifications, actions list|plan|invoke or cache plan|prune",
        )),
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn run_preferences(mut args: Vec<String>) -> Result<()> {
    let mut directory = preferences::directory()?;
    let mut lock_fd = None;
    for option in ["--state", "--lock-fd"] {
        if let Some(index) = args.iter().position(|argument| argument == option) {
            let value = args
                .get(index + 1)
                .ok_or_else(|| invalid(format!("{option} requires a value")))?;
            if option == "--state" {
                directory = PathBuf::from(value)
            } else {
                let fd = value
                    .parse::<i32>()
                    .map_err(|_| invalid("Invalid inherited preference lock descriptor"))?;
                if fd < 0 {
                    return Err(invalid("Invalid inherited preference lock descriptor"));
                }
                lock_fd = Some(fd);
            }
            args.drain(index..=index + 1);
        }
    }
    let result = match args.as_slice() {
        [action, kind, key, value] if action == "set" => {
            preferences::manage_setting(&directory, [kind.clone(), key.clone()], value, lock_fd)
        }
        [action] if action == "check" => preferences::check(&directory, lock_fd),
        [action] if action == "prepare-restore" => {
            preferences::prepare_restore(&directory, lock_fd)
        }
        [action] if action == "restore" => preferences::restore(&directory, lock_fd),
        [action] if action == "gtk-refresh" && lock_fd.is_none() => {
            preferences::gtk_refresh(&directory)
        }
        _ => Err(invalid(
            "Expected preferences [--state DIRECTORY] set TYPE KEY VALUE|check|prepare-restore|restore|gtk-refresh",
        )),
    }?;
    print_json(&result)
}

fn run_clipboard(mut args: Vec<String>) -> Result<()> {
    let mut directory = clipboard::directory()?;
    if let Some(i) = args.iter().position(|s| s == "--state") {
        directory = PathBuf::from(
            args.get(i + 1)
                .ok_or_else(|| invalid("--state requires a directory"))?,
        );
        args.drain(i..=i + 1);
    }
    let action = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| invalid("Clipboard action required"))?;
    if action == "store" {
        if args.len() == 3 && args[1] == "--mime" {
            if !clipboard::supported_mime(&args[2]) {
                return Err(invalid("Unsupported clipboard format"));
            }
            if std::env::var("CLIPBOARD_STATE").is_ok_and(|state| state != "data") {
                return Ok(());
            }
            let mut data = Vec::new();
            io::stdin()
                .take((clipboard::MAX_BYTES + 1) as u64)
                .read_to_end(&mut data)?;
            if data.is_empty() || data.len() > clipboard::MAX_BYTES {
                return Ok(());
            }
            return clipboard::History::open(&directory)?.store_bundle(&[clipboard::Format {
                mime: args[2].clone(),
                data,
            }]);
        }
        if args.len() != 2 {
            return Err(invalid("store requires text or image"));
        }
        let mut data = Vec::new();
        io::stdin()
            .take((clipboard::MAX_BYTES + 1) as u64)
            .read_to_end(&mut data)?;
        if clipboard::accepted(&args[1], &data)? {
            clipboard::History::open(&directory)?.store(&args[1], &data)?;
        }
        return Ok(());
    }
    if !matches!(
        (action, args.len()),
        ("init" | "watch" | "serve" | "clear" | "clear-all", 1)
            | ("copy" | "copy-plain" | "pin" | "unpin" | "delete", 2)
    ) {
        return Err(invalid(
            "Expected clipboard init, watch, copy/copy-plain/pin/unpin/delete ID, clear/clear-all or store text|image|--mime MIME",
        ));
    }
    if action == "watch" {
        return clipboard_transport::watch(&directory);
    }
    if action == "serve" {
        return clipboard_transport::serve_stdin();
    }
    let mut history = clipboard::History::open(&directory)?;
    match action {
        "init" => history.export(),
        "clear" => history.clear(),
        "clear-all" => history.clear_all(),
        "pin" => history.pin(&args[1], true),
        "unpin" => history.pin(&args[1], false),
        "delete" => history.delete(&args[1]),
        "copy" => {
            let formats = history.bundle(&args[1])?;
            drop(history); // Release storage ownership before contacting Wayland.
            clipboard_transport::publish(formats, &args[1])
        }
        "copy-plain" => {
            let data = history.plain_payload(&args[1])?;
            drop(history);
            clipboard_transport::publish(
                vec![clipboard::Format {
                    mime: "text/plain;charset=utf-8".into(),
                    data,
                }],
                &args[1],
            )
        }
        _ => Err(invalid("Unknown clipboard action")),
    }
}

fn main() {
    // SAFETY: before starting any threads; all clipboard-related files are private.
    unsafe {
        libc::umask(0o077);
    }
    if let Err(error) = run() {
        eprintln!("Foundation: {error}");
        std::process::exit(1);
    }
}
