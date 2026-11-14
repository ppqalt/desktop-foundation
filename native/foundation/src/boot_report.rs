//! Read-only boot evidence. Kernel source time is deliberately distinct from receipt time.
use crate::{
    Result,
    doctor::{self, Runtime},
    invalid, startup_report, state,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug)]
pub struct Options {
    pub boots: usize,
    pub save: Option<String>,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            boots: 4,
            save: None,
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let (key, inline) = arg
                .split_once('=')
                .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
            let value = inline
                .or_else(|| args.next().map(String::as_str))
                .ok_or_else(|| invalid(format!("{key} requires a value")))?;
            match key {
                "--boots" => {
                    options.boots = value
                        .parse()
                        .map_err(|_| invalid("Choose between one and twenty boots"))?
                }
                "--save" => options.save = Some(value.into()),
                _ => return Err(invalid(format!("Unknown boot-report option: {key}"))),
            }
        }
        if !(1..=20).contains(&options.boots) {
            return Err(invalid("Choose between one and twenty boots"));
        }
        if options.save.as_deref().is_some_and(|s| !label_valid(s)) {
            return Err(invalid(
                "Invalid snapshot label; use letters, numbers, underscores or hyphens",
            ));
        }
        Ok(options)
    }
}
fn label_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
/// Parse systemd's duration tokens, including compound minutes and subseconds.
pub fn seconds(text: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut remainder = text.trim();
    if remainder.is_empty() {
        return None;
    }
    while !remainder.is_empty() {
        let end = remainder
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(remainder.len());
        if end == 0 {
            return None;
        }
        let number = remainder[..end].parse::<f64>().ok()?;
        remainder = &remainder[end..];
        let (unit, multiplier) = [("min", 60.0), ("ms", 0.001), ("us", 0.000001), ("s", 1.0)]
            .into_iter()
            .find(|(unit, _)| remainder.starts_with(unit))?;
        total += number * multiplier;
        remainder = remainder[unit.len()..].trim_start();
    }
    total.is_finite().then_some(total)
}
fn timestamp(row: &Value, key: &str) -> Option<f64> {
    let v = row[key].as_str()?.parse::<u64>().ok()?;
    Some(v as f64 / 1e6)
}
pub fn collect_boot(rows: &str) -> Result<Value> {
    let mut events = BTreeMap::new();
    let mut kernel = BTreeMap::new();
    let mut totals = BTreeMap::new();
    let mut durations = BTreeMap::new();
    let mut boot_id = None;
    for line in rows.lines().filter(|s| !s.trim().is_empty()) {
        let row: Value = serde_json::from_str(line)
            .map_err(|e| invalid(format!("Invalid boot journal record: {e}")))?;
        if boot_id.is_none() {
            boot_id = row["_BOOT_ID"].as_str().map(str::to_owned);
        }
        let Some(stamp) = timestamp(&row, "__MONOTONIC_TIMESTAMP") else {
            continue;
        };
        let message = startup_report::decode_message(&row["MESSAGE"]);
        for (name, needle) in [
            ("root_ready", "Reached target Initrd Root File System."),
            ("initrd_default", "Reached target Initrd Default Target."),
            ("udev_cleanup", "Starting Cleanup udev Database..."),
            ("switch_root", "Switching root."),
            ("greetd_started", "Started Greeter daemon."),
            ("sddm_started", "Started Simple Desktop Display Manager."),
            (
                "binfmt_start",
                "Starting Set Up Additional Binary Formats...",
            ),
            ("binfmt_end", "Finished Set Up Additional Binary Formats."),
        ] {
            if message.contains(needle) {
                events.entry(name).or_insert(stamp);
            }
        }
        if let Some(message) = message
            .strip_prefix("Startup finished in ")
            .filter(|s| s.contains("(firmware)"))
            && let Some((stages, total)) = message.split_once(" = ")
        {
            for stage in stages.split(" + ") {
                if let Some((duration, name)) = stage.trim().split_once(" (") {
                    let name = name.trim_end_matches(')');
                    if ["firmware", "loader", "kernel", "initrd", "userspace"].contains(&name)
                        && let Some(value) = seconds(duration)
                    {
                        totals.insert(name.to_owned(), value);
                    }
                }
            }
            if let Some(value) = seconds(total.trim_end_matches('.')) {
                totals.insert("total".into(), value);
            }
        }
        if row["_TRANSPORT"] == "kernel" {
            // Never substitute a journal receipt stamp for an explicitly invalid source stamp.
            let source = if row.get("_SOURCE_MONOTONIC_TIMESTAMP").is_some() {
                timestamp(&row, "_SOURCE_MONOTONIC_TIMESTAMP")
            } else {
                Some(stamp)
            };
            if let Some(source) = source {
                for (name, needle) in [
                    ("nvidia_module_start", "nvidia: loading out-of-tree module"),
                    ("nvidia_module_announced", "NVRM: loading NVIDIA UNIX"),
                    ("nvidia_drm_ready", "[drm] Initialized nvidia-drm"),
                    ("nvidia_framebuffer", "[drm] fb0: nvidia-drmdrmfb"),
                    ("nvme_discovery", "nvme nvme0: pci function"),
                    ("unpack_start", "Trying to unpack rootfs image"),
                    ("unpack_end", "Freeing initrd memory:"),
                    ("init_process", "Run /init as init process"),
                    ("amdgpu_init", "initializing kernel modesetting"),
                    ("gpu_stb", "STB initialized"),
                    ("gpu_ready", "fb0: amdgpudrmfb frame buffer device"),
                ] {
                    if message.contains(needle) {
                        kernel.entry(name).or_insert(source);
                    }
                }
            }
        }
    }
    for (name, data, start, end) in [
        (
            "initramfs_unpack_interval",
            &kernel,
            "unpack_start",
            "unpack_end",
        ),
        ("amdgpu_initialization", &kernel, "amdgpu_init", "gpu_ready"),
        (
            "root_ready_to_switch_root",
            &events,
            "root_ready",
            "switch_root",
        ),
        ("binfmt", &events, "binfmt_start", "binfmt_end"),
        (
            "nvidia_module_to_drm",
            &kernel,
            "nvidia_module_start",
            "nvidia_drm_ready",
        ),
        (
            "nvidia_announcement_to_nvme_discovery",
            &kernel,
            "nvidia_module_announced",
            "nvme_discovery",
        ),
    ] {
        if let (Some(start), Some(end)) = (data.get(start), data.get(end)) {
            durations.insert(name, ((end - start) * 1e6_f64).round() / 1e6);
        }
    }
    Ok(
        json!({"boot_id":boot_id,"stages_seconds":totals,"journal_events_seconds":events,"kernel_source_events_seconds":kernel,"intervals_seconds":durations}),
    )
}
pub fn collect(options: &Options, r: &impl Runtime) -> Result<Value> {
    if !(1..=20).contains(&options.boots) {
        return Err(invalid("Choose between one and twenty boots"));
    }
    let mut boots = Vec::new();
    for offset in 0..options.boots {
        let reply = doctor::query(
            r,
            &[
                "journalctl",
                "-b",
                &format!("-{}", offset),
                "-o",
                "json",
                "--no-pager",
                "--grep",
                "Reached target Initrd|Cleanup udev Database|Switching root|Started Greeter daemon|Started Simple Desktop Display Manager|nvidia: loading|NVRM: loading NVIDIA UNIX|Initialized nvidia-drm|fb0: nvidia-drmdrmfb|nvme nvme0: pci function|Additional Binary Formats|Startup finished in|Trying to unpack rootfs image|Freeing initrd memory:|Run /init as init process|initializing kernel modesetting|STB initialized|fb0: amdgpudrmfb frame buffer device",
            ],
        );
        if !reply.ok() {
            return Err(invalid(format!(
                "Boot -{offset} journal unavailable: {}",
                reply.text
            )));
        }
        let boot = collect_boot(&reply.text)?;
        if boot["boot_id"].is_null() {
            return Err(invalid(format!("Boot -{offset} has no journal evidence")));
        }
        boots.push(boot);
    }
    let binfmt = doctor::query(
        r,
        &[
            "systemctl",
            "show",
            "systemd-binfmt.service",
            "-p",
            "ExecMainStartTimestampMonotonic",
            "-p",
            "ExecMainExitTimestampMonotonic",
            "-p",
            "InvocationID",
            "-p",
            "ConditionResult",
        ],
    );
    let mut medians = BTreeMap::new();
    if let Some(stages) = boots[0]["stages_seconds"].as_object() {
        for key in stages.keys() {
            let mut values: Vec<f64> = boots
                .iter()
                .filter_map(|b| b["stages_seconds"][key].as_f64())
                .collect();
            values.sort_by(f64::total_cmp);
            let n = values.len();
            medians.insert(
                key,
                if n.is_multiple_of(2) {
                    (values[n / 2 - 1] + values[n / 2]) / 2.0
                } else {
                    values[n / 2]
                },
            );
        }
    }
    Ok(
        json!({"boots":boots,"stage_medians_seconds":medians,"current_binfmt_unit_metadata":binfmt.text,"current_session":startup_report::collect(r),"limitations":[
        "Bootloader time includes time spent choosing entries.",
        "Login-manager service timestamps are not measured first visible frames.",
        "Kernel source timestamps and journal receipt timestamps are separate timelines.",
        "Initramfs unpack interval includes other overlapping kernel work.",
        "Saved labels do not by themselves prove a change was deployed."]}),
    )
}
pub fn save(report: &Value, label: &str, directory: &std::path::Path) -> Result<PathBuf> {
    if !label_valid(label) {
        return Err(invalid("Invalid snapshot label"));
    }
    let boot = report["boots"][0]["boot_id"]
        .as_str()
        .filter(|s| {
            !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        })
        .ok_or_else(|| invalid("Invalid boot ID; snapshot not saved"))?;
    state::private_directory(directory)?;
    let target = directory.join(format!("{label}-{boot}.json"));
    let mut data = serde_json::to_vec_pretty(report)?;
    data.push(b'\n');
    state::atomic(&target, &data, 0o600)?;
    Ok(target)
}
pub fn run(args: &[String]) -> Result<()> {
    let options = Options::parse(args)?;
    // SAFETY: no pointers; keep user-controlled diagnostics/tools unprivileged.
    if unsafe { libc::geteuid() } == 0 {
        return Err(invalid("Run boot-report as your normal desktop user"));
    }
    let report = collect(&options, &doctor::Native)?;
    if let Some(label) = options.save {
        let directory = state::xdg("XDG_CACHE_HOME", ".cache")?
            .join("desktop-foundation/boot-audit/optimization");
        println!("Saved {}", save(&report, &label, &directory)?.display());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
