pub const LEN: usize = 0x0C;

pub const MIN_SIZE: usize = 0x0C;

pub const MAX_SIZE: usize = 10_240;

pub const SIZE_OFF: usize = 0x00;

pub const KEY_OFF: usize = 0x02;

pub const CHECKSUM_OFF: usize = 0x03;

pub const OPCODE_OFF: usize = 0x04;

pub const CLIENT_ID_OFF: usize = 0x06;

pub const TICK_OFF: usize = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub size: u16,
    pub key: u8,
    pub checksum: u8,
    pub opcode: u16,
    pub client_id: u16,
    pub tick: u32,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Option<Header> {
        let head: &[u8; LEN] = bytes.get(..LEN)?.try_into().ok()?;

        Some(Header {
            size: u16::from_le_bytes([head[0], head[1]]),
            key: head[KEY_OFF],
            checksum: head[CHECKSUM_OFF],
            opcode: u16::from_le_bytes([head[OPCODE_OFF], head[OPCODE_OFF + 1]]),
            client_id: u16::from_le_bytes([head[CLIENT_ID_OFF], head[CLIENT_ID_OFF + 1]]),
            tick: u32::from_le_bytes([
                head[TICK_OFF],
                head[TICK_OFF + 1],
                head[TICK_OFF + 2],
                head[TICK_OFF + 3],
            ]),
        })
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        out[0..2].copy_from_slice(&self.size.to_le_bytes());

        out[KEY_OFF] = self.key;

        out[CHECKSUM_OFF] = self.checksum;

        out[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&self.opcode.to_le_bytes());

        out[CLIENT_ID_OFF..CLIENT_ID_OFF + 2].copy_from_slice(&self.client_id.to_le_bytes());

        out[TICK_OFF..TICK_OFF + 4].copy_from_slice(&self.tick.to_le_bytes());

        out
    }

    pub fn size_is_legal(size: u16) -> bool {
        (MIN_SIZE..=MAX_SIZE).contains(&(size as usize))
    }
}

pub fn opcode_of(packet: &[u8]) -> Option<u16> {
    let bytes = packet.get(OPCODE_OFF..OPCODE_OFF + 2)?;

    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub fn set_opcode(packet: &mut [u8], opcode: u16) {
    packet[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&opcode.to_le_bytes());
}
