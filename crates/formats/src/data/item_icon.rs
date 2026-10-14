use super::ITEM_SLOTS;

pub const SLOTS: usize = ITEM_SLOTS;

pub const BUFFER_LEN: usize = SLOTS * 4;

pub const SHIPPED_ENTRIES: usize = 6_500;

pub const CELL: u32 = 100;

pub const CELLS_PER_SHEET: i32 = 100;

pub const OVERRIDE_ITEM: usize = 0x1911;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Icon {
    pub sheet: i32,
    pub u0: u32,
    pub v0: u32,
    pub u1: u32,
    pub v1: u32,
}

impl Icon {
    pub fn file(&self) -> String {
        format!("UI/ItemIcon{:02}.wyt", self.sheet)
    }

    pub const FALLBACK: Icon = Icon {
        sheet: 1,
        u0: 0,
        v0: 0,
        u1: CELL,
        v1: CELL,
    };
}

#[derive(Debug, Clone)]

pub struct IconTable {
    entries: Vec<i32>,
}

pub fn parse(bytes: &[u8]) -> IconTable {
    let mut entries = vec![0i32; SLOTS];

    for (i, w) in bytes.chunks_exact(4).take(SLOTS).enumerate() {
        entries[i] = i32::from_le_bytes([w[0], w[1], w[2], w[3]]);
    }

    IconTable { entries }
}

impl IconTable {
    pub fn index(&self, item_id: usize) -> i32 {
        self.entries.get(item_id).copied().unwrap_or(0)
    }

    pub fn resolve(&self, item_id: usize, overridden: bool) -> Icon {
        let stored = self.index(item_id);

        let n = if item_id == OVERRIDE_ITEM && overridden {
            stored
        } else {
            stored - 1
        };

        if n < 0 {
            return Icon::FALLBACK;
        }

        let within = n % CELLS_PER_SHEET;

        let u0 = (within % 10) as u32 * CELL;

        let v0 = (within / 10) as u32 * CELL;

        Icon {
            sheet: n / CELLS_PER_SHEET + 1,
            u0,
            v0,
            u1: u0 + CELL,
            v1: v0 + CELL,
        }
    }

    pub fn populated(&self) -> usize {
        self.entries.iter().filter(|&&n| n > 0).count()
    }

    pub fn max_index(&self) -> i32 {
        self.entries.iter().copied().max().unwrap_or(0)
    }

    pub fn as_slice(&self) -> &[i32] {
        &self.entries
    }
}
