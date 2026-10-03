use desktop_foundationctl::{Result, clipboard, invalid, process};
use std::{
    io::{self, Read},
    path::PathBuf,
    time::Duration,
};

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--root") {
        if args.len() < 3 {
            return Err(invalid("--root requires a checkout and command"));
        }
        args.drain(..2);
    }
    if args.first().map(String::as_str) != Some("clipboard") {
        return Err(invalid(
            "Usage: desktop-foundationctl clipboard [--state DIRECTORY] init|store|copy|delete|clear",
        ));
    }
    args.remove(0);
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
