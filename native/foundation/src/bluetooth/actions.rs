//! One-shot BlueZ connection actions; BlueZ and PipeWire retain state ownership.
use super::{audio, boolean_property, device_path, property, valid_address};
use crate::{Error, Result, invalid, process};
use serde_json::{Value, json};
use std::{
    thread,
    time::{Duration, Instant},
};

const DEVICE: &str = "org.bluez.Device1";
const PROPERTY_LIMIT: Duration = Duration::from_secs(8);
const RECONNECT_BUDGET: Duration = Duration::from_secs(30);
const RECONNECT_PAUSE: Duration = Duration::from_secs(2);
const AUDIO_SINK_UUID: &str = "0000110b-0000-1000-8000-00805f9b34fb";

fn canonical_codec(codec: &str) -> Option<&'static str> {
    match codec {
        "sbc" => Some("sbc"),
        "sbc_xq" | "sbc-xq" => Some("sbc_xq"),
        _ => None,
    }
}

fn validate(action: &str, path: &str, codec: Option<&str>) -> Result<()> {
    if !matches!(action, "connect" | "disconnect" | "reconnect" | "codec") {
        return Err(invalid("Unknown Bluetooth action"));
    }
    if !device_path(path) {
        return Err(invalid("Invalid BlueZ device path"));
    }
    if codec.is_some_and(|codec| canonical_codec(codec).is_none()) {
        return Err(invalid(
            "Only SBC or SBC XQ playback can be selected through this action",
        ));
    }
    if action == "codec" && codec.is_none() {
        return Err(invalid("No codec selected"));
    }
    Ok(())
}

fn friendly_error(error: &Error, action: &str) -> &'static str {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("notready") || text.contains("not ready") {
        "Bluetooth is turned off or unavailable."
    } else if text.contains("blocked") {
        "This device is blocked. Check Bluetooth Manager."
    } else if text.contains("inprogress") {
        "A connection is already in progress. Try again shortly."
    } else if text.contains("authentication") || text.contains("notauthorized") {
        "Connection was refused. Check the device in Bluetooth Manager."
    } else if matches!(action, "codec" | "codecs") {
        "Could not change the playback codec. Check the device and audio service."
    } else if action == "disconnect" {
        "Could not disconnect. Try again or open Bluetooth Manager."
    } else {
        "Could not connect. Make sure the device is on and nearby."
    }
}

fn connected(path: &str, timeout: Duration) -> Result<bool> {
    boolean_property(path, DEVICE, "Connected", timeout)
}

fn address(path: &str) -> Result<String> {
    let value = property(path, DEVICE, "Address", PROPERTY_LIMIT)?;
    value
        .as_str()
        .filter(|address| valid_address(address, ':'))
        .map(str::to_ascii_uppercase)
        .ok_or_else(|| invalid("Bluetooth device address is invalid"))
}

fn call(path: &str, method: &str, bus_timeout: &str, timeout: Duration) -> Result<()> {
    process::checked_c_locale(
        &[
            "busctl",
            "--system",
            bus_timeout,
            "call",
            "org.bluez",
            path,
            DEVICE,
            method,
        ]
        .map(String::from),
        timeout,
        64 * 1024,
    )?;
    Ok(())
}

fn remaining(deadline: Instant, limit: Duration) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .map(|remaining| remaining.min(limit))
        .ok_or_else(|| invalid("Device has not reconnected yet"))
}

