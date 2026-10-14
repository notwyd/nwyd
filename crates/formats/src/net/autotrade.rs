use crate::error::{Error, Kind, Result};
use crate::net::account::{ITEM_LEN, Item, item_at};
use crate::util::{u16_at, u32_at};

pub const CONTENTS: u16 = 0x397;

pub const BUY: u16 = 0x398;

pub const BROWSE: u16 = 0x39A;

pub const SLOT_SOLD: u16 = 0x39B;

pub const CLOSE_OWN: u16 = 0x384;

pub const SLOTS: usize = 12;

pub const PANEL_SLOTS: usize = 10;

pub const CONTENTS_FRAME_LEN: usize = 0xF4;

pub const CONTENTS_LEN: usize = CONTENTS_FRAME_LEN - 0x0C;

pub const TITLE_OFF: usize = 0x00;

pub const TITLE_LEN: usize = 24;

pub const TITLE_MAX: usize = 22;

pub const ITEMS_OFF: usize = 0x18;

pub const SOURCE_SLOTS_OFF: usize = ITEMS_OFF + SLOTS * ITEM_LEN;

pub const PRICES_OFF: usize = SOURCE_SLOTS_OFF + SLOTS;

pub const TOKEN_OFF: usize = PRICES_OFF + SLOTS * 4;

pub const OWNER_OFF: usize = TOKEN_OFF + 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contents {
    pub title: [u8; TITLE_LEN],
    pub items: [Item; SLOTS],
    pub source_slots: [u8; SLOTS],
    pub prices: [i32; SLOTS],
    pub token: u16,
    pub owner: u16,
}

impl Default for Contents {
    fn default() -> Contents {
        Contents {
            title: [0; TITLE_LEN],
            items: [Item::default(); SLOTS],
            source_slots: [0; SLOTS],
            prices: [0; SLOTS],
            token: 0,
            owner: 0,
        }
    }
}

impl Contents {
    pub fn title(&self) -> String {
        decode(&self.title[..TITLE_MAX])
    }

