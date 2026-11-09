//! Small line-mode terminal picker: no raw terminal state or new dependencies.
use super::*;
use std::io::{self, IsTerminal};

pub(super) enum Choice {
    Local,
    Remote(String, Option<String>),
}

struct Metadata(PathBuf);
impl Drop for Metadata {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn menu(title: &str, subtitle: &str, entries: &[String], local: bool) -> Result<Option<usize>> {
    let colored = io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none();
    let accent = if colored { "\x1b[1;36m" } else { "" };
    let reset = if colored { "\x1b[0m" } else { "" };
    let mut page = 0;
    loop {
        println!("\n{accent}┌─ Desktop Foundation ─────────────────────────────────────┐{reset}");
        println!("  {title}\n  {subtitle}\n");
        if local {
            println!("    0  Rebuild from local files (no GitHub fetch)");
        }
        for (index, entry) in entries.iter().enumerate().skip(page * 12).take(12) {
            let label: String = entry.chars().take(52).collect();
            println!("  {accent}{:>3}{reset}  {label}", index + 1);
        }
        println!(
            "\n  Page {}/{} · n next · p previous · q cancel",
            page + 1,
            entries.len().div_ceil(12).max(1)
        );
        print!("{accent}└─ Choose a number, then Enter: {reset}");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            return Ok(None);
        }
        match line.trim() {
            "0" if local => return Ok(Some(usize::MAX)),
            "q" | "Q" => return Ok(None),
            "n" if (page + 1) * 12 < entries.len() => page += 1,
            "p" if page > 0 => page -= 1,
            value => {
                if let Ok(index) = value.parse::<usize>()
                    && index > 0
                    && index <= entries.len()
                {
                    return Ok(Some(index - 1));
                }
                println!("  Choose a listed number, or q to cancel.");
            }
        }
    }
}
pub(super) fn choose(root: &Path, source: &str) -> Result<Option<Choice>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(invalid(
            "Interactive selection needs a terminal; use --branch NAME [--version TAG] instead",
        ));
    }
    println!(
        "Installed: {} on {}",
        &checkout_revision(root)?[..12],
        current_branch(root)?
    );
    println!("Reading branches from GitHub…");
    let output = process::checked_capture(
        &git(root, &["ls-remote", "--heads", source]),
        Duration::from_secs(120),
        1024 * 1024,
    );
    let rows = match &output {
        Ok(output) => {
            std::str::from_utf8(&output.stdout).map_err(|_| invalid("Invalid branch metadata"))?
        }
        Err(_) => {
            println!(
                "GitHub branches unavailable; local rebuild is still available (or use --local directly)."
            );
            ""
        }
    };
    let mut branches: Vec<String> = rows
        .lines()
        .filter_map(|line| {
            line.split_once('\t')?
                .1
                .strip_prefix("refs/heads/")
                .map(str::to_owned)
        })
        .collect();
    branches.sort_by_key(|name| (name != "main", name.clone()));
    branches.dedup();
    let Some(index) = menu(
        "1 / 2  Branch",
        "Choose the GitHub source branch. Local work stays protected.",
        &branches,
        true,
    )?
    else {
        return Ok(None);
    };
    if index == usize::MAX {
        println!("Using local checkout including edits; GitHub source will not be fetched.");
        return Ok(Some(Choice::Local));
    }
    let branch = branches[index].clone();
    println!("Reading release history for {branch}…");
    let cache = state::xdg("XDG_CACHE_HOME", ".cache")?.join("desktop-foundation/update");
    state::private_directory(&cache)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Invalid system clock"))?
        .as_nanos();
    let path = cache.join(format!("picker-{}-{nonce}", std::process::id()));
    fs::create_dir(&path)?;
    let metadata = Metadata(path);
    process::checked(&git(&metadata.0, &["init", "--bare"]), None, SHORT, true)?;
    let remote = if source == "origin" {
        text(&git(root, &["config", "--get", "remote.origin.url"]))?
    } else {
        source.to_owned()
    };
    let refspec = format!("+refs/heads/{branch}:refs/heads/selected");
    process::checked(
        &git(
            &metadata.0,
            &[
                "fetch",
                "--filter=blob:none",
                "--no-tags",
                &remote,
                &refspec,
                "+refs/tags/*:refs/tags/*",
            ],
        ),
        None,
        Duration::from_secs(180),
        true,
    )?;
    let tags = text(&git(
        &metadata.0,
        &[
            "tag",
            "--merged",
            "refs/heads/selected",
            "--sort=-version:refname",
        ],
    ))?;
    let releases: Vec<String> = tags.lines().map(str::to_owned).collect();
    let mut entries = vec![format!("Latest source on {branch}")];
    entries.extend(releases.iter().map(|tag| format!("Release tag  {tag}")));
    let subtitle = format!("{branch} · tags in its history; older versions may require rollback.");
    let Some(index) = menu("2 / 2  Version", &subtitle, &entries, false)? else {
        return Ok(None);
    };
    let version = index.checked_sub(1).map(|i| releases[i].clone());
    println!(
        "\n  Source: {branch} / {}",
        version.as_deref().unwrap_or("latest")
    );
    println!("  Build locally, validate, then apply. Your local branch and edits stay protected.");
    print!("  Continue? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        return Ok(None);
    }
    Ok(Some(Choice::Remote(branch, version)))
}
