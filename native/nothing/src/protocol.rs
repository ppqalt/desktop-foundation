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
    loop {
        if buffer.len() < 8 {
            return None;
        }
        if buffer[..3] != [0x55, 0x60, 1] {
            buffer.remove(0);
            continue;
        }
        let length = usize::from(u16::from_le_bytes([buffer[5], buffer[6]]));
        if length > 4096 {
            buffer.remove(0);
            continue;
        }
        let total = length + 10;
        if buffer.len() < total {
            return None;
        }
        if crc(&buffer[..total - 2]) != u16::from_le_bytes([buffer[total - 2], buffer[total - 1]]) {
            buffer.remove(0);
            continue;
        }
        let bytes: Vec<_> = buffer.drain(..total).collect();
        return Some(Packet {
            command: u16::from_le_bytes([bytes[3], bytes[4]]),
            id: bytes[7],
            payload: bytes[8..total - 2].to_vec(),
        });
    }
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
}