fn reconnect(path: &str) -> Result<()> {
    // Earbud codec changes reboot the device. This finite delay belongs to the
    // requested operation; there is no resident retry task or background poller.
    thread::sleep(Duration::from_secs(5));
    let deadline = Instant::now() + RECONNECT_BUDGET;
    loop {
        let attempt = (|| {
            let timeout = remaining(deadline, Duration::from_secs(12))?;
            let bus_timeout = if timeout >= Duration::from_secs(10) {
                "--timeout=10s".to_owned()
            } else {
                // Bound the native D-Bus wait too when the retry budget is nearly
                // spent. The subprocess owner enforces the same outer deadline.
                format!("--timeout={}ms", timeout.as_millis().max(1))
            };
            call(path, "Connect", &bus_timeout, timeout)?;
            if !connected(path, remaining(deadline, PROPERTY_LIMIT)?)? {
                return Err(invalid("Device has not reconnected yet"));
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(error @ (Error::Process { .. } | Error::Invalid(_))) => {
                let rest = deadline.saturating_duration_since(Instant::now());
                if rest.is_zero() {
                    return Err(error);
                }
                thread::sleep(rest.min(RECONNECT_PAUSE));
            }
            // Malformed native state is not repaired by repeated connection
            // attempts, matching the previous helper's parse-error behavior.
            Err(error) => return Err(error),
        }
    }
}

fn inner(action: &str, path: &str, codec: Option<&str>) -> Result<Value> {
    if !(boolean_property(path, DEVICE, "Paired", PROPERTY_LIMIT)?
        || boolean_property(path, DEVICE, "Bonded", PROPERTY_LIMIT)?)
    {
        return Err(invalid("Device is not paired"));
    }
    match action {
        "codec" => {
            let address = address(path)?;
            if !connected(path, PROPERTY_LIMIT)? {
                return Err(invalid("Device disconnected"));
            }
            let audio = audio::configure(&address, codec)?;
            let mut result = audio
                .as_object()
                .cloned()
                .ok_or_else(|| invalid("Audio routing result is invalid"))?;
            let success = result.get("routed").and_then(Value::as_bool) == Some(true);
            result.insert("success".into(), Value::Bool(success));
            if !success {
                let message = result
                    .get("error")
                    .and_then(Value::as_str)
                    .filter(|message| !message.is_empty())
                    .or_else(|| {
                        result
                            .get("warning")
                            .and_then(Value::as_str)
                            .filter(|message| !message.is_empty())
                    })
                    .unwrap_or("Playback codec could not be confirmed.")
                    .to_owned();
                result.insert("error".into(), Value::String(message));
            }
            Ok(Value::Object(result))
        }
        "connect" | "reconnect" => {
            let adapter = path
                .rsplit_once('/')
                .map(|(adapter, _)| adapter)
                .ok_or_else(|| invalid("Invalid BlueZ device path"))?;
            if !boolean_property(adapter, "org.bluez.Adapter1", "Powered", PROPERTY_LIMIT)? {
                return Err(invalid("Bluetooth not ready"));
            }
            if boolean_property(path, DEVICE, "Blocked", PROPERTY_LIMIT)? {
                return Err(invalid("Device blocked"));
            }
            let uuids = property(path, DEVICE, "UUIDs", PROPERTY_LIMIT)?;
            let uuids = uuids
                .as_array()
                .ok_or_else(|| invalid("Bluetooth device UUIDs are invalid"))?;
            let address = address(path)?;
            if action == "reconnect" {
                reconnect(path)?;
            } else {
                call(path, "Connect", "--timeout=40s", Duration::from_secs(45))?;
            }
            if !connected(path, PROPERTY_LIMIT)? {
                return Err(invalid("BlueZ did not confirm connection"));
            }
            let mut result = json!({"success": true, "connected": true});
            if uuids
                .iter()
                .any(|uuid| uuid.as_str() == Some(AUDIO_SINK_UUID))
            {
                match audio::configure(&address, None) {
                    Ok(audio) if audio.is_object() => {
                        result
                            .as_object_mut()
                            .unwrap()
                            .extend(audio.as_object().unwrap().clone());
                    }
                    Ok(_) | Err(_) => {
                        result["warning"] = Value::String(
                            "Connected; audio could not be switched automatically.".into(),
                        );
                    }
                }
            }
            if !connected(path, PROPERTY_LIMIT)? {
                return Err(invalid("Device disconnected while connecting"));
            }
            Ok(result)
        }
        "disconnect" => {
            call(path, "Disconnect", "--timeout=15s", Duration::from_secs(20))?;
            if connected(path, PROPERTY_LIMIT)? {
                return Err(invalid("BlueZ did not confirm disconnection"));
            }
            Ok(json!({"success": true, "connected": false}))
        }
        _ => unreachable!("action was validated before native requests"),
    }
}

pub fn action(action: &str, path: &str, codec: Option<&str>) -> Result<Value> {
    validate(action, path, codec)?;
    let codec = codec.and_then(canonical_codec);
    inner(action, path, codec).map_err(|error| invalid(friendly_error(&error, action)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_actions_paths_and_codec_before_side_effects() {
        let path = "/org/bluez/hci37/dev_ab_CD_EF_01_02_03";
        for action in ["connect", "disconnect", "reconnect"] {
            assert!(validate(action, path, None).is_ok());
        }
        assert!(validate("codec", path, Some("sbc")).is_ok());
        assert!(validate("codec", path, Some("sbc_xq")).is_ok());
        assert!(validate("codec", path, Some("sbc-xq")).is_ok());
        assert_eq!(canonical_codec("sbc-xq"), Some("sbc_xq"));
        assert_eq!(canonical_codec("sbc_xq"), Some("sbc_xq"));
        assert_eq!(canonical_codec("sbc"), Some("sbc"));
        assert_eq!(canonical_codec("SBC XQ"), None);
        assert!(validate("codec", path, None).is_err());
        assert!(validate("codec", path, Some("ldac")).is_err());
        assert!(validate("power", path, None).is_err());
        assert!(validate("connect", "/org/bluez/hci0/dev_bad", None).is_err());
    }

    #[test]
    fn maps_native_errors_to_action_specific_messages() {
        for (raw, expected) in [
            (
                "org.bluez.Error.NotReady: adapter",
                "Bluetooth is turned off or unavailable.",
            ),
            (
                "Bluetooth not ready",
                "Bluetooth is turned off or unavailable.",
            ),
            (
                "org.bluez.Error.Blocked",
                "This device is blocked. Check Bluetooth Manager.",
            ),
            (
                "org.bluez.Error.InProgress",
                "A connection is already in progress. Try again shortly.",
            ),
            (
                "org.bluez.Error.AuthenticationFailed",
                "Connection was refused. Check the device in Bluetooth Manager.",
            ),
            (
                "org.bluez.Error.NotAuthorized",
                "Connection was refused. Check the device in Bluetooth Manager.",
            ),
        ] {
            assert_eq!(friendly_error(&invalid(raw), "connect"), expected);
        }
        let error = Error::Process {
            command: "busctl".into(),
            detail: "sensitive raw bus diagnostic".into(),
        };
        assert_eq!(
            friendly_error(&error, "codec"),
            "Could not change the playback codec. Check the device and audio service."
        );
        assert_eq!(
            friendly_error(&error, "disconnect"),
            "Could not disconnect. Try again or open Bluetooth Manager."
        );
        assert_eq!(
            friendly_error(&error, "reconnect"),
            "Could not connect. Make sure the device is on and nearby."
        );
    }
}
