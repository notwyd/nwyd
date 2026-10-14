use crate::scene::enter::Entered;
use crate::scene::item_move::{self, Cooldowns, Coord, EFFECT_KIND, Request, container, item_at};
use formats::data::item_effect::Effects;
use formats::data::item_list::ItemList;
use formats::data::mount;
use formats::net::account::Item;

pub const HP_POTION_KEYS: [char; 2] = ['q', 'Q'];

pub const MP_POTION_KEYS: [char; 2] = ['w', 'W'];

pub const FEED_KEYS: [char; 2] = ['v', 'V'];

pub const ALIVE_GATES_THE_POTIONS: bool = true;

pub const HUD_COMMAND_HP: u32 = 0x415;

pub const HUD_COMMAND_MP: u32 = 0x416;

pub const HUD_COMMAND_INERT: u32 = 0x417;

pub const MENU_COMMAND_FEED: u32 = 0x9CA4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Which {
    Hp,
    Mp,
}

impl Which {
    pub fn ids(self) -> &'static [(i16, i16)] {
        match self {
            Which::Hp => &HP_POTION_IDS,
            Which::Mp => &MP_POTION_IDS,
        }
    }

    pub fn group(self) -> usize {
        match self {
            Which::Hp => HP_GROUP,
            Which::Mp => MP_GROUP,
        }
    }

    pub fn keys(self) -> [char; 2] {
        match self {
            Which::Hp => HP_POTION_KEYS,
            Which::Mp => MP_POTION_KEYS,
        }
    }
}

pub const HP_POTION_IDS: [(i16, i16); 7] = [
    (0x1001, 0x1001),
    (0x0D67, 0x0D67),
    (0x0CFA, 0x0CFA),
    (0x0D95, 0x0D95),
    (0x0190, 0x0194),
    (0x01AC, 0x01AF),
    (0x02A8, 0x02AD),
];

pub const MP_POTION_IDS: [(i16, i16); 5] = [
    (0x0CFB, 0x0CFB),
    (0x0D90, 0x0D90),
    (0x0195, 0x0199),
    (0x01B0, 0x01B3),
    (0x02AE, 0x02B3),
];

pub const POTION_KIND: i32 = 1;

pub fn whitelisted(which: Which, id: i16) -> bool {
    which
        .ids()
        .iter()
        .any(|&(low, high)| (low..=high).contains(&id))
}

pub const COOLDOWN_GROUP_JUMP: [usize; 5] = [0, 1, 4, 3, 2];

pub const COOLDOWN_GROUP_TABLE: [(i32, usize); 5] =
    [(0x01, 0), (0x0F, 1), (0xB8, 4), (0xF2, 3), (0xF3, 3)];

pub const COOLDOWN_GROUP_DEFAULT: usize = 2;

pub const COOLDOWN_GROUP_DOMAIN: std::ops::RangeInclusive<i32> = 1..=0xF3;

