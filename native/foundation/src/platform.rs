use crate::{Result, invalid};
use std::{fs, path::Path};
pub fn cachyos_release(text: &str) -> bool {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .any(|(key, value)| {
            key.trim() == "ID" && value.trim().trim_matches(['\'', '"']) == "cachyos"
        })
}
pub fn require_cachyos() -> Result<()> {
    if cachyos_release(&fs::read_to_string(Path::new("/etc/os-release"))?) {
        Ok(())
    } else {
        Err(invalid(
            "Desktop Foundation supports CachyOS with Niri only",
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_cachyos_is_supported() {
        assert!(cachyos_release("ID=cachyos\nID_LIKE=arch\n"));
        assert!(cachyos_release("ID=\"cachyos\"\n"));
        assert!(!cachyos_release("ID=arch\n"));
        assert!(!cachyos_release("ID=ubuntu\nID_LIKE=cachyos\n"));
    }
}
