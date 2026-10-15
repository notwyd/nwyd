use crate::scene::enter::Entered;
use crate::scene::equip;
use crate::scene::item_move::{self, Cooldowns, Coord, EFFECT_KIND, Request, container, item_at};
use formats::data::item_effect::{self, Effects};
use formats::data::item_list::ItemList;
use formats::net::account::Item;
use ui::text_entry::Mods;

pub mod content {

    pub const RPS_GAME: u32 = 0x23;

    pub const COIN_INPUT: u32 = 0x27;

    pub const ITEM_REFINE: u32 = 0x35;

    pub const OPTION_STONE: u32 = 0x36;
}

pub mod strdef {

    pub const EXCHANGE_GOLD: usize = 149;

    pub const EXCHANGE_POINT: usize = 150;

    pub const REGISTER_TICKET: usize = 175;

    pub const TITLE: usize = 237;

    pub const DETACH_FIRST: usize = 285;

    pub const ABILITY_MASTER: usize = 286;

    pub const EXP_QUEST: usize = 300;

    pub const SEAL_CHARACTER: usize = 350;

    pub const SECOND_CLASS: usize = 356;

    pub const SELL_NAME: usize = 342;

    pub const SELL_CONFIRM: usize = 343;

    pub const SHARE_AMOUNT: usize = 408;
}

pub mod action {

    pub const SELL: u8 = 0x29;

    pub const CONFIRM: u8 = 0x26;

    pub const TICKET: u8 = 0x27;

