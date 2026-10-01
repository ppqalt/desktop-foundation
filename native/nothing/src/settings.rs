// SPDX-License-Identifier: AGPL-3.0-only
// Model identification and setting codecs adapted from earctl; see ../NOTICE.md.
use crate::upstream::models::{ModelInfo, model_from_serial_prefix, model_from_sku};
use serde_json::{Value, json};
pub fn identify(payload: &[u8]) -> Option<&'static ModelInfo> {
    let text = std::str::from_utf8(payload.get(7..)?).ok()?;
    let serial = text.lines().find_map(|line| {
        let parts: Vec<_> = line.split(',').collect();
        (parts.len() == 3 && parts[1].trim() == "4").then(|| parts[2].trim())
    })?;
    if let Some(model) = model_from_serial_prefix(serial) {
        return Some(model);
    }
    let sku = if serial == "12345678901234567" {
        "01"
    } else if serial.starts_with("MA") {
        match serial.get(6..8)? {
            "22" | "23" => "14",
            "24" => "11200005",
            _ => return None,
        }
    } else if serial.starts_with("SH") || serial.starts_with("13") {
        serial.get(4..6)?
    } else {
        return None;
    };
    model_from_sku(sku)
}
pub fn decode(command: u16, p: &[u8]) -> Option<(&'static str, Value)> {
    Some(match command {
        0xe001 | 0x4007 => {
            let count = usize::from(*p.first()?);
            if p.len() != 1 + count * 2 {
                return None;
            }
            let mut battery = json!({"left":null,"right":null,"case":null,"headphone":null});
            for pair in p[1..].as_chunks::<2>().0 {
                let key = match pair[0] {
                    2 => "left",
                    3 => "right",
                    4 => "case",
                    6 => "headphone",
                    _ => continue,
                };
                let level = pair[1] & 0x7f;
                if level <= 100 {
                    battery[key] = json!({"percent":level,"charging":pair[1]&0x80!=0});
                }
            }
            ("battery", battery)
        }
        0x401e | 0xe003 => {
            let mode = *p.get(1)?;
            if !(1..=5).contains(&mode) && mode != 7 {
                return None;
            }
            ("anc", json!(mode))
        }
        0x401f => {
            let value = *p.first()?;
            if ![0, 1, 2, 3, 5, 6].contains(&value) {
                return None;
            }
            ("eq", json!(value))
        }
        0x4050 => ("listening", json!(*p.first()?)),
        0x400e => {
            let value = *p.get(2)?;
            if value > 1 {
                return None;
            }
            ("inEar", json!(value == 1))
        }
        0x4041 => {
            let value = *p.first()?;
            if ![1, 2].contains(&value) {
                return None;
            }
            ("latency", json!(value == 1))
        }
        0x404e => {
            let enabled = *p.first()?;
            let level = *p.get(1)?;
            if enabled > 1 || level % 2 != 0 || !(2..=10).contains(&level) {
                return None;
            }
            ("bass", json!({"enabled":enabled==1,"level":level/2}))
        }
        0x4042 => (
            "firmware",
            json!(std::str::from_utf8(p).ok()?.trim_matches('\0')),
        ),
        0x404c => {
            let value = *p.first()?;
            if value > 1 {
                return None;
            }
            ("advanced", json!(value == 1))
        }
        0x4044 => {
            let mut bands = vec![];
            for offset in [6, 19, 32] {
                let value = f32::from_le_bytes(p.get(offset..offset + 4)?.try_into().ok()?);
                if !value.is_finite() || !(-6.01..=6.01).contains(&value) {
                    return None;
                }
                bands.push(value);
            }
            ("customEq", json!([bands[2], bands[0], bands[1]]))
        }
        0x4018 => {
            let count = usize::from(*p.first()?);
            if p.len() != 1 + count * 4 {
                return None;
            }
            (
                "gestures",
                json!(
                    p[1..]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|v| json!({"device":v[0],"common":v[1],"kind":v[2],"action":v[3]}))
                        .collect::<Vec<_>>()
                ),
            )
        }
        _ => return None,
    })
}
pub fn custom_eq(bands: &[f32]) -> Vec<u8> {
    let mut p = vec![
        3, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0x75, 0x44, 0xc3, 0xf5, 0x28, 0x3f, 2, 0, 0, 0, 0, 0,
        0xc0, 0x5a, 0x45, 0, 0, 0x80, 0x3f, 0, 0, 0, 0, 0, 0, 0, 0, 0x0c, 0x43, 0xcd, 0xcc, 0x4c,
        0x3f, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let gain = -bands.iter().copied().fold(0.0_f32, f32::max);
    p[1..5].copy_from_slice(&gain.to_le_bytes());
    for (offset, value) in [(6, bands[1]), (19, bands[2]), (32, bands[0])] {
        p[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    p
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_batteries_are_null() {
        let (_, v) = decode(0xe001, &[2, 2, 80, 3, 255]).unwrap();
        assert_eq!(v["left"]["percent"], 80);
        assert!(v["right"].is_null());
        assert!(v["case"].is_null());
    }
    #[test]
    fn rejects_short_and_invalid() {
        assert!(decode(0x400e, &[1]).is_none());
        assert!(decode(0x4044, &[0; 45]).is_some());
        assert!(identify(b"00000001,4,MA12").is_none());
    }
    #[test]
    fn custom_roundtrip() {
        let bands = [-3.0, 1.0, 6.0];
        let (_, v) = decode(0x4044, &custom_eq(&bands)).unwrap();
        assert_eq!(v, json!(bands));
    }
}
