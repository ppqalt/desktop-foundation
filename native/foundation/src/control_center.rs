//! Finite control-center requests. PipeWire, BlueZ and NetworkManager own state.
use crate::{Result, apps, bluetooth, invalid, process, state};
use serde_json::{Value, json};
use std::{
    env,
    path::PathBuf,
    time::{Duration, Instant},
};
const QUERY: Duration = Duration::from_secs(3);

fn request(args: &[&str], timeout: Duration) -> Result<String> {
    let output = process::checked_c_locale(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        timeout,
        1024 * 1024,
    )?;
    String::from_utf8(output.stdout).map_err(|_| invalid("Device service returned invalid UTF-8"))
}
fn pulse(kind: &str) -> Result<Value> {
    Ok(serde_json::from_str(&request(
        &["pactl", "--format=json", "list", kind],
        QUERY,
    )?)?)
}
fn endpoint(value: &Value) -> Value {
    // Port labels are intended for people (e.g. "Line Out" or "Headphones").
    // Keep the complete description and stable name for the device picker.
    let description = value["description"]
        .as_str()
        .or(value["name"].as_str())
        .unwrap_or("Audio device");
    let label = value["active_port"]
        .as_str()
        .and_then(|active| {
            value["ports"]
                .as_array()?
                .iter()
                .find(|port| port["name"] == active)?["description"]
                .as_str()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or(description);
    let channels = value["volume"].as_object();
    let percent = channels
        .map(|v| {
            let values: Vec<_> = v.values().filter_map(|v| v["value"].as_f64()).collect();
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64 / 655.36
            }
        })
        .unwrap_or(0.0)
        .round();
    json!({"name":value["name"], "description":description, "label":label,
        "volume":percent.clamp(0.0, 100.0) as u32, "muted":value["mute"].as_bool().unwrap_or(false)})
}
fn audio() -> Result<Value> {
    let info: Value = serde_json::from_str(&request(&["pactl", "--format=json", "info"], QUERY)?)?;
    let sinks = pulse("sinks")?;
    let sources = pulse("sources")?;
    let outputs: Vec<_> = sinks
        .as_array()
        .ok_or_else(|| invalid("Invalid output list"))?
        .iter()
        .map(endpoint)
        .collect();
    let inputs: Vec<_> = sources
        .as_array()
        .ok_or_else(|| invalid("Invalid microphone list"))?
        .iter()
        .filter(|s| !s["name"].as_str().unwrap_or("").ends_with(".monitor"))
        .map(endpoint)
        .collect();
    let streams = pulse("sink-inputs")?;
    let applications: Vec<_> = streams
        .as_array()
        .ok_or_else(|| invalid("Invalid playback stream list"))?
        .iter()
        .filter_map(|stream| {
            let id = stream["index"].as_u64()?;
            let serial = stream["properties"]["object.serial"].as_str().filter(|s| {
                !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_digit())
            })?;
            let properties = &stream["properties"];
            let name = properties["application.name"]
                .as_str()
                .or_else(|| properties["application.process.binary"].as_str())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Application");
            let lower = name.to_ascii_lowercase();
            let node = properties["node.name"].as_str().unwrap_or("");
            if lower.contains("easy effects")
                || lower.contains("easyeffects")
                || node.starts_with("ee_")
                || node.starts_with("easyeffects")
            {
                return None;
            }
            let mut item = endpoint(stream);
            item["name"] = name.into();
            let detail = properties["media.name"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && *s != name);
            item["label"] = detail
                .map_or_else(|| name.to_owned(), |s| format!("{name} · {s}"))
                .into();
            item["id"] = id.into();
            item["serial"] = serial.into();
            Some(item)
        })
        .take(256)
        .collect();
    Ok(
        json!({"available":true,"outputs":outputs,"inputs":inputs,"applications":applications,"output":info["default_sink_name"],"input":info["default_source_name"]}),
    )
}
// nmcli's escaped terse fields may contain colons and backslashes in names.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            out.last_mut().unwrap().push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    if escaped {
        out.last_mut().unwrap().push('\\');
    }
    out
}
fn network() -> Result<Value> {
    let general = request(
        &[
            "nmcli",
            "--terse",
            "--escape",
            "yes",
            "--fields",
            "NETWORKING,WIFI,STATE,CONNECTIVITY",
            "general",
        ],
        QUERY,
    )?;
    let general = fields(general.trim());
    if general.len() != 4 {
        return Err(invalid("Invalid NetworkManager status"));
    }
    let devices = request(
        &[
            "nmcli",
            "--terse",
            "--escape",
            "yes",
            "--fields",
            "DEVICE,TYPE,STATE,CONNECTION",
            "device",
            "status",
        ],
        QUERY,
    )?;
    let devices: Vec<_> = devices
        .lines()
        .map(fields)
        .filter(|v| v.len() == 4 && ["ethernet", "wifi"].contains(&v[1].as_str()))
        .map(|v| json!({"name":v[0],"kind":v[1],"state":v[2],"connection":v[3]}))
        .collect();
    let connections = request(
        &[
            "nmcli",
            "--terse",
            "--escape",
            "yes",
            "--fields",
            "NAME,UUID,TYPE,DEVICE",
            "connection",
            "show",
        ],
        QUERY,
    )?;
    let connections: Vec<_> = connections.lines().map(fields).filter(|v| v.len() == 4 && uuid(&v[1]) && v[2] != "loopback")
        .map(|v| json!({"name":v[0],"uuid":v[1],"kind":v[2],"active":!v[3].is_empty() && v[3] != "--"})).collect();
    Ok(
        json!({"available":true,"editor":apps::executable("nmtui"),"enabled":general[0] == "enabled","wifi":general[1] == "enabled", "state":general[2],"connectivity":general[3],"devices":devices,"connections":connections}),
    )
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn device(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
fn on_off(value: &str) -> Result<bool> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(invalid("Expected on or off")),
    }
}
fn audio_action(args: &[String]) -> Result<()> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
        .ok_or_else(|| invalid("XDG_RUNTIME_DIR is unavailable"))?;
    // Same lock as the keyboard volume backend; no competing read/modify/write.
    let _lock = state::lock(&runtime.join("desktop-foundation-volume.lock"), QUERY)?;
    match args {
        [action, id, serial, value] if action == "stream-volume" => {
            let id_num: u32 = id.parse().map_err(|_| invalid("Invalid playback stream"))?;
            let percent: u32 = value.parse().map_err(|_| invalid("Invalid volume"))?;
            if percent > 100
                || serial.is_empty()
                || serial.len() > 32
                || !serial.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(invalid("Invalid playback stream or volume"));
            }
            let streams = pulse("sink-inputs")?;
            if !streams.as_array().is_some_and(|streams| {
                streams.iter().any(|s| {
                    s["index"].as_u64() == Some(id_num.into())
                        && s["properties"]["object.serial"].as_str() == Some(serial)
                })
            }) {
                return Err(invalid("Playback stream has changed or disappeared"));
            }
            request(
                &[
                    "pactl",
                    "set-sink-input-volume",
                    &id_num.to_string(),
                    &format!("{percent}%"),
                ],
                QUERY,
            )?;
        }
        [action, kind, value]
            if action == "volume" && ["output", "input"].contains(&kind.as_str()) =>
        {
            let percent: u32 = value.parse().map_err(|_| invalid("Invalid volume"))?;
            if percent > 100 {
                return Err(invalid("Volume must be between 0 and 100"));
            }
            let command = if kind == "output" {
                "set-sink-volume"
            } else {
                "set-source-volume"
            };
            let target = if kind == "output" {
                "@DEFAULT_SINK@"
            } else {
                "@DEFAULT_SOURCE@"
            };
            request(&["pactl", command, target, &format!("{percent}%")], QUERY)?;
        }
        [action, kind, value]
            if action == "mute" && ["output", "input"].contains(&kind.as_str()) =>
        {
            let enabled = on_off(value)?;
            let command = if kind == "output" {
                "set-sink-mute"
            } else {
                "set-source-mute"
            };
            let target = if kind == "output" {
                "@DEFAULT_SINK@"
            } else {
                "@DEFAULT_SOURCE@"
            };
            request(
                &["pactl", command, target, if enabled { "1" } else { "0" }],
                QUERY,
            )?;
        }
        [action, kind, name]
            if action == "select" && ["output", "input"].contains(&kind.as_str()) =>
        {
            let output = kind == "output";
            let list = pulse(if output { "sinks" } else { "sources" })?;
            if name.starts_with('-')
                || !list
                    .as_array()
                    .is_some_and(|v| v.iter().any(|v| v["name"].as_str() == Some(name)))
            {
                return Err(invalid("The selected audio device is no longer available"));
            }
            request(
                &[
                    "pactl",
                    if output {
                        "set-default-sink"
                    } else {
                        "set-default-source"
                    },
                    name,
                ],
                QUERY,
            )?;
            // Re-route existing playback/capture as well as future streams.
            let streams = pulse(if output {
                "sink-inputs"
            } else {
                "source-outputs"
            })?;
            let streams = streams
                .as_array()
                .ok_or_else(|| invalid("Invalid audio stream list"))?;
            if streams.len() > 256 {
                return Err(invalid("Too many audio streams to route"));
            }
            let deadline = Instant::now() + Duration::from_secs(8);
            for stream in streams {
                let index = stream["index"]
                    .as_u64()
                    .ok_or_else(|| invalid("Invalid audio stream"))?;
                // Streams can end between enumeration and routing. Only suppress
                // failure after confirming that the stream has disappeared.
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(invalid(
                        "Audio routing timed out; some streams may still use the previous device",
                    ));
                }
                if let Err(error) = request(
                    &[
                        "pactl",
                        if output {
                            "move-sink-input"
                        } else {
                            "move-source-output"
                        },
                        &index.to_string(),
                        name,
                    ],
                    remaining.min(QUERY),
                ) {
                    let current = pulse(if output {
                        "sink-inputs"
                    } else {
                        "source-outputs"
                    })?;
                    if current
                        .as_array()
                        .is_some_and(|v| v.iter().any(|s| s["index"].as_u64() == Some(index)))
                    {
                        return Err(error);
                    }
                }
            }
        }
        _ => return Err(invalid("Expected volume|mute|select output|input VALUE")),
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<Value> {
    let parts: Vec<_> = args.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["check-updates"] => {
            request(&["arch-update", "--check"], Duration::from_secs(120))?;
            return Ok(json!({"success":true}));
        }
        ["snapshot"] => {
            let audio =
                audio().unwrap_or_else(|e| json!({"available":false,"error":e.to_string()}));
            let network =
                network().unwrap_or_else(|e| json!({"available":false,"error":e.to_string()}));
            return Ok(json!({"audio":audio,"network":network}));
        }
        ["volume" | "mute" | "select" | "stream-volume", ..] => {
            audio_action(args)?;
            return Ok(json!({"success":true,"audio":audio()?}));
        }
        ["networking", value] => {
            on_off(value)?;
            request(
                &["nmcli", "--wait", "10", "networking", value],
                Duration::from_secs(12),
            )?;
        }
        ["wifi", value] => {
            on_off(value)?;
            request(
                &["nmcli", "--wait", "10", "radio", "wifi", value],
                Duration::from_secs(12),
            )?;
        }
        ["connection", action @ ("up" | "down"), id] if uuid(id) => {
            request(
                &["nmcli", "--wait", "10", "connection", action, "uuid", id],
                Duration::from_secs(12),
            )?;
        }
        ["device", action @ ("connect" | "disconnect"), name] if device(name) => {
            request(
                &["nmcli", "--wait", "10", "device", action, name],
                Duration::from_secs(12),
            )?;
        }
        ["bluetooth", "power", value] => return bluetooth::power(on_off(value)?),
        ["bluetooth", action @ ("connect" | "disconnect"), path] => {
            return bluetooth::action(action, path, None);
        }
        _ => return Err(invalid("Invalid control-center action")),
    }
    Ok(json!({"success":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn friendly_port_label_preserves_device_identity() {
        let mut value = json!({"name":"alsa_output.card", "description":"Hardware Digital Stereo",
            "active_port":"digital", "ports":[{"name":"digital","description":"Digital Output (S/PDIF)"}]});
        let device = endpoint(&value);
        assert_eq!(device["name"], "alsa_output.card");
        assert_eq!(device["description"], "Hardware Digital Stereo");
        assert_eq!(device["label"], "Digital Output (S/PDIF)");
        value["active_port"] = json!("missing");
        assert_eq!(endpoint(&value)["label"], "Hardware Digital Stereo");
    }
    #[test]
    fn escaped_network_fields() {
        assert_eq!(
            fields(r"Office\: WiFi:abc:802-11-wireless:wl\\an0"),
            ["Office: WiFi", "abc", "802-11-wireless", r"wl\an0"]
        );
    }
    #[test]
    fn strict_network_identifiers() {
        assert!(uuid("12345678-abcd-1234-abcd-123456789abc"));
        assert!(!uuid("--help"));
        assert!(!device("--help"));
        assert!(!device("eth0;reboot"));
        assert!(device("enp4s0"));
    }
}
