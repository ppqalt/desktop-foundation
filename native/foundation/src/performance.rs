//! Explicit host-profile selection; privileged changes have a separate journal.
use crate::{Result, deploy, invalid, state};
use std::{fs, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

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
    state::atomic(&file, format!("{value}\n").as_bytes(), 0o600)?;
    Ok(())
}

/// Driver-independent detection. Callers use `/`; the root parameter permits
/// isolated filesystem fixtures without a production environment-variable bypass.
pub fn detect_in(root: &Path) -> bool {
    let read = |path: &str| fs::read_to_string(root.join(path)).ok();
    if read("sys/class/dmi/id/board_vendor")
        .as_deref()
        .map(str::trim)
        != Some("ASUSTeK COMPUTER INC.")
        || read("sys/class/dmi/id/board_name")
            .as_deref()
            .map(str::trim)
            != Some("TUF B450-PRO GAMING")
    {
        return false;
    }
    let Some(cpu) = read("proc/cpuinfo") else {
        return false;
    };
    let models: Vec<_> = cpu
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.trim() == "model name")
        .map(|(_, value)| value.trim())
        .collect();
    if models.is_empty()
        || models
            .iter()
            .any(|model| *model != "AMD Ryzen 7 5700X3D 8-Core Processor")
    {
        return false;
    }
    let Ok(devices) = fs::read_dir(root.join("sys/bus/pci/devices")) else {
        return false;
    };
    let mut displays = Vec::new();
    for device in devices {
        let Ok(device) = device else {
            return false;
        };
        let Ok(class) = fs::read_to_string(device.path().join("class")) else {
            return false;
        };
        if class.trim().starts_with("0x03") {
            let (Ok(vendor), Ok(id)) = (
                fs::read_to_string(device.path().join("vendor")),
                fs::read_to_string(device.path().join("device")),
            ) else {
                return false;
            };
            displays.push((vendor.trim().to_owned(), id.trim().to_owned()));
        }
    }
    displays == [("0x10de".into(), "0x2487".into())]
}

pub fn selection(directory: &Path) -> Result<Option<String>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(directory.join("performance-profile"))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(invalid("Performance selection must be a regular file"));
    }
    let mut value = String::new();
    file.take(33).read_to_string(&mut value)?;
    if value.len() > 32 || !["lucky38", "off"].contains(&value.trim()) {
        return Err(invalid(
            "Unknown saved performance profile; selection preserved",
        ));
    }
    Ok(Some(value.trim().into()))
}
