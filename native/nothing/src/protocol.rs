// SPDX-License-Identifier: AGPL-3.0-only
// Framing adapted from Daan Hessen's earctl; see ../NOTICE.md.
#[derive(Debug)]
pub struct Packet {
    pub command: u16,
    pub id: u8,
    pub payload: Vec<u8>,
}
pub fn crc(bytes: &[u8]) -> u16 {
    let mut value = 0xffff;
    for byte in bytes {
        value ^= u16::from(*byte);
        for _ in 0..8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xa001
            } else {
                value >> 1
            };
        }
    }
    value
}
pub fn encode(command: u16, id: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x55, 0x60, 1];
    bytes.extend(command.to_le_bytes());
    bytes.extend((payload.len() as u16).to_le_bytes());
    bytes.push(id);
    bytes.extend(payload);
    bytes.extend(crc(&bytes).to_le_bytes());
    bytes
}
pub fn parse(buffer: &mut Vec<u8>) -> Option<Packet> {
    let mut offset = 0;
    while buffer.len() - offset >= 8 {
        let bytes = &buffer[offset..];
        if bytes[..3] != [0x55, 0x60, 1] {
            offset += 1;
            continue;
        }
        let length = usize::from(u16::from_le_bytes([bytes[5], bytes[6]]));
        if length > 4096 {
            offset += 1;
            continue;
        }
        let total = length + 10;
        if bytes.len() < total {
            break;
        }
        if crc(&bytes[..total - 2]) != u16::from_le_bytes([bytes[total - 2], bytes[total - 1]]) {
            offset += 1;
            continue;
        }
        let packet = Packet {
            command: u16::from_le_bytes([bytes[3], bytes[4]]),
            id: bytes[7],
            payload: bytes[8..total - 2].to_vec(),
        };
        buffer.drain(..offset + total);
        return Some(packet);
    }
    // Discard noise once, preserving an incomplete header/frame. Repeated
    // remove(0) moved the remaining buffer on every bad byte (quadratic work).
    buffer.drain(..offset);
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_bad_crc_and_large_payload() {
        let good = encode(0x4007, 24, &vec![6; 300]);
        let mut bad = encode(0x4007, 1, &[2, 3]);
        bad[8] ^= 1;
        let mut buffer = vec![3, 5, 0x55];
        buffer.extend(bad);
        buffer.extend(&good[..20]);
        assert!(parse(&mut buffer).is_none());
        buffer.extend(&good[20..]);
        let packet = parse(&mut buffer).unwrap();
        assert_eq!(packet.id, 24);
        assert_eq!(packet.payload.len(), 300);
        assert!(buffer.is_empty());
    }
    #[test]
    fn long_noise_and_partial_headers_resynchronize_without_losing_packets() {
        let mut buffer = vec![0xaa; 1024 * 1024];
        buffer.extend([0x55, 0x60]);
        assert!(parse(&mut buffer).is_none());
        assert!(buffer.len() <= 7);
        let frame = encode(0x4007, 5, &[1, 2, 3]);
        buffer.extend(&frame[2..]);
        let packet = parse(&mut buffer).unwrap();
        assert_eq!(packet.command, 0x4007);
        assert_eq!(packet.payload, [1, 2, 3]);
        assert!(buffer.is_empty());
    }
}
