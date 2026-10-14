use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::account::{ITEM_LEN, Item, item_at};
use crate::util::{u16_at, u32_at};

const KIND: ErrorKind = ErrorKind::AccountInfo;

pub const STOCK_SLOTS: usize = 27;

pub const KIND_OFF: usize = 0;

pub const ITEMS_OFF: usize = 4;

pub const COPIED_LEN: usize = 0x56 * 4 - 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shop,
    SkillMaster,
    Other(u32),
}

impl Kind {
    pub fn of(value: u32) -> Kind {
        match value {
            1 => Kind::Shop,
            3 => Kind::SkillMaster,
            other => Kind::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stock {
    pub kind: Kind,
    pub items: [Item; STOCK_SLOTS],
    pub tail: [u8; 4],
}

impl Stock {
    pub fn opens_shop(&self) -> bool {
        self.kind == Kind::Shop
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; COPIED_LEN];

        let kind = match self.kind {
            Kind::Shop => 1,
            Kind::SkillMaster => 3,
            Kind::Other(value) => value,
        };

        out[KIND_OFF..KIND_OFF + 4].copy_from_slice(&kind.to_le_bytes());

        for (n, item) in self.items.iter().enumerate() {
            let at = ITEMS_OFF + n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        let at = ITEMS_OFF + STOCK_SLOTS * ITEM_LEN;

        out[at..at + 4].copy_from_slice(&self.tail);

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Stock> {
    if payload.len() < COPIED_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: COPIED_LEN,
            len: payload.len(),
        });
    }

    let mut items = [Item::default(); STOCK_SLOTS];

    for (n, item) in items.iter_mut().enumerate() {
        let at = ITEMS_OFF + n * ITEM_LEN;

        *item = crate::net::account::item_at(payload, at)?;
    }

    let at = ITEMS_OFF + STOCK_SLOTS * ITEM_LEN;

    Ok(Stock {
        kind: Kind::of(u32_at(payload, KIND_OFF, KIND)?),
        items,
        tail: [
            payload[at],
            payload[at + 1],
            payload[at + 2],
            payload[at + 3],
        ],
    })
}

pub const SALE_SHOP_OFF: usize = 0x00;

pub const SALE_CONTAINER_OFF: usize = 0x02;

pub const SALE_SLOT_OFF: usize = 0x04;

pub const SALE_FRAME_LEN: usize = 0x14;

pub const SALE_PAYLOAD_LEN: usize = SALE_FRAME_LEN - 0x0C;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sale {
    pub shop: u16,
    pub container: i16,
    pub slot: i16,
}

impl Sale {
    pub fn to_bytes(&self) -> [u8; SALE_PAYLOAD_LEN] {
        let mut out = [0u8; SALE_PAYLOAD_LEN];

        out[SALE_SHOP_OFF..SALE_SHOP_OFF + 2].copy_from_slice(&self.shop.to_le_bytes());

        out[SALE_CONTAINER_OFF..SALE_CONTAINER_OFF + 2]
            .copy_from_slice(&self.container.to_le_bytes());

        out[SALE_SLOT_OFF..SALE_SLOT_OFF + 2].copy_from_slice(&self.slot.to_le_bytes());

        out
    }
}

pub fn parse_sale(payload: &[u8]) -> Result<Sale> {
    if payload.len() < SALE_PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: SALE_PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    Ok(Sale {
        shop: u16_at(payload, SALE_SHOP_OFF, KIND)?,
        container: u16_at(payload, SALE_CONTAINER_OFF, KIND)? as i16,
        slot: u16_at(payload, SALE_SLOT_OFF, KIND)? as i16,
    })
}

pub const SELL_LIST: u16 = 0x3E8;

pub const SELL_SLOTS: usize = 10;

pub const SELL_ROW_LEN: usize = 0x14;

pub const SELL_PAYLOAD_LEN: usize = SELL_SLOTS * SELL_ROW_LEN;

pub const SELL_FRAME_LEN: usize = SELL_PAYLOAD_LEN + 0x0C;

pub const SELL_ITEM_OFF: usize = 0x04;

pub const SELL_TABLE_OFF: u32 = 0x291DA;

pub const SELL_MODE_OFF: u32 = 0x292A2;

pub const SELL_MODE: u32 = 1;

pub const SELL_ID_LIMIT: i32 = 0x38A3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SellRow {
    pub head: u32,
    pub item: Item,
    pub tail: u32,
}

impl SellRow {
    pub fn copied(&self) -> bool {
        (self.item.index as i16) > 0
    }

    pub fn drawn(&self) -> bool {
        let index = i32::from(self.item.index as i16);

        (1..=SELL_ID_LIMIT).contains(&index)
    }

    pub fn to_bytes(&self) -> [u8; SELL_ROW_LEN] {
        let mut out = [0u8; SELL_ROW_LEN];

        out[0..4].copy_from_slice(&self.head.to_le_bytes());

        out[SELL_ITEM_OFF..SELL_ITEM_OFF + ITEM_LEN].copy_from_slice(&self.item.to_bytes());

        out[0x10..0x14].copy_from_slice(&self.tail.to_le_bytes());

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellList {
    pub rows: [SellRow; SELL_SLOTS],
}

impl Default for SellList {
    fn default() -> Self {
        SellList {
            rows: [SellRow::default(); SELL_SLOTS],
        }
    }
}

impl SellList {
    pub fn filled(&self) -> usize {
        self.rows.iter().filter(|row| !row.item.is_empty()).count()
    }

    pub fn drawn(&self) -> usize {
        self.rows.iter().filter(|row| row.drawn()).count()
    }

    pub fn to_payload(&self) -> [u8; SELL_PAYLOAD_LEN] {
        let mut out = [0u8; SELL_PAYLOAD_LEN];

        for (n, row) in self.rows.iter().enumerate() {
            out[n * SELL_ROW_LEN..][..SELL_ROW_LEN].copy_from_slice(&row.to_bytes());
        }

        out
    }
}

pub fn parse_sell_list(payload: &[u8]) -> Result<SellList> {
    if payload.len() < SELL_PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: SELL_PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    let mut list = SellList::default();

    for (n, row) in list.rows.iter_mut().enumerate() {
        let at = n * SELL_ROW_LEN;

        let candidate = SellRow {
            head: u32_at(payload, at, KIND)?,
            item: item_at(payload, at + SELL_ITEM_OFF)?,
            tail: u32_at(payload, at + 0x10, KIND)?,
        };

        if candidate.copied() {
            *row = candidate;
        }
    }

    Ok(list)
}
