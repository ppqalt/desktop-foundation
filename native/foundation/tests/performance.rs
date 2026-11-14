//! Driver-free hardware fixtures; never inspect or modify the real machine.
use desktop_foundationctl::performance;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
            "df-hardware-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        f.write("sys/class/dmi/id/board_vendor", "ASUSTeK COMPUTER INC.\n");
        f.write("sys/class/dmi/id/board_name", "TUF B450-PRO GAMING\n");
        f.write(
            "proc/cpuinfo",
            "model name : AMD Ryzen 7 5700X3D 8-Core Processor\n",
        );
        f.write("sys/bus/pci/devices/0000:09:00.0/class", "0x030000\n");
        f.write("sys/bus/pci/devices/0000:09:00.0/vendor", "0x10de\n");
        f.write("sys/bus/pci/devices/0000:09:00.0/device", "0x2487\n");
        f
    }
    fn write(&self, name: &str, text: &str) {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn exact_hardware_matches_without_driver_or_hostname_dependencies() {
    let f = Fixture::new();
    assert!(performance::detect_in(&f.0));
    f.write("sys/bus/pci/devices/0000:09:00.1/class", "0x040300\n");
    assert!(performance::detect_in(&f.0));
    for (path, value) in [
        ("sys/class/dmi/id/board_vendor", "Other"),
        ("sys/class/dmi/id/board_name", "Other"),
        ("proc/cpuinfo", "model name : AMD Ryzen 7 5700X\n"),
        ("sys/bus/pci/devices/0000:09:00.0/device", "0x2504"),
    ] {
        let original = fs::read(f.0.join(path)).unwrap();
        f.write(path, value);
        assert!(!performance::detect_in(&f.0), "{path}");
        fs::write(f.0.join(path), original).unwrap();
    }
    f.write("sys/bus/pci/devices/0000:01:00.0/class", "0x030000");
    f.write("sys/bus/pci/devices/0000:01:00.0/vendor", "0x1002");
    f.write("sys/bus/pci/devices/0000:01:00.0/device", "0x1234");
    assert!(!performance::detect_in(&f.0));
    assert!(!performance::detect_in(Path::new(
        "/nonexistent-df-hardware"
    )));
}
#[test]
fn stored_opt_out_is_preserved_and_unknown_state_is_not_overwritten() {
    let f = Fixture::new();
    assert!(performance::selection(&f.0).unwrap().is_none());
    f.write("performance-profile", "off\n");
    assert_eq!(
        performance::selection(&f.0).unwrap().as_deref(),
        Some("off")
    );
    f.write("performance-profile", "future-profile\n");
    assert!(performance::selection(&f.0).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("performance-profile")).unwrap(),
        "future-profile\n"
    );
    fs::remove_file(f.0.join("performance-profile")).unwrap();
    std::os::unix::fs::symlink("/dev/null", f.0.join("performance-profile")).unwrap();
    assert!(performance::selection(&f.0).is_err());
}
