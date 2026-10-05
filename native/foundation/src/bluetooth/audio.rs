//! Finite playback routing owned by the active Bluetooth request. PipeWire-Pulse
//! events trigger snapshots; client/query events never become a polling loop.
use super::{address_of, profile_codec, valid_address};
use crate::{Result, invalid, process};
use serde_json::{Value, json};
use std::{
    cmp::Reverse,
    collections::BTreeSet,
    time::{Duration, Instant},
};

const AUDIO_BUDGET: Duration = Duration::from_secs(12);
const REQUEST_LIMIT: Duration = Duration::from_secs(8);
const SNAPSHOT_LIMIT: u64 = 2 * 1024 * 1024;
const EVENT_LINE_LIMIT: usize = 64 * 1024;
const NOT_READY: &str = "Connected; audio output is not ready yet.";
const UNAVAILABLE: &str = "Connected; audio service is unavailable.";

fn remaining(deadline: Instant) -> Option<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .map(|duration| duration.min(REQUEST_LIMIT))
}

fn warning(message: &str) -> Value {
    json!({"routed": false, "warning": message})
}

fn profiles<'a>(card: &'a Value, requested_codec: Option<&str>) -> Vec<&'a str> {
    let Some(profiles) = card["profiles"].as_object() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for (name, info) in profiles {
        if !name.starts_with("a2dp-sink") || info["available"] == false || info["available"] == "no"
        {
            continue;
        }
        let codec = profile_codec(name, info);
        if requested_codec.is_some_and(|requested| codec != requested) {
            continue;
        }
        let rank = match codec {
            "ldac" => 0,
            "aac" => 1,
            _ => 2,
        };
        candidates.push((
            rank,
            Reverse(info["priority"].as_i64().unwrap_or(0)),
            name.as_str(),
        ));
    }
    candidates.sort_unstable();
    candidates.into_iter().map(|(_, _, name)| name).collect()
}

fn snapshot(kind: &str, timeout: Duration) -> Result<Vec<Value>> {
    let output = process::checked_c_locale(
        &["pactl", "--format=json", "list", kind].map(String::from),
        timeout,
        SNAPSHOT_LIMIT,
    )?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn command(args: &[&str], timeout: Duration) -> Result<()> {
    process::checked_c_locale(
        &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
        timeout,
        64 * 1024,
    )?;
    Ok(())
}

fn codec_of(sink: &Value) -> &str {
    sink["properties"]["api.bluez5.codec"]
        .as_str()
        .unwrap_or("")
}

fn codec_matches(actual: &str, expected: &str) -> bool {
    fn normalized(codec: &str) -> impl Iterator<Item = u8> + '_ {
        codec
            .bytes()
            .filter(|byte| !matches!(byte, b'_' | b'-') && !byte.is_ascii_whitespace())
            .map(|byte| byte.to_ascii_lowercase())
    }
    normalized(actual).eq(normalized(expected))
}

fn matches_profile(sink: &Value, card: Option<&Value>, selected: &str, expected: &str) -> bool {
    selected.is_empty()
        || (sink["properties"]["api.bluez5.profile"] == "a2dp-sink"
            && card.is_some_and(|card| card["active_profile"] == selected)
            && (expected.is_empty() || codec_matches(codec_of(sink), expected)))
}

fn matches_request(sink: &Value, requested_codec: Option<&str>) -> bool {
    requested_codec.is_none_or(|codec| {
        sink["properties"]["api.bluez5.profile"] == "a2dp-sink"
            && codec_matches(codec_of(sink), codec)
    })
}

/// Retain only a partial event line. Complete ignored lines are discarded, so
/// ordinary client/source/stream traffic cannot accumulate memory or snapshots.
fn relevant_events(buffered: &mut Vec<u8>, data: &[u8]) -> Result<bool> {
    let mut relevant = false;
    for &byte in data {
        if byte == b'\n' {
            relevant |= [b" on card #".as_slice(), b" on sink #", b" on server #"]
                .iter()
                .any(|marker| {
                    buffered
                        .windows(marker.len())
                        .any(|window| window == *marker)
                });
            buffered.clear();
        } else {
            if buffered.len() == EVENT_LINE_LIMIT {
                return Err(invalid("Audio event line exceeded 64 KiB"));
            }
            buffered.push(byte);
        }
    }
    Ok(relevant)
}

