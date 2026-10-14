#[path = "src/source_identity.rs"]
mod source_identity;

use std::{env, path::Path, process::Command};

fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

fn main() {
    let manifest =
        std::path::PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let root = manifest.join("../..");
    let (files, directories) = source_identity::inputs(&manifest).expect("native source inputs");
    // Directory watches also notice new Rust modules. Individual files cover
    // edits on filesystems that do not update the containing directory's mtime.
    for input in files.into_iter().chain(directories) {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    println!("cargo:rerun-if-changed={}", root.join(".git").display());
    for name in ["HEAD", "index", "packed-refs", "refs"] {
        if let Some(path) = git(&root, &["rev-parse", "--git-path", name]) {
            let path = std::path::PathBuf::from(path.trim());
            let path = if path.is_absolute() {
                path
            } else {
                root.join(path)
            };
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    if let Some(reference) = git(&root, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&root, &["rev-parse", "--git-path", reference.trim()])
    {
        let path = std::path::PathBuf::from(path.trim());
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let revision = git(&root, &["rev-parse", "--verify", "HEAD"])
        .filter(|revision| {
            revision.trim().len() == 40
                && revision.trim().bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .map(|revision| revision.trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let dirty = git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
            "--",
            "native/foundation",
        ],
    )
    .map(|status| {
        if status.trim().is_empty() {
            "false"
        } else {
            "true"
        }
    })
    .unwrap_or("unknown");
    println!("cargo:rustc-env=FOUNDATION_BUILD_REVISION={revision}");
    println!("cargo:rustc-env=FOUNDATION_BUILD_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=FOUNDATION_SOURCE_DIGEST={}",
        source_identity::digest(&manifest).expect("native source digest")
    );
}
