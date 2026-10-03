use desktop_foundationctl::{Result, actions, apps, cache, clipboard, invalid, process, volume};
use std::{
    io::{self, Read},
    path::PathBuf,
    time::Duration,
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
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    };
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!(
            "Foundation commands:\n  clipboard [--state DIRECTORY] init|store text|store image|copy ID|delete ID|clear\n  apps launch [terminal|browser|files|pdf|image|text] [--check]\n  volume up|down\n  power suspend|logout|reboot|poweroff [--check]\n  actions list|plan ID|invoke ID\n  cache plan|prune"
        );
        return Ok(());
    }
    match args.first().map(String::as_str) {
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
        Some("volume") if args.len() == 2 => volume::adjust(&root, &args[1]),
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
            "Expected clipboard, apps launch, volume, power, actions list|plan|invoke or cache plan|prune",
        )),
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
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
        ("init" | "clear", 1) | ("copy" | "delete", 2)
    ) {
        return Err(invalid(
            "Expected clipboard init, clear, copy ID, delete ID or store text|image",
        ));
    }
    let mut history = clipboard::History::open(&directory)?;
    match action {
        "init" => history.export(),
        "clear" => history.clear(),
        "delete" => history.delete(&args[1]),
        "copy" => {
            let (mime, payload) = history.payload(&args[1])?;
            drop(history); // Release storage ownership before contacting Wayland.
            process::checked(
                &["wl-copy".into(), "--type".into(), mime],
                Some(payload),
                Duration::from_secs(5),
                false,
            )?;
            Ok(())
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