pub fn configure(address: &str, requested_codec: Option<&str>) -> Result<Value> {
    if !valid_address(address, ':') {
        return Err(invalid("Bluetooth device address is invalid"));
    }
    let requested_codec = requested_codec.map(|codec| match codec {
        "sbc-xq" => "sbc_xq",
        _ => codec,
    });
    let address = address.to_ascii_uppercase();
    let deadline = Instant::now() + AUDIO_BUDGET;
    // Subscribe before the initial snapshots to avoid missing a newly created
    // card/sink between discovery and waiting. Drop always stops this child.
    let mut events = process::Subscription::open(&["pactl", "subscribe"].map(String::from))?;
    let mut attempted = BTreeSet::new();
    let mut selected_profile = String::new();
    let mut expected_codec = String::new();
    let mut buffered = Vec::new();
    loop {
        let Some(timeout) = remaining(deadline) else {
            return Ok(warning(NOT_READY));
        };
        let cards = match snapshot("cards", timeout) {
            Ok(cards) => cards,
            Err(_) if remaining(deadline).is_none() => return Ok(warning(NOT_READY)),
            Err(error) => return Err(error),
        };
        let card = cards.iter().find(|card| address_of(card) == address);
        if let Some(card) = card
            && selected_profile.is_empty()
        {
            let candidates = profiles(card, requested_codec);
            if let Some(codec) = requested_codec
                && candidates.is_empty()
            {
                return Ok(json!({"routed": false, "error": format!(
                    "This device does not currently offer {} playback.", codec.to_ascii_uppercase()
                )}));
            }
            let card_name = card["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| invalid("Playback card name is missing"))?;
            for profile in candidates {
                if !attempted.insert(profile.to_owned()) {
                    continue;
                }
                let Some(timeout) = remaining(deadline) else {
                    return Ok(warning(NOT_READY));
                };
                // Codec preference must never prevent connection. Failed native
                // profile requests fall back through the remaining candidates.
                if command(&["pactl", "set-card-profile", card_name, profile], timeout).is_ok() {
                    selected_profile = profile.to_owned();
                    let codec = profile_codec(profile, &card["profiles"][profile]);
                    expected_codec = requested_codec
                        .unwrap_or(if matches!(codec, "ldac" | "aac") {
                            codec
                        } else {
                            ""
                        })
                        .to_owned();
                    break;
                }
            }
        }
        let Some(timeout) = remaining(deadline) else {
            return Ok(warning(NOT_READY));
        };
        let sinks = match snapshot("sinks", timeout) {
            Ok(sinks) => sinks,
            Err(_) if remaining(deadline).is_none() => return Ok(warning(NOT_READY)),
            Err(error) => return Err(error),
        };
        // Confirm the chosen A2DP profile and codec against the fresh matching
        // card. An old headset sink must not be routed while nodes are replaced.
        if let Some(sink) = sinks.iter().find(|sink| {
            address_of(sink) == address
                && matches_profile(sink, card, &selected_profile, &expected_codec)
                && matches_request(sink, requested_codec)
        }) {
            let name = sink["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| invalid("Playback sink name is missing"))?;
            let Some(timeout) = remaining(deadline) else {
                return Ok(warning(NOT_READY));
            };
            if let Err(error) = command(&["pactl", "set-default-sink", name], timeout) {
                if remaining(deadline).is_none() {
                    return Ok(warning(NOT_READY));
                }
                return Err(error);
            }
            return Ok(json!({"codec": codec_of(sink), "routed": true}));
        }
        loop {
            match events.read(deadline)? {
                process::StreamRead::Data(data) => {
                    if relevant_events(&mut buffered, &data)? {
                        break;
                    }
                }
                process::StreamRead::Eof => return Ok(warning(UNAVAILABLE)),
                process::StreamRead::Timeout => return Ok(warning(NOT_READY)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_prefer_ldac_aac_then_other_available_playback() {
        let card = json!({"profiles": {
            "off": {"priority": 10000},
            "headset-head-unit": {"priority": 2000},
            "a2dp-sink-sbc": {"priority": 10, "available": true},
            "a2dp-sink-aptx_hd": {"priority": 50, "available": true},
            "a2dp-sink-aac": {"priority": 20, "available": true},
            "a2dp-sink": {"description": "codec LDAC", "priority": 1, "available": true},
            "a2dp-sink-unavailable": {"priority": 100, "available": false},
            "a2dp-sink-disabled": {"priority": 999, "available": "no"}
        }});
        assert_eq!(
            profiles(&card, None),
            [
                "a2dp-sink",
                "a2dp-sink-aac",
                "a2dp-sink-aptx_hd",
                "a2dp-sink-sbc"
            ]
        );
        assert_eq!(profiles(&card, Some("sbc")), ["a2dp-sink-sbc"]);
        assert!(profiles(&card, Some("missing")).is_empty());
    }

    #[test]
    fn profiles_have_stable_priority_and_do_not_confuse_sbc_xq() {
        let card = json!({"profiles": {
            "a2dp-sink-b": {"description": "codec SBC", "priority": 20},
            "a2dp-sink-a": {"description": "codec SBC", "priority": 20},
            "a2dp-sink-xq": {"description": "codec SBC XQ", "priority": 50},
            "a2dp-sink-low": {"description": "codec SBC", "priority": 1}
        }});
        assert_eq!(
            profiles(&card, Some("sbc")),
            ["a2dp-sink-a", "a2dp-sink-b", "a2dp-sink-low"]
        );
        assert_eq!(profiles(&card, Some("sbc_xq")), ["a2dp-sink-xq"]);
    }

    #[test]
    fn xq_sink_spellings_match_exact_codec_without_accepting_plain_sbc() {
        for actual in ["sbc_xq", "SBC_XQ", "sbc-xq", "SBC XQ"] {
            assert!(codec_matches(actual, "sbc_xq"));
            assert!(codec_matches(actual, "sbc-xq"));
            assert!(!codec_matches(actual, "sbc"));
        }
        assert!(!codec_matches("sbc", "sbc_xq"));
        assert!(!codec_matches("aac", "sbc_xq"));
        let card = json!({"active_profile": "a2dp-sink-sbc_xq"});
        let sink = json!({"properties": {"api.bluez5.profile": "a2dp-sink", "api.bluez5.codec": "SBC XQ"}});
        assert!(matches_profile(
            &sink,
            Some(&card),
            "a2dp-sink-sbc_xq",
            "sbc_xq"
        ));
        assert!(!matches_profile(
            &sink,
            Some(&card),
            "a2dp-sink-sbc_xq",
            "sbc"
        ));
        // A failed profile write leaves selected empty. The explicit request
        // still rejects an existing plain SBC or headset sink in that case.
        assert!(matches_profile(&sink, None, "", ""));
        assert!(matches_request(&sink, Some("sbc_xq")));
        assert!(!matches_request(&sink, Some("sbc")));
        let plain_sbc =
            json!({"properties": {"api.bluez5.profile": "a2dp-sink", "api.bluez5.codec": "sbc"}});
        assert!(!matches_request(&plain_sbc, Some("sbc_xq")));
        let headset = json!({"properties": {"api.bluez5.profile": "headset-head-unit", "api.bluez5.codec": "SBC_XQ"}});
        assert!(!matches_request(&headset, Some("sbc_xq")));
        assert!(matches_request(&headset, None));
    }

    #[test]
    fn partial_events_ignore_query_clients_and_streams() {
        let mut buffer = Vec::new();
        assert!(!relevant_events(&mut buffer, b"Event 'new' on client #1\nEvent 'change' on source #2\nEvent 'new' on sink-input #3\nEvent 'change' on si").unwrap());
        assert!(relevant_events(&mut buffer, b"nk #4\nEvent 'new' on source-output #5\n").unwrap());
        assert!(buffer.is_empty());
        assert!(relevant_events(&mut buffer, b"Event 'change' on server #6\n").unwrap());
        assert!(relevant_events(&mut buffer, b"Event 'change' on card #7\n").unwrap());
    }

    #[test]
    fn unfinished_events_are_bounded_and_stale_sinks_rejected() {
        let mut buffer = vec![b'a'; EVENT_LINE_LIMIT];
        assert!(relevant_events(&mut buffer, b"b").is_err());
        let sink =
            json!({"properties": {"api.bluez5.profile": "a2dp-sink", "api.bluez5.codec": "aac"}});
        let old_card = json!({"active_profile": "headset-head-unit"});
        let card = json!({"active_profile": "a2dp-sink-aac"});
        assert!(!matches_profile(
            &sink,
            Some(&old_card),
            "a2dp-sink-aac",
            "aac"
        ));
        assert!(!matches_profile(&sink, None, "a2dp-sink-aac", "aac"));
        assert!(!matches_profile(
            &sink,
            Some(&card),
            "a2dp-sink-aac",
            "ldac"
        ));
        assert!(matches_profile(&sink, Some(&card), "a2dp-sink-aac", "aac"));
    }
}