pub const COOLDOWN_GROUP_MS_BYTES: [u8; 20] = [
    0xFA, 0x00, 0x00, 0x00, 0xF4, 0x01, 0x00, 0x00, 0x2C, 0x01, 0x00, 0x00, 0x2C, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

pub const HP_GROUP: usize = 0;

pub const MP_GROUP: usize = 2;

pub const FEED_GROUP: usize = 1;

pub const WRAP_ESCAPE_FEED: bool = true;

pub const WRAP_ESCAPE_POTION: bool = false;

fn group_running(cooldowns: &Cooldowns, group: usize, now_ms: u32, wrap_escape: bool) -> bool {
    let (Some(&last), Some(&window)) =
        (cooldowns.last.get(group), item_move::COOLDOWN_MS.get(group))
    else {
        return false;
    };

    if last == 0 {
        return false;
    }

    if wrap_escape && now_ms < last {
        return false;
    }

    now_ms.wrapping_sub(last) < window
}

fn stamp_group(cooldowns: &mut Cooldowns, group: usize, now_ms: u32) {
    if let Some(slot) = cooldowns.last.get_mut(group) {
        *slot = now_ms;
    }
}

pub const FEED_DST_SLOT_OFFSET: usize = item_move::HEADER_LEN + 3 * 4;

pub const FEED_DST_SLOT: i32 = 0x0E;

pub const POTION_SOUND: i64 = 0x29;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum Stop {
    #[default]
    Exhausted,
    Affect20,
    Cooldown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct Drunk {
    pub sent: Vec<(i32, Request)>,
    pub emptied: usize,
    pub refreshes: usize,
    pub stopped: Stop,
}

pub const POTION_ANSWER: bool = false;

impl Drunk {
    pub fn answer(&self) -> bool {
        POTION_ANSWER
    }

    pub fn describe(&self, which: Which) -> Option<String> {
        let (slot, _) = self.sent.first()?;

        let what = match which {
            Which::Hp => "HP",
            Which::Mp => "MP",
        };

        Some(format!(
            "NWYD: drank an {what} potion from slot {slot} ({} frame(s), {} slot(s) emptied)",
            self.sent.len(),
            self.emptied,
        ))
    }
}

pub fn potion(
    player: &mut Entered,
    items: &ItemList,
    live: &super::item_use::Live,
    cooldowns: &mut Cooldowns,
    now_ms: u32,
    which: Which,
) -> Drunk {
    let mut out = Drunk::default();

    let effects = Effects::new(items, &[]);

    let group = which.group();

    for slot in 0..item_move::INVENTORY_SLOTS as i32 {
        let at = Coord {
            container: container::INVENTORY,
            slot,
        };

        let Some(item) = item_at(player, at) else {
            continue;
        };

        let id = item.index as i16;

        if id == 0 {
            continue;
        }

        if !whitelisted(which, id) {
            continue;
        }

        let kind = effects.total_scaled(&item, EFFECT_KIND);

        if kind != POTION_KIND {
            out.refreshes += 1;

            continue;
        }

        if live.affect_20 {
            out.stopped = Stop::Affect20;

            return out;
        }

        if group_running(cooldowns, group, now_ms, WRAP_ESCAPE_POTION) {
            out.stopped = Stop::Cooldown;

            return out;
        }

        out.sent.push((
            slot,
            Request::Use {
                slot,
                at: live.position,
            },
        ));

        stamp_group(cooldowns, group, now_ms);

        if spend(player, slot) {
            out.emptied += 1;
        }

        out.refreshes += 1;
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct FeedSend {
    pub slot: i32,
    pub at: (u16, u16),
}

impl FeedSend {
    pub fn frame(&self, client_id: u16) -> Vec<u8> {
        let mut out = Request::Use {
            slot: self.slot,
            at: self.at,
        }
        .to_bytes(client_id, item_move::ANTI_CHEAT_TOKEN);

        if let Some(field) = out.get_mut(FEED_DST_SLOT_OFFSET..FEED_DST_SLOT_OFFSET + 4) {
            field.copy_from_slice(&FEED_DST_SLOT.to_le_bytes());
        }

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Fed {
    pub sent: Option<FeedSend>,
    pub emptied: bool,
    pub mismatched: Option<(i32, i32)>,
    pub refused_by_cooldown: bool,
}

impl Fed {
    pub fn answer(&self) -> bool {
        self.sent.is_some() || self.mismatched.is_some()
    }

    pub fn describe(&self) -> Option<String> {
        if let Some(send) = self.sent {
            return Some(format!("NWYD: fed the mount from slot {}", send.slot));
        }

        let (slot, kind) = self.mismatched?;

        Some(format!(
            "NWYD: slot {slot} pairs with the mount but its effect(0x26) is {kind:#04x}, \
             not {:#04x} — no packet sent; ends the macro tick",
            mount::FEED_ITEM_KIND,
        ))
    }
}

pub fn feed(
    player: &mut Entered,
    items: &ItemList,
    cooldowns: &mut Cooldowns,
    now_ms: u32,
    position: (u16, u16),
) -> Fed {
    let mut out = Fed::default();

    let effects = Effects::new(items, &[]);

    let mount_id = player.equipment.mount().index as i16;

    for slot in 0..item_move::INVENTORY_SLOTS as i32 {
        let at = Coord {
            container: container::INVENTORY,
            slot,
        };

        let Some(item) = item_at(player, at) else {
            continue;
        };

        let id = item.index as i16;

        if id == 0 {
            continue;
        }

        if !mount::feeds(mount_id, id) {
            continue;
        }

        let kind = effects.total_scaled(&item, EFFECT_KIND);

        if kind != mount::FEED_ITEM_KIND {
            out.mismatched = Some((slot, kind));

            return out;
        }

        if group_running(cooldowns, FEED_GROUP, now_ms, WRAP_ESCAPE_FEED) {
            out.refused_by_cooldown = true;

            return out;
        }

        out.sent = Some(FeedSend { slot, at: position });

        stamp_group(cooldowns, FEED_GROUP, now_ms);

        out.emptied = spend(player, slot);

        return out;
    }

    out
}

pub fn stored_count(item: &Item) -> i32 {
    let amount = i32::from(item.appearance as i16);

    if amount > ui::icon::MAX_STACK || amount <= 0 {
        1
    } else {
        amount
    }
}

fn spend(player: &mut Entered, slot: i32) -> bool {
    let Ok(index) = usize::try_from(slot) else {
        return false;
    };

    let Some(item) = player.carry.get(index).copied() else {
        return false;
    };

    let count = stored_count(&item);

    let Some(cell) = player.carry.get_mut(index) else {
        return false;
    };

    if count < 2 {
        *cell = Item::default();

        return true;
    }

    cell.appearance = (count - 1) as u16;

    false
}
