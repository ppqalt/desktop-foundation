//! Explicit host-profile selection; privileged changes have a separate journal.
use crate::{Result, deploy, invalid, state};
use std::{fs, path::Path};

pub fn select(root: &Path, value: &str) -> Result<()> {
    if !["lucky38", "off"].contains(&value) {
        return Err(invalid("Performance profile must be lucky38 or off"));
    }
    // SAFETY: geteuid takes no pointers.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Select performance policy as your desktop user"));
    }
    let paths = deploy::Paths::environment()?;
    if let Some(manifest) = deploy::read(&paths)?
        && manifest.root != root.canonicalize()?
    {
        return Err(invalid(
            "Performance policy belongs to another deployment checkout",
        ));
    }
    state::private_directory(&paths.state)?;
    let file = paths.state.join("performance-profile");
    if file.symlink_metadata().is_ok_and(|m| !m.is_file()) {
        return Err(invalid("Unexpected performance selection path"));
    }
    if value == "off" {
        if file.try_exists()? {
            fs::remove_file(file)?;
        }
    } else {
        state::atomic(&file, b"lucky38\n", 0o600)?;
    }
    Ok(())
}