    pub fn panel_slots(&self) -> impl Iterator<Item = (usize, Item, i32)> + '_ {
        (0..PANEL_SLOTS).map(|n| (n, self.items[n], self.prices[n]))
    }

    pub fn filled(&self) -> usize {
        self.items[..PANEL_SLOTS]
            .iter()
            .filter(|item| !item.is_empty())
            .count()
    }

    pub fn to_bytes(&self) -> [u8; CONTENTS_LEN] {
        let mut out = [0u8; CONTENTS_LEN];

        out[TITLE_OFF..TITLE_OFF + TITLE_LEN].copy_from_slice(&self.title);

        for (n, item) in self.items.iter().enumerate() {
            let at = ITEMS_OFF + n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out[SOURCE_SLOTS_OFF..SOURCE_SLOTS_OFF + SLOTS].copy_from_slice(&self.source_slots);

        for (n, price) in self.prices.iter().enumerate() {
            let at = PRICES_OFF + n * 4;

            out[at..at + 4].copy_from_slice(&price.to_le_bytes());
        }

        out[TOKEN_OFF..TOKEN_OFF + 2].copy_from_slice(&self.token.to_le_bytes());

        out[OWNER_OFF..OWNER_OFF + 2].copy_from_slice(&self.owner.to_le_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Contents> {
    if payload.len() < CONTENTS_LEN {
        return Err(Error::Truncated {
            kind: Kind::AutoTrade,
            offset: 0,
            needed: CONTENTS_LEN,
            len: payload.len(),
        });
    }

    let mut title = [0u8; TITLE_LEN];

    title.copy_from_slice(&payload[TITLE_OFF..TITLE_OFF + TITLE_LEN]);

    let mut items = [Item::default(); SLOTS];

    for (n, item) in items.iter_mut().enumerate() {
        *item = item_at(payload, ITEMS_OFF + n * ITEM_LEN)?;
    }

    let mut source_slots = [0u8; SLOTS];

    source_slots.copy_from_slice(&payload[SOURCE_SLOTS_OFF..SOURCE_SLOTS_OFF + SLOTS]);

    let mut prices = [0i32; SLOTS];

    for (n, price) in prices.iter_mut().enumerate() {
        *price = u32_at(payload, PRICES_OFF + n * 4, Kind::AutoTrade)? as i32;
    }

    Ok(Contents {
        title,
        items,
        source_slots,
        prices,
        token: u16_at(payload, TOKEN_OFF, Kind::AutoTrade)?,
        owner: u16_at(payload, OWNER_OFF, Kind::AutoTrade)?,
    })
}

pub const BUY_FRAME_LEN: usize = 0x28;

pub const BUY_LEN: usize = BUY_FRAME_LEN - 0x0C;

pub const BUY_SLOT_OFF: usize = 0x00;

pub const BUY_OWNER_OFF: usize = 0x04;

pub const BUY_PRICE_OFF: usize = 0x08;

pub const BUY_TOKEN_OFF: usize = 0x0C;

pub const BUY_ITEM_OFF: usize = 0x10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Buy {
    pub slot: i32,
    pub owner: u32,
    pub price: u32,
    pub token: u32,
    pub item: Item,
}

impl Buy {
    pub fn to_bytes(&self) -> [u8; BUY_LEN] {
        let mut out = [0u8; BUY_LEN];

        out[BUY_SLOT_OFF..BUY_SLOT_OFF + 4].copy_from_slice(&self.slot.to_le_bytes());

        out[BUY_OWNER_OFF..BUY_OWNER_OFF + 4].copy_from_slice(&self.owner.to_le_bytes());

        out[BUY_PRICE_OFF..BUY_PRICE_OFF + 4].copy_from_slice(&self.price.to_le_bytes());

        out[BUY_TOKEN_OFF..BUY_TOKEN_OFF + 4].copy_from_slice(&self.token.to_le_bytes());

        out[BUY_ITEM_OFF..BUY_ITEM_OFF + ITEM_LEN].copy_from_slice(&self.item.to_bytes());

        out
    }

    pub fn of(contents: &Contents, slot: usize) -> Option<Buy> {
        if slot >= PANEL_SLOTS {
            return None;
        }

        Some(Buy {
            slot: slot as i32,
            owner: u32::from(contents.owner),
            price: contents.prices[slot] as u32,
            token: u32::from(contents.token),
            item: contents.items[slot],
        })
    }
}

pub const SOLD_FRAME_LEN: usize = 0x14;

pub const SOLD_LEN: usize = SOLD_FRAME_LEN - 0x0C;

pub const SOLD_OWNER_OFF: usize = 0x00;

pub const SOLD_SLOT_OFF: usize = 0x04;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sold {
    pub owner: u32,
    pub slot: i32,
}

impl Sold {
    pub fn slot_is_drawable(&self) -> bool {
        self.slot >= 0 && (self.slot as usize) < PANEL_SLOTS
    }

    pub fn to_bytes(&self) -> [u8; SOLD_LEN] {
        let mut out = [0u8; SOLD_LEN];

        out[SOLD_OWNER_OFF..SOLD_OWNER_OFF + 4].copy_from_slice(&self.owner.to_le_bytes());

        out[SOLD_SLOT_OFF..SOLD_SLOT_OFF + 4].copy_from_slice(&self.slot.to_le_bytes());

        out
    }
}

pub fn parse_sold(payload: &[u8]) -> Result<Sold> {
    if payload.len() < SOLD_LEN {
        return Err(Error::Truncated {
            kind: Kind::AutoTrade,
            offset: 0,
            needed: SOLD_LEN,
            len: payload.len(),
        });
    }

    Ok(Sold {
        owner: u32_at(payload, SOLD_OWNER_OFF, Kind::AutoTrade)?,
        slot: u32_at(payload, SOLD_SLOT_OFF, Kind::AutoTrade)? as i32,
    })
}

fn decode(run: &[u8]) -> String {
    let end = run.iter().position(|&b| b == 0).unwrap_or(run.len());

    crate::text::codepage::decode(&run[..end], crate::text::Codepage::Windows1252)
}
