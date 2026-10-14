pub mod arena_visual;
pub mod cdata;
pub mod collection;
pub mod curse;
pub mod envelope;
pub mod event_coupon;
pub mod guild_mark;
pub mod init_item;
pub mod item_effect;
pub mod item_help;
pub mod item_icon;
pub mod item_list;
pub mod item_name;
pub mod item_price;
pub mod item_set;
pub mod mission;
pub mod mitem;
pub mod mix_list;
pub mod mount;
pub mod quick_slot;
pub mod skill_data;

pub use envelope::{Integrity, nine_step_checksum, seven_step_checksum, xor_5a};
pub use item_effect::Effects;
pub use item_list::ItemList;
pub use mix_list::MixList;

pub const ITEM_SLOTS: usize = 14_500;

pub const ITEM_RECORD_LEN: usize = 172;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemEffect {
    pub id: u8,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Item {
    pub index: u16,
    pub ex1: u16,
    pub ex2: u16,
    pub effect: [ItemEffect; 3],
}

impl Item {
    pub const LEN: usize = 12;

    pub fn parse(bytes: &[u8]) -> Item {
        let Some(b) = bytes.get(..Item::LEN) else {
            return Item::default();
        };

        Item {
            index: u16::from_le_bytes([b[0], b[1]]),
            ex1: u16::from_le_bytes([b[2], b[3]]),
            ex2: u16::from_le_bytes([b[4], b[5]]),
            effect: [
                ItemEffect {
                    id: b[6],
                    value: b[7],
                },
                ItemEffect {
                    id: b[8],
                    value: b[9],
                },
                ItemEffect {
                    id: b[10],
                    value: b[11],
                },
            ],
        }
    }

    pub fn to_bytes(self) -> [u8; Item::LEN] {
        let mut out = [0u8; Item::LEN];

        out[0..2].copy_from_slice(&self.index.to_le_bytes());

        out[2..4].copy_from_slice(&self.ex1.to_le_bytes());

        out[4..6].copy_from_slice(&self.ex2.to_le_bytes());

        for (i, e) in self.effect.iter().enumerate() {
            out[6 + i * 2] = e.id;

            out[7 + i * 2] = e.value;
        }

        out
    }

    pub fn is_empty(self) -> bool {
        self.index == 0
    }
}

pub(crate) fn fixed_string(field: &[u8], codepage: crate::text::Codepage) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());

    crate::text::codepage::decode(&field[..end], codepage)
}

pub mod retail;
