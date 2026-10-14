pub const SERIES_SHIFT: u32 = 12;

pub const GUILD_MASK: u16 = 0x0FFF;

pub const URL_RECORD: usize = 377;

pub const FILE_FORMAT: &str = "%c%02d%02d%04d.bmp";

pub const FILE_PREFIX: char = 'w';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Mark {
    pub series: u16,
    pub guild: u16,
}

impl Mark {
    pub const fn split(guild_id: u16) -> Self {
        Mark {
            series: guild_id >> SERIES_SHIFT,
            guild: guild_id & GUILD_MASK,
        }
    }

    pub const fn absent(guild_id: u16) -> bool {
        guild_id == 0
    }
}

pub fn file_name(server: u32, mark: Mark) -> String {
    format!(
        "{FILE_PREFIX}{server:02}{:02}{:04}.bmp",
        mark.series, mark.guild
    )
}

pub const fn kind(rank: i16) -> u16 {
    match rank {
        2 => 0x20F,
        3 => 0x20E,
        4 => 0x1FD,
        _ => 0x1FC,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Frame {
    Gold,
    Silver,
    None,
}

pub const FRAME_GOLD: u32 = 0xFFFF_D700;

pub const FRAME_SILVER: u32 = 0xFFC0_C0C0;

pub const fn frame(kind: u16) -> Frame {
    match kind {
        0x1FD | 9 => Frame::Gold,
        0x20E..=0x213 => Frame::Silver,
        3..=8 => Frame::Silver,
        _ => Frame::None,
    }
}

pub const fn frame_downloaded(kind: u16) -> Frame {
    match kind {
        0x1FD => Frame::Gold,
        0x20E..=0x213 => Frame::Silver,
        _ => Frame::None,
    }
}

pub const BMP_MAGIC: u16 = 0x4D42;

pub const BMP_LEN: [u32; 2] = [0x276, 0x278];

pub const BMP_WIDTH: i32 = 16;

pub const BMP_HEIGHT: i32 = 12;

pub const BMP_BITS: u16 = 24;

pub const READ_MAX: usize = 0x278;

pub fn validate(bytes: &[u8]) -> bool {
    if bytes.len() < 0x36 {
        return false;
    }

    let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);

    let dword =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);

    if word(0) != BMP_MAGIC || !BMP_LEN.contains(&dword(2)) {
        return false;
    }

    let off_bits = dword(0xA) as usize;

    if off_bits < 0x1E || off_bits > bytes.len() {
        return false;
    }

    dword(0x12) as i32 == BMP_WIDTH && dword(0x16) as i32 == BMP_HEIGHT && word(0x1C) == BMP_BITS
}

pub fn decode(bytes: &[u8]) -> Option<Vec<u8>> {
    if !validate(bytes) {
        return None;
    }

    let dword =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);

    let start = dword(0xA) as usize;

    let stride = (BMP_WIDTH as usize * 3).next_multiple_of(4);

    if start + stride * BMP_HEIGHT as usize > bytes.len() {
        return None;
    }

    let mut out = Vec::with_capacity(BMP_WIDTH as usize * BMP_HEIGHT as usize * 4);

    for y in (0..BMP_HEIGHT as usize).rev() {
        let row = start + y * stride;

        for x in 0..BMP_WIDTH as usize {
            let at = row + x * 3;

            out.extend_from_slice(&[bytes[at + 2], bytes[at + 1], bytes[at], 0xFF]);
        }
    }

    Some(out)
}
