//! Finite Bluetooth requests and event-driven playback routing. Native services
//! own Bluetooth/PipeWire state; no helper remains between requests.
mod actions;
mod audio;
use crate::{Error, Result, invalid, process};
pub use actions::action;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    thread,
    time::{Duration, Instant},
};

const REQUEST_LIMIT: Duration = Duration::from_secs(2);
const POWER_BUDGET: Duration = Duration::from_secs(5);
const RETRY_PAUSE: Duration = Duration::from_millis(150);

fn request(args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    Ok(process::checked_c_locale(
        &args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        timeout,
        64 * 1024,
    )?
    .stdout)
}

fn property(path: &str, interface: &str, name: &str, timeout: Duration) -> Result<Value> {
    let result: Value = serde_json::from_slice(&request(
        &[
            "busctl",
            "--system",
            "--json=short",
            "get-property",
            "org.bluez",
            path,
            interface,
            name,
        ],
        timeout,
    )?)?;
    result
        .get("data")
        .cloned()
        .ok_or_else(|| invalid("Bluetooth property data is missing"))
}

fn boolean_property(path: &str, interface: &str, name: &str, timeout: Duration) -> Result<bool> {
    property(path, interface, name, timeout)?
        .as_bool()
        .ok_or_else(|| invalid("Bluetooth property is not a boolean"))
}

fn adapter_path(path: &str) -> bool {
    path.strip_prefix("/org/bluez/hci")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
}

pub fn device_path(path: &str) -> bool {
    path.rsplit_once("/dev_")
        .is_some_and(|(adapter, address)| adapter_path(adapter) && valid_address(address, '_'))
}

fn valid_address(address: &str, separator: char) -> bool {
    let parts: Vec<_> = address.split(separator).collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .map(|duration| duration.min(REQUEST_LIMIT))
        .ok_or_else(|| invalid("Bluetooth radio operation timed out"))
}

fn power_inner(enabled: bool) -> Result<Value> {
    let tree = request(
        &["busctl", "--system", "--list", "tree", "org.bluez"],
        REQUEST_LIMIT,
    )?;
    let mut pending: BTreeSet<_> = std::str::from_utf8(&tree)
        .map_err(|_| invalid("Bluetooth adapter list is not UTF-8"))?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|path| adapter_path(path))
        .map(str::to_owned)
        .collect();
    if pending.is_empty() {
        return Err(invalid("No Bluetooth adapter is available."));
    }
    if enabled {
        request(&["rfkill", "unblock", "bluetooth"], REQUEST_LIMIT)?;
    }
    let deadline = Instant::now() + POWER_BUDGET;
    while !pending.is_empty() && Instant::now() < deadline {
        for path in pending.clone() {
            let timeout = remaining(deadline)?;
            let result = request(
                &[
                    "busctl",
                    "--system",
                    "set-property",
                    "org.bluez",
                    &path,
                    "org.bluez.Adapter1",
                    "Powered",
                    "b",
                    if enabled { "true" } else { "false" },
                ],
                timeout,
            );
            // BlueZ may briefly reject power-on immediately after rfkill unblock.
            // Only native command failures are retried; malformed data is an error.
            if let Err(Error::Process { .. }) = result {
                continue;
            }
            result?;
            match boolean_property(&path, "org.bluez.Adapter1", "Powered", remaining(deadline)?) {
                Ok(state) if state == enabled => {
                    pending.remove(&path);
                }
                Ok(_) | Err(Error::Process { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        if !pending.is_empty() {
            thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(RETRY_PAUSE),
            );
        }
    }
    if !pending.is_empty() {
        return Err(invalid(if enabled {
            "Bluetooth could not be turned on. Check the hardware radio switch or airplane mode."
        } else {
            "Bluetooth could not be turned off. Try again."
        }));
    }
    Ok(json!({"success": true, "powered": enabled}))
}

pub fn power(enabled: bool) -> Result<Value> {
    power_inner(enabled).map_err(|error| match error {
        Error::Invalid(message) if message == "No Bluetooth adapter is available." => invalid(message),
        Error::Invalid(message) if message.starts_with("Bluetooth could not be turned") => invalid(message),
        Error::Invalid(message) if message == "Bluetooth radio operation timed out" => invalid(if enabled {
            "Bluetooth could not be turned on. Check the hardware radio switch or airplane mode."
        } else {
            "Bluetooth could not be turned off. Try again."
        }),
        _ => invalid("Bluetooth radio request failed. Check adapter availability and permissions."),
    })
}

fn address_of(card: &Value) -> String {
    let properties = &card["properties"];
    let address = properties["api.bluez5.address"]
        .as_str()
        .filter(|address| !address.is_empty())
        .or_else(|| properties["device.string"].as_str())
        .unwrap_or("");
    if valid_address(address, ':') {
        return address.to_ascii_uppercase();
    }
    properties["api.bluez5.path"]
        .as_str()
        .and_then(|path| path.rsplit_once("/dev_").map(|(_, address)| address))
        .filter(|address| valid_address(address, '_'))
        .map(|address| address.replace('_', ":").to_ascii_uppercase())
        .unwrap_or_default()
}

fn profile_codec(name: &str, info: &Value) -> &'static str {
    let hint =
        format!("{} {}", name, info["description"].as_str().unwrap_or("")).to_ascii_lowercase();
    if hint.contains("ldac") {
        "ldac"
    } else if hint.contains("aac") {
        "aac"
    } else if ["sbc_xq", "sbc-xq", "sbc xq"]
        .iter()
        .any(|s| hint.contains(s))
    {
        "sbc_xq"
    } else if hint.contains("sbc") {
        "sbc"
    } else {
        ""
    }
}

fn available_codecs(cards: &[Value], address: &str) -> BTreeSet<&'static str> {
    let mut codecs = BTreeSet::new();
    for card in cards.iter().filter(|card| address_of(card) == address) {
        if let Some(profiles) = card["profiles"].as_object() {
            for (name, info) in profiles {
                if !name.starts_with("a2dp-sink")
                    || info["available"] == false
                    || info["available"] == "no"
                {
                    continue;
                }
                let codec = profile_codec(name, info);
                if !codec.is_empty() {
                    codecs.insert(codec);
                }
            }
        }
    }
    codecs
}

