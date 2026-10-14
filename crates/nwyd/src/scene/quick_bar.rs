use super::consume::FeedSend;
use super::enter::Entered;
use super::item_move::{self, Cooldowns, Coord, Request};
use super::item_use;
use assets::GameDir;
use formats::data::item_effect::Effects;
use formats::data::item_list::ItemList;
use formats::data::quick_slot;
use formats::text::Codepage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct QuickBar {
    pub record: Option<usize>,
    pub ids: [u32; quick_slot::SLOTS],
    pub selected: Option<usize>,
    pub save: Option<Vec<u8>>,
    pub notes: Vec<String>,
    records: Vec<quick_slot::Slot>,
}

pub const FILE: &[u8] = b"NQuickSLot_01.bin";

impl QuickBar {
    pub fn none() -> QuickBar {
        QuickBar::default()
    }

    pub fn bind(&mut self, display_slot: usize, cache_slot: usize, id: u32) {
        let Some(display) = self.ids.get_mut(display_slot) else {
            return;
        };

        *display = id;

        self.write(cache_slot, id);
    }

    pub fn clear(&mut self, slot: usize) -> bool {
        let Some(display) = self.ids.get_mut(slot) else {
            return false;
        };

        *display = 0;

        if !self.write(slot, 0) {
            return false;
        }

        self.selected = None;

        true
    }

    pub(super) fn write(&mut self, slot: usize, id: u32) -> bool {
        let Some(record) = self.record else {
            return false;
        };

        let Some(value) = self
            .records
            .get_mut(record)
            .and_then(|record| record.slot.get_mut(slot))
        else {
            return false;
        };

        *value = id;

        self.save = Some(quick_slot::to_bytes(&self.records));

        true
    }
}

pub fn load(game: &GameDir, char_name: &str, now: u32) -> QuickBar {
    let mut bar = QuickBar::none();

    let mut slots = match crate::user_paths::read_quick_slots(game) {
        Ok(raw) => match quick_slot::parse(&raw, Codepage::Windows1252) {
            Ok(slots) => slots,
            Err(error) => {
                bar.notes.push(format!(
                    "{} did not parse ({error}); starting from empty",
                    name()
                ));

                vec![quick_slot::Slot::default(); quick_slot::RECORDS]
            }
        },
        Err(error) => {
            bar.notes.push(format!(
                "{} could not be read ({error}); starting from empty",
                name()
            ));

            vec![quick_slot::Slot::default(); quick_slot::RECORDS]
        }
    };

    let Some(index) = quick_slot::claim(&mut slots, char_name, now) else {
        bar.notes.push(format!(
            "{} has no record older than the clock; the bar stays empty",
            name()
        ));

        return bar;
    };

    bar.ids = slots[index].slot;

    bar.record = Some(index);

    bar.save = Some(quick_slot::to_bytes(&slots));

    bar.records = slots;

    bar
}

pub fn hotkey(ch: char) -> Option<usize> {
    ["qQ", "wW", "eE", "rR", "tT"]
        .iter()
        .position(|pair| pair.contains(ch))
}

pub fn use_slot(
    bar: &QuickBar,
    player: &mut Entered,
    items: &ItemList,
    effects: &Effects<'_>,
    cooldowns: &mut Cooldowns,
    slot: usize,
    now_ms: u32,
    at: (u16, u16),
) -> Vec<item_use::Action> {
    let Some(&id) = bar.ids.get(slot) else {
        return Vec::new();
    };

    if !(1..=super::shop::MAX_TEMPLATE_ID as u32).contains(&id) {
        return Vec::new();
    }

    let Some(bag_slot) = player
        .carry
        .iter()
        .take(item_move::INVENTORY_SLOTS)
        .position(|item| u32::from(item.index) == id)
    else {
        return Vec::new();
    };

    let cell = Coord {
        container: item_move::container::INVENTORY,
        slot: bag_slot as i32,
    };

    let item = player.carry[bag_slot];

    let kind = effects.total_scaled(&item, item_move::EFFECT_KIND);

    if cooldowns.running(kind, now_ms) {
        return Vec::new();
    }

    let sent = match kind == item_use::MOUNT_FOOD_KIND {
        true => item_use::Action::Feed(FeedSend {
            slot: cell.slot,
            at,
        }),
        false => item_use::Action::Send(Request::Use {
            slot: cell.slot,
            at,
        }),
    };

    cooldowns.stamp(kind, now_ms);

    item_use::spend(player, items, cell);

    vec![
        sent,
        item_use::Action::Sound { slot: 0x29 },
        item_use::Action::ItemChanged { at: cell },
    ]
}

fn name() -> String {
    String::from_utf8_lossy(FILE).into_owned()
}

pub fn cooldown_handled(
    bar: &QuickBar,
    player: &Entered,
    effects: &Effects<'_>,
    cooldowns: &Cooldowns,
    slot: usize,
    now_ms: u32,
) -> bool {
    let Some(&id) = bar.ids.get(slot) else {
        return false;
    };

    if !(1..=super::shop::MAX_TEMPLATE_ID as u32).contains(&id) {
        return false;
    }

    player
        .carry
        .iter()
        .take(item_move::INVENTORY_SLOTS)
        .find(|item| u32::from(item.index) == id)
        .is_some_and(|item| {
            cooldowns.running(effects.total_scaled(item, item_move::EFFECT_KIND), now_ms)
        })
}