    pub const EXCHANGE: u8 = 0x28;
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Live {
    pub position: (u16, u16),
    pub affect_20: bool,
    pub move_alpha: f32,
    pub pet: i16,
    pub mode_letter: u8,
    pub last_attack_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Open {
        content: u32,
        slot: i32,
    },
    Refine {
        at: Coord,
    },
    Prompt {
        action: u8,
        strdef: usize,
        slot: i32,
    },
    Toast {
        strdef: usize,
    },
    Send(Request),
    Feed(super::consume::FeedSend),
    TrackUse {
        id: i32,
    },
    Sound {
        slot: i64,
    },
    ItemChanged {
        at: Coord,
    },
    HuntingScroll {
        page: i32,
        request: Request,
    },
    Firecracker {
        slot: i32,
    },
    Seal {
        slot: i32,
    },
    Dispatch {
        kind: i32,
        id: i32,
        slot: i32,
        refused: Refused,
    },
    Stage(Request),
    Unbuilt {
        what: &'static str,
    },
}

pub const MAX_SLOT: i32 = 0x3C;

pub const CARGO_CONTENT: u32 = 0x18;

pub const CARGO_GATE_FIELD: u8 = 0x6F;

pub const CARGO_BLOCKED_IDS: [u16; 5] = [0x1FC, 0x1FD, 0x20A, 0x2EB, 0x1329];

pub const CARGO_BLOCKED_RANGE: std::ops::RangeInclusive<u16> = 0x20E..=0x219;

pub fn deposits(player: &Entered, items: &ItemList, at: Coord) -> bool {
    let Some(item) = item_move::item_at(player, at).filter(|item| !item.is_empty()) else {
        return false;
    };

    if Effects::new(items, &[]).total_scaled(&item, CARGO_GATE_FIELD) != 0 {
        return false;
    }

    !CARGO_BLOCKED_IDS.contains(&item.index) && !CARGO_BLOCKED_RANGE.contains(&item.index)
}

#[allow(clippy::too_many_arguments)]
pub fn right_click(
    player: &mut Entered,
    items: &ItemList,
    live: &Live,
    cooldowns: &mut Cooldowns,
    keys: Mods,
    now_ms: u32,
    at: Coord,
    open_tab: i32,
) -> Vec<Action> {
    let mut out = Vec::new();

    let Some(item) = item_at(player, at) else {
        return out;
    };

    let id = i32::from(item.index as i16);

    if id == 0 {
        return out;
    }

    if at.container != container::INVENTORY {
        if at.container == container::EQUIPMENT && !keys.ctrl && refinable(items, &item) {
            out.push(Action::Refine { at });
        }

        out.extend(ctrl_move(player, items, keys, at, open_tab));

        return out;
    }

    if keys.shift && ANGELS.contains(&live.pet) && bag_unlocked(player, at.slot) {
        out.push(Action::Prompt {
            action: action::SELL,
            strdef: strdef::SELL_CONFIRM,
            slot: at.slot,
        });

        return out;
    }

    let tab = at.slot / item_move::BAG_CELLS;

    for (bag, &locked) in item_move::PREMIUM_BAGS.iter().enumerate() {
        if tab == locked && player.bag_extension(bag).is_none() {
            if keys.ctrl {
                tracing::debug!(?at, tab, "CTRL equip blocked by locked inventory tab");
            }

            return out;
        }
    }

    let effects = Effects::new(items, &[]);

    let kind = effects.total_scaled(&item, EFFECT_KIND);

    if cooldowns.running(kind, now_ms) {
        if keys.ctrl {
            tracing::debug!(?at, id, kind, now_ms, "CTRL equip blocked by item cooldown");
        }

        return out;
    }

    if live.mode_letter == MODE_EXCHANGE {
        if keys.ctrl {
            tracing::debug!(?at, id, "CTRL equip redirected by selected exchange skill");
        }

        if now_ms.wrapping_sub(live.last_attack_ms) > 1000 {
            out.push(Action::Prompt {
                action: action::EXCHANGE,
                strdef: strdef::EXCHANGE_GOLD,
                slot: at.slot,
            });
        }

        return out;
    }

    if id == ID_INERT {
        return out;
    }

    if id == ID_OPTION_STONE {
        out.push(Action::Open {
            content: content::OPTION_STONE,
            slot: at.slot,
        });
    } else {
        if POWDERS.contains(&id) {
            out.push(Action::Refine { at });
        }

        if BLESSING.contains(&id) {
            if !live.affect_20 && !cooldowns.running(kind, now_ms) {
                out.push(Action::Send(use_request(live.position, at.slot)));

                if spend(player, items, at) {
                    cooldowns.stamp(kind, now_ms);
                }

                out.push(Action::ItemChanged { at });
            }

            return out;
        }

        if id == ID_TRAVELING_BAG {
            if !live.affect_20 && !cooldowns.running(kind, now_ms) {
                out.push(Action::Send(use_request(live.position, at.slot)));

                cooldowns.stamp(kind, now_ms);

                clear(player, at);

                out.push(Action::ItemChanged { at });
            }

            return out;
        }
    }

    if kind == KIND_TICKET {
        out.push(Action::Prompt {
            action: action::TICKET,
            strdef: strdef::REGISTER_TICKET,
            slot: at.slot,
        });

        return out;
    }

    if kind == KIND_RECALL || kind == KIND_WARP {
        if live.move_alpha > 0.0 && live.move_alpha < 0.9 {
            return out;
        }

        out.push(Action::Stage(use_request(live.position, at.slot)));

        out.push(Action::Send(Request::Drink));

        spend(player, items, at);

        out.push(Action::ItemChanged { at });
    }

    if id == ID_RETURN_OF_ABILITY {
        out.push(Action::Toast {
            strdef: strdef::ABILITY_MASTER,
        });

        return out;
    }

    if id == ID_FIRECRACKER {
        if !cooldowns.running(kind, now_ms) {
            out.push(Action::Firecracker { slot: at.slot });

            cooldowns.stamp(kind, now_ms);
        }

        return out;
    }

    if kind == KIND_HUNTING_SCROLL {
        let page = match id {
            _ if HUNTING_SCROLLS.contains(&id) => id - HUNTING_SCROLLS.start(),
            ID_QUEST_SCROLL => QUEST_SCROLL_PAGE,
            _ => return out,
        };

        out.push(Action::HuntingScroll {
            page,
            request: use_request(live.position, at.slot),
        });

        return out;
    }

    if kind == KIND_EXP_QUEST {
        out.push(Action::Prompt {
            action: action::CONFIRM,
            strdef: strdef::EXP_QUEST,
            slot: at.slot,
        });

        return out;
    }

    if !usable(items, kind, id) {
        if !keys.ctrl && refinable(items, &item) {
            out.push(Action::Refine { at });

            return out;
        }

        out.extend(ctrl_move(player, items, keys, at, open_tab));

        return out;
    }

    if live.affect_20 && kind == 1 {
        return out;
    }

    if KIND_GUARDED.contains(&kind) {
        if mount_attached(player) {
            out.push(Action::Toast {
                strdef: strdef::DETACH_FIRST,
            });

            return out;
        }
    } else if kind == KIND_SEAL {
        if item.effects.first().map(|e| e.id) == Some(b';') {
            out.push(Action::Seal { slot: at.slot });

            return out;
        }

        out.push(Action::Prompt {
            action: action::CONFIRM,
            strdef: strdef::SEAL_CHARACTER,
            slot: at.slot,
        });

        return out;
    } else if kind == KIND_SECOND_CLASS {
        out.push(Action::Prompt {
            action: action::CONFIRM,
            strdef: strdef::SECOND_CLASS,
            slot: at.slot,
        });

        return out;
    }

    if id != ID_NO_DISPATCH {
        if id == ID_RPS_COUPON {
            out.push(Action::Open {
                content: content::RPS_GAME,
                slot: at.slot,
            });
        }

        out.extend(
            dispatch(
                player,
                items,
                cooldowns,
                kind,
                id,
                at,
                now_ms,
                live.position,
            )
            .actions(kind, id, at),
        );
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NotTheBackpack,
    EmptyCell,
    NotThatItem,
    Cooling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatched {
    pub sent: Option<Action>,
    pub refused: Option<Refused>,
    pub emptied: bool,
    pub portion: i32,
    pub sound: Option<i64>,
}

impl Dispatched {
    fn refused(refused: Refused) -> Dispatched {
        Dispatched {
            sent: None,
            refused: Some(refused),
            emptied: false,
            portion: 0,
            sound: None,
        }
    }

    pub fn actions(&self, kind: i32, id: i32, at: Coord) -> Vec<Action> {
        match (self.sent.clone(), self.refused) {
            (Some(sent), _) => {
                let mut out = vec![sent, Action::TrackUse { id }];

                out.extend(self.sound.map(|slot| Action::Sound { slot }));

                out.push(Action::ItemChanged { at });

                out
            }
            (None, Some(refused)) => {
                vec![Action::Dispatch {
                    kind,
                    id,
                    slot: at.slot,
                    refused,
                }]
            }
            (None, None) => Vec::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn dispatch(
    player: &mut Entered,
    items: &ItemList,
    cooldowns: &mut Cooldowns,
    kind: i32,
    id: i32,
    at: Coord,
    now_ms: u32,
    position: (u16, u16),
) -> Dispatched {
    if at.container != container::INVENTORY {
        return Dispatched::refused(Refused::NotTheBackpack);
    }

    let Some(item) = item_at(player, at).filter(|item| item.index != 0) else {
        return Dispatched::refused(Refused::EmptyCell);
    };

    if i32::from(item.index as i16) != id {
        return Dispatched::refused(Refused::NotThatItem);
    }

    if cooldowns.running(kind, now_ms) {
        return Dispatched::refused(Refused::Cooling);
    }

    let sent = match kind == MOUNT_FOOD_KIND {
        true => Action::Feed(super::consume::FeedSend {
            slot: at.slot,
            at: position,
        }),
        false => Action::Send(use_request(position, at.slot)),
    };

    cooldowns.stamp(kind, now_ms);

    let portion = portion(id);

    let emptied = spend_n(player, items, at, portion);

    Dispatched {
        sent: Some(sent),
        refused: None,
        emptied,
        portion,
        sound: use_sound(kind),
    }
}

pub const MOUNT_FOOD_KIND: i32 = 0x0F;

pub(crate) fn ticket_accepts(scene: &mut super::field::FieldScene) {
    let Some(at) = scene.panels.as_mut().and_then(|p| p.ticket_accepted.take()) else {
        return;
    };

    if at.container != 1 {
        return;
    }

    let Some(body) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    let tile = (body.position.x as i32 as u16, body.position.z as i32 as u16);

    let Some(item) = item_at(&scene.entered, at).filter(|i| !i.is_empty()) else {
        return;
    };

    let Some(p) = scene.panels.as_mut() else {
        return;
    };

    let kind = formats::data::item_effect::Effects::new(&p.items, &[]).total_scaled(&item, 0x26);

    if kind != 14 || p.cooldowns.running(kind, scene.time.now_ms) {
        return;
    }

    scene.outbox.push(
        Request::Use {
            slot: at.slot,
            at: tile,
        }
        .to_bytes(scene.entered.client_id, 0),
    );

    p.cooldowns.stamp(kind, scene.time.now_ms);

    spend(&mut scene.entered, &p.items, at);

    scene.world.play_sound(47);

    super::screens::refresh_all(p, &scene.entered);

    super::panels::refresh_inventory_hover(p, &scene.entered, at.slot);

    super::panels::refine_item_changed(p, &scene.entered);
}

pub const ID_SPENT_TEN: i32 = 0xFD1;

pub const PORTION_TEN: i32 = 10;

pub fn portion(id: i32) -> i32 {
    match id == ID_SPENT_TEN {
        true => PORTION_TEN,
        false => 1,
    }
}

pub fn use_sound(kind: i32) -> Option<i64> {
    if kind == SILENT_KIND {
        return None;
    }

    match TELEPORT_SOUND_KINDS.contains(&kind) {
        true => Some(SOUND_TELEPORT),
        false => Some(SOUND_USE),
    }
}

pub const SILENT_KIND: i32 = 0x13;

pub const SOUND_TELEPORT: i64 = 0x36;

pub const SOUND_USE: i64 = 0x29;

pub const TELEPORT_SOUND_KINDS: std::ops::RangeInclusive<i32> = 0x0B..=0x0D;

pub const RAISED_PROMPTS: [u8; 2] = [action::CONFIRM, action::TICKET];

pub fn prompt_box(action: u8, strdef: usize) -> super::msgbox::Request {
    super::msgbox::Request {
        caption: Some(strdef::TITLE),
        kind: super::interact::TALK_BOX_KIND,
        context: u32::from(action),
        body: super::msgbox::Body::StrDef(strdef),
        flags: super::msgbox::FLAGS,
    }
}

pub fn describe(action: &Action) -> Option<String> {
    match action {
        Action::Send(_)
        | Action::TrackUse { .. }
        | Action::Feed(_)
        | Action::Sound { .. }
        | Action::Stage(_)
        | Action::ItemChanged { .. } => None,
        Action::Open { content, slot } => Some(format!(
            "NWYD: slot {slot} wants {} (content {content:#04X})",
            super::panels::screen_name(*content),
        )),
        Action::Refine { .. } => None,
        Action::Prompt {
            action,
            strdef,
            slot,
        } => Some(format!(
            "NWYD: slot {slot} raises message box ({:#04X}, {action:#04X}) over strdef[{strdef}]",
            super::interact::TALK_BOX_KIND,
        )),
        Action::Toast { strdef } => Some(format!("NWYD: toast strdef[{strdef}]")),
        Action::HuntingScroll { page, .. } => {
            Some(format!("NWYD: hunting scroll opened page {page}"))
        }
        Action::Seal { slot } => Some(format!("NWYD: seal name entry opened from slot {slot}")),
        Action::Firecracker { slot } => Some(format!(
            "NWYD: Premium Firecracker pattern editor opened from slot {slot}"
        )),
        Action::Dispatch {
            kind,
            id,
            slot,
            refused,
        } => Some(format!(
            "NWYD: item use kind {kind:#04X}, id {id:#06X}, slot {slot} refused — {refused:?}"
        )),
        Action::Unbuilt { what } => Some(format!("NWYD: not built — {what}")),
    }
}

pub fn mount_attached(player: &Entered) -> bool {
    player.equipment.mount().index != 0
}

pub const MODE_EXCHANGE: u8 = b'S';

pub const ID_INERT: i32 = 0xC87;

pub const ID_OPTION_STONE: i32 = 0x1911;

pub const ID_TRAVELING_BAG: i32 = 0xD8B;

pub const ID_RETURN_OF_ABILITY: i32 = 0xD08;

pub const ID_FIRECRACKER: i32 = 0xD72;

pub const ID_RPS_COUPON: i32 = 0x132B;

pub const ID_NO_DISPATCH: i32 = 0x132A;

pub const ID_QUEST_SCROLL: i32 = 0xDB4;

pub const QUEST_SCROLL_PAGE: i32 = 6;

pub const POWDERS: [i32; 3] = [0x102D, 0x19D, 0x19C];

pub const BLESSING: std::ops::RangeInclusive<i32> = 0xD8C..=0xD8F;

pub const HUNTING_SCROLLS: std::ops::RangeInclusive<i32> = 0xD68..=0xD6D;

pub const KIND_TICKET: i32 = 0x0E;

pub const KIND_RECALL: i32 = 0x0B;

pub const KIND_WARP: i32 = 0x0D;

pub const KIND_HUNTING_SCROLL: i32 = 0xC3;

pub const KIND_EXP_QUEST: i32 = 0xBB;

pub const KIND_SEAL: i32 = 0xCE;

pub const KIND_SECOND_CLASS: i32 = 0xD3;

pub const KIND_GUARDED: std::ops::RangeInclusive<i32> = 0x46..=0x59;

fn use_request(position: (u16, u16), slot: i32) -> Request {
    Request::Use { slot, at: position }
}

pub(crate) fn spend(player: &mut Entered, items: &ItemList, at: Coord) -> bool {
    spend_n(player, items, at, 1)
}

fn spend_n(player: &mut Entered, items: &ItemList, at: Coord, portion: i32) -> bool {
    let count = ui::icon::stack_count(&item_at(player, at).unwrap_or_default(), items);

    match count > portion {
        false => {
            clear(player, at);

            true
        }
        true => {
            if let Some(slot) = slot_mut(player, at) {
                slot.appearance = (count - portion) as u16;
            }

            false
        }
    }
}

fn clear(player: &mut Entered, at: Coord) {
    if let Some(slot) = slot_mut(player, at) {
        *slot = Item::default();
    }
}

fn slot_mut(player: &mut Entered, at: Coord) -> Option<&mut Item> {
    let slot = usize::try_from(at.slot).ok()?;

    match at.container {
        container::EQUIPMENT => player.equipment.items.get_mut(slot),
        container::INVENTORY => player.carry.get_mut(slot),
        _ => None,
    }
}

pub const ANGELS: [i16; 2] = [0xF4A, 0xF4B];

pub fn bag_unlocked(player: &Entered, slot: i32) -> bool {
    let tab = slot / item_move::BAG_CELLS;

    if !(0..item_move::BAG_TABS).contains(&tab) {
        return false;
    }

    match usize::try_from(tab - item_move::PREMIUM_BAGS[0]) {
        Ok(bag) => player.bag_extension(bag).is_some(),
        Err(_) => true,
    }
}

pub fn free_carry_slot(player: &Entered, tab: i32, gated: bool) -> i32 {
    let open = [
        true,
        true,
        player.bag_extension(0).is_some(),
        player.bag_extension(1).is_some(),
    ];

    let tab = if (0..item_move::BAG_TABS).contains(&tab) {
        tab
    } else {
        0
    };

    let lo = tab * item_move::BAG_CELLS;

    let hi = (lo + item_move::BAG_CELLS).min(MAX_SLOT);

    let free = |slot: i32| {
        (!gated || open[(slot / item_move::BAG_CELLS) as usize])
            && player
                .carry
                .get(slot as usize)
                .is_some_and(|item| item.index == 0)
    };

    (lo..hi)
        .find(|&slot| free(slot))
        .or_else(|| (0..MAX_SLOT).find(|&slot| !(lo..hi).contains(&slot) && free(slot)))
        .unwrap_or(-1)
}

pub fn ctrl_move(
    player: &Entered,
    items: &ItemList,
    keys: Mods,
    at: Coord,
    open_tab: i32,
) -> Option<Action> {
    if !keys.ctrl {
        return None;
    }

    let item = item_at(player, at)?;

    if !equip::ctrl_movable(items, &item) {
        tracing::debug!(
            ?at,
            item = item.index,
            "CTRL item move rejected by equipment mask"
        );

        return None;
    }

    if at.container == container::EQUIPMENT {
        let free = free_carry_slot(player, open_tab, true);

        if !(0..MAX_SLOT).contains(&free) {
            tracing::debug!(?at, open_tab, "CTRL unequip has no free inventory cell");

            return None;
        }

        return Some(Action::Send(Request::Move {
            src: at,
            dst: Coord {
                container: container::INVENTORY,
                slot: free,
            },
        }));
    }

    let target = equip::ctrl_target(player, items, &item)?;

    Some(Action::Send(Request::Move {
        src: at,
        dst: Coord {
            container: container::EQUIPMENT,
            slot: target,
        },
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Nothing,
    Split { slot: i32 },
    Send(Request),
    PickUp,
}

#[derive(Debug, Clone, Copy)]
pub struct Cargo<'a> {
    pub slots: &'a [Item],
    pub tab: i32,
}

pub fn free_cargo_slot(slots: &[Item], tab: i32) -> i32 {
    let tab = if (0..CARGO_TABS).contains(&tab) {
        tab
    } else {
        0
    };

    let lo = tab * CARGO_TAB_CELLS;

    let hi = (lo + CARGO_TAB_CELLS).min(CARGO_SLOTS);

    let free = |slot: i32| {
        slots
            .get(slot as usize)
            .is_some_and(|item: &Item| item.index == 0)
    };

    (lo..hi)
        .find(|&slot| free(slot))
        .or_else(|| (0..CARGO_SLOTS).find(|&slot| !(lo..hi).contains(&slot) && free(slot)))
        .unwrap_or(-1)
}

pub const CARGO_SLOTS: i32 = 0x78;

pub const CARGO_TAB_CELLS: i32 = 0x28;

pub const CARGO_TABS: i32 = 3;

pub fn max_stack(items: &ItemList, id: i32) -> i32 {
    if !(1..=MAX_TEMPLATE_ID).contains(&id) {
        return 0;
    }

    let Some(entry) = usize::try_from(id).ok().and_then(|id| items.get(id)) else {
        return 0;
    };

    if FLAT_STACK_TYPES.contains(&entry.item_type) {
        return FLAT_STACK;
    }

    i32::from(entry.max_stack)
}

pub const MAX_TEMPLATE_ID: i32 = 0x38A3;

pub const FLAT_STACK_TYPES: [i16; 2] = [0x36, 0x3D];

pub const FLAT_STACK: i32 = 100;

pub fn left_click(
    player: &Entered,
    items: &ItemList,
    keys: Mods,
    cargo: Option<Cargo<'_>>,
    at: Coord,
) -> Click {
    if keys.shift && at.container == container::INVENTORY {
        let Some(item) = item_at(player, at) else {
            return Click::Nothing;
        };

        let id = i32::from(item.index as i16);

        if id != 0
            && item_effect::stack_size(&item) > 1
            && max_stack(items, id) != 0
            && SPLIT_DIALOG_BUILT
        {
            return Click::Split { slot: at.slot };
        }

        return Click::Nothing;
    }

    if !keys.ctrl {
        return Click::PickUp;
    }

    let Some(cargo) = cargo else {
        return Click::PickUp;
    };

    let free = free_cargo_slot(cargo.slots, cargo.tab);

    if free < 0 {
        return Click::Nothing;
    }

    let source = i32::from(at.container == container::INVENTORY);

    Click::Send(Request::Move {
        src: Coord {
            container: source,
            slot: at.slot,
        },
        dst: Coord {
            container: container::CARGO,
            slot: free,
        },
    })
}

const SPLIT_DIALOG_BUILT: bool = true;

pub fn usable(items: &ItemList, kind: i32, id: i32) -> bool {
    let item_type = usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .map_or(0, |entry| entry.item_type);

    item_type == USABLE_ITEM_TYPE
        || USABLE_KINDS.iter().any(|r| r.contains(&kind))
        || USABLE_IDS.iter().any(|r| r.contains(&id))
        || id == ID_ALWAYS_USABLE
}

pub const USABLE_ITEM_TYPE: i16 = 0x3D;

pub const ID_ALWAYS_USABLE: i32 = 0xD99;

pub const USABLE_KINDS: [std::ops::RangeInclusive<i32>; 30] = [
    1..=1,
    6..=8,
    0xA..=0xA,
    0xC..=0xC,
    0xF..=0xF,
    0x12..=0x12,
    0x13..=0x1C,
    0x1E..=0x1E,
    0x1F..=0x24,
    0x28..=0x3A,
    0x3C..=0x45,
    0x46..=0x59,
    0x83..=0x8A,
    0x8C..=0x8C,
    0xA1..=0xA8,
    0xAA..=0xAF,
    0xB0..=0xB2,
    0xB8..=0xB9,
    0xBC..=0xBD,
    0xBF..=0xC2,
    0xC5..=0xC6,
    0xC8..=0xCA,
    0xCB..=0xCD,
    0xCE..=0xCE,
    0xD0..=0xD0,
    0xD2..=0xD2,
    0xF2..=0xF2,
    0xD3..=0xD3,
    0xBB..=0xBB,
    0xC3..=0xC3,
];

pub const USABLE_IDS: [std::ops::RangeInclusive<i32>; 34] = [
    0xF1..=0xF1,
    0x19F..=0x19F,
    0x1D9..=0x1D9,
    0x1E9..=0x1E9,
    0x286..=0x287,
    0x2A7..=0x2A7,
    0x6ED..=0x6ED,
    0x6F1..=0x6F4,
    0xBCC..=0xBCC,
    0xBCD..=0xBD2,
    0xC80..=0xCE3,
    0xC8A..=0xC8A,
    0xD32..=0xD32,
    0xD75..=0xD78,
    0xD7B..=0xD7E,
    0xD81..=0xD83,
    0xD91..=0xD91,
    0xD93..=0xD93,
    0xD96..=0xD98,
    0xFAE..=0xFAE,
    0xFBE..=0xFBF,
    0xFCC..=0xFCF,
    0xFD0..=0xFD2,
    0xFFB..=0xFFB,
    0x1032..=0x1035,
    0x1324..=0x1335,
    0x1411..=0x1411,
    0x14DA..=0x14DA,
    0x154D..=0x154E,
    0x1711..=0x1711,
    0x1714..=0x1714,
    0x1722..=0x1723,
    0x1902..=0x1902,
    0x190B..=0x190B,
];

pub fn refinable(items: &ItemList, item: &Item) -> bool {
    if formats::data::game_rules::refinement_denied(
        item.index,
        formats::mesh::appearance::refine_level(item, items) as i32,
    ) {
        return false;
    }

    let id = i32::from(item.index as i16);

    if NEVER_REFINABLE.iter().any(|r| r.contains(&id)) {
        return false;
    }

    let effects = Effects::new(items, &[]);

    let mask = effects.total_scaled(item, formats::data::item_effect::effect::EQUIP_MASK);

    let has_room = |slot: usize| {
        item.effects.get(slot).is_none_or(|e| {
            e.id == 0 || e.id == b'+' || (REFINE_EFFECTS.start..=REFINE_EFFECTS.end).contains(&e.id)
        })
    };

    if !has_room(0) && !has_room(1) && !has_room(2) && mask != MOUNT_MASK {
        return false;
    }

    let refine = formats::mesh::appearance::refine_level(item, items) as i32;

    if refine < 0
        || refine_ceiling(items, item) <= refine
        || effects.total_scaled(item, EFFECT_KIND) != 0
    {
        return false;
    }

    if TYPE_EXEMPT.iter().any(|r| r.contains(&id)) {
        return true;
    }

    let item_type = usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .map_or(0, |entry| entry.item_type);

    if matches!(item_type, 0x3F | 0x15 | 0x16 | 0x17) {
        return false;
    }

    item_type == 0x14
        || id == ID_REFINE_EXEMPT
        || (2..=0x200).contains(&mask)
        || matches!(mask, 0x10000 | 0x20000 | 0xF00 | 0x800 | 0x2000)
}

pub const MOUNT_MASK: i32 = 0x4000;

pub const REFINE_EFFECTS: std::ops::Range<u8> = 0x73..0x7E;

pub const ID_REFINE_EXEMPT: i32 = 0x1718;

pub const NEVER_REFINABLE: [std::ops::RangeInclusive<i32>; 11] = [
    0x6C4..=0x6C4,
    0x6D0..=0x6D7,
    0x6E0..=0x6E3,
    0x6E7..=0x6E7,
    0x6EA..=0x6EA,
    0x3E5..=0x3E8,
    0xBB8..=0xBE9,
    0xC7D..=0xC7F,
    0xDAF..=0xDB2,
    0xDB7..=0xDB8,
    0x16B3..=0x16B7,
];

pub const TYPE_EXEMPT: [std::ops::RangeInclusive<i32>; 6] = [
    0x76D..=0x776,
    0x4D2..=0x4D5,
    0x559..=0x55C,
    0x5EF..=0x5F2,
    0x685..=0x688,
    0x6B2..=0x6B2,
];

pub fn refine_ceiling(items: &ItemList, item: &Item) -> i32 {
    let id = i32::from(item.index as i16);

    if TYPE_EXEMPT.iter().any(|r| r.contains(&id)) {
        return 9;
    }

    let effects = Effects::new(items, &[]);

    let entry = usize::try_from(id).ok().and_then(|entry| items.get(entry));

    if entry.as_ref().is_some_and(|e| e.item_type == 0x14) {
        return effects.slot_scaled(item, 0x4E);
    }

    let mut restriction =
        effects.total_scaled(item, formats::data::item_effect::effect::RESTRICTION);

    if matches!(id, 0xDB3 | 0xDAD | 0xDAE | 0xDAF) {
        restriction = 0;
    }

    let capped = (0x204..=0x209).contains(&id)
        || (0x24F..=0x253).contains(&id)
        || (0x1644..=0x164A).contains(&id);

    if (0x1676..=0x167C).contains(&id) || capped {
        return 0xF;
    }

    let eleven = (0x1659..=0x165E).contains(&id) || (0x168B..=0x1690).contains(&id);

    let ten = (0x237..=0x23A).contains(&id);

    let also_capped = (0x164B..=0x1651).contains(&id);

    if (0x167D..=0x1687).contains(&id) || also_capped {
        return 0xF;
    }

    if eleven || (0x1676..=0x167C).contains(&id) || ten {
        return 0xB;
    }

    if (0x24F..=0x253).contains(&id) {
        return 10;
    }

    let refine = formats::mesh::appearance::refine_level(item, items) as i32;

    if restriction < 3 && !slow_refine(id) && refine <= 10 {
        return 9;
    }

    0xF
}

pub fn slow_refine(id: i32) -> bool {
    const WINDOWS: [std::ops::RangeInclusive<i32>; 15] = [
        0x6B5..=0x6B5,
        0xE1D..=0xE20,
        0xE31..=0xE34,
        0xE45..=0xE48,
        0xE59..=0xE5C,
        0xE6D..=0xE70,
        0xE81..=0xE84,
        0xEA1..=0xEA8,
        0xEBD..=0xEC0,
        0xED1..=0xED4,
        0xEDE..=0xEE2,
        0xEF2..=0xEF6,
        0xF06..=0xF0A,
        0xF1A..=0xF1E,
        0x16AE..=0x16B2,
    ];

    WINDOWS.iter().any(|r| r.contains(&id))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn dispatched_items_track_use_after_the_send_and_before_inventory_refresh() {
        let at = Coord {
            container: container::INVENTORY,
            slot: 3,
        };

        let sent = Action::Send(Request::Use {
            slot: at.slot,
            at: (10, 20),
        });

        let dispatched = Dispatched {
            sent: Some(sent.clone()),
            refused: None,
            emptied: true,
            portion: 1,
            sound: Some(SOUND_USE),
        };

        assert_eq!(
            dispatched.actions(0, 700, at),
            vec![
                sent,
                Action::TrackUse { id: 700 },
                Action::Sound { slot: SOUND_USE },
                Action::ItemChanged { at },
            ]
        );
    }

    #[test]
    fn refused_and_empty_dispatches_do_not_track_item_use() {
        let at = Coord {
            container: container::INVENTORY,
            slot: 3,
        };

        for refused in [
            Refused::NotTheBackpack,
            Refused::EmptyCell,
            Refused::NotThatItem,
            Refused::Cooling,
        ] {
            assert_eq!(
                Dispatched::refused(refused).actions(0, 700, at),
                vec![Action::Dispatch {
                    kind: 0,
                    id: 700,
                    slot: at.slot,
                    refused,
                }]
            );
        }

        assert!(
            Dispatched {
                sent: None,
                refused: None,
                emptied: false,
                portion: 0,
                sound: None,
            }
            .actions(0, 700, at)
            .is_empty()
        );
    }
}