pub fn codecs(path: &str) -> Result<Value> {
    if !device_path(path) {
        return Err(invalid("Invalid BlueZ device path"));
    }
    let result = (|| {
        if !(boolean_property(path, "org.bluez.Device1", "Paired", REQUEST_LIMIT)?
            || boolean_property(path, "org.bluez.Device1", "Bonded", REQUEST_LIMIT)?)
        {
            return Err(invalid("Device is not paired"));
        }
        let address = property(path, "org.bluez.Device1", "Address", REQUEST_LIMIT)?;
        let address = address
            .as_str()
            .filter(|s| valid_address(s, ':'))
            .ok_or_else(|| invalid("Bluetooth device address is invalid"))?
            .to_ascii_uppercase();
        let snapshot = process::checked_c_locale(
            &["pactl", "--format=json", "list", "cards"].map(String::from),
            Duration::from_secs(8),
            2 * 1024 * 1024,
        )?;
        let cards: Vec<Value> = serde_json::from_slice(&snapshot.stdout)?;
        Ok(json!({"success": true, "codecs": available_codecs(&cards, &address)}))
    })();
    result.map_err(|_| {
        invalid("Could not read the playback codecs. Check the device and audio service.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_address_fallbacks_are_dynamic_and_strict() {
        assert!(device_path("/org/bluez/hci37/dev_ab_CD_EF_01_02_03"));
        for path in [
            "/org/bluez/hci/dev_AB_CD_EF_01_02_03",
            "/org/bluez/hci0/dev_AB_CD_EF_01_02",
            "/org/bluez/custom/dev_AB_CD_EF_01_02_03",
            "/org/bluez/hci0/dev_AB_CD_EF_01_02_03;sh",
        ] {
            assert!(!device_path(path));
        }
        assert_eq!(
            address_of(
                &json!({"properties": {"api.bluez5.path": "/org/bluez/hci37/dev_ab_CD_EF_01_02_03"}})
            ),
            "AB:CD:EF:01:02:03"
        );
        assert_eq!(
            address_of(&json!({"properties": {"device.string": "hw:0"}})),
            ""
        );
    }
}
