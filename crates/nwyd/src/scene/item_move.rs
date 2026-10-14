use crate::scene::enter::Entered;
use crate::scene::equip;
use formats::data::item_effect::Effects;
use formats::data::item_list::ItemList;
use formats::mesh::appearance::refine_level;
use formats::net::account::Item;

pub mod container {

    pub const EQUIPMENT: i32 = 0;

    pub const INVENTORY: i32 = 1;

    pub const CARGO: i32 = 2;
}

pub const EQUIPMENT_SLOTS: usize = 0x12;

pub const INVENTORY_SLOTS: usize = 0x3C;

pub const CARGO_SLOTS: usize = formats::net::account::CARGO_SLOTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    pub container: i32,
    pub slot: i32,
}

impl Coord {
    pub fn of_cell(group: i32, slot: i32) -> Coord {
        Coord {
            container: group - 1,
            slot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Move { src: Coord, dst: Coord },
    Combine { src: Coord, dst: Coord },
    Use { slot: i32, at: (u16, u16) },
    Warp { slot: i32, at: (u16, u16), row: u16 },
    Drink,
    Sell { shop: u16, at: Coord },
    Buy { shop: u16, stock: u16, slot: i32 },
    Delete { slot: i32, item_id: u16 },
}

pub const HEADER_LEN: usize = 0x0C;

pub const MOVE_LEN: usize = 0x14;

pub const COMBINE_LEN: usize = 0x24;

pub const DRINK_LEN: usize = 0x10;

pub const SELL_LEN: usize = 0x14;

pub const BUY_LEN: usize = 0x18;

pub const DELETE_LEN: usize = 0x14;

pub const OPCODE_MOVE: u16 = 0x376;

pub const OPCODE_COMBINE: u16 = 0x373;

pub const OPCODE_DRINK: u16 = 0x3AE;

pub const OPCODE_SELL: u16 = 0x37A;

pub const OPCODE_BUY: u16 = 0x379;

pub const OPCODE_DELETE: u16 = 0x2E4;

pub fn stock_index(cell: i32) -> u16 {
    u16::try_from((cell / 9) * 0x1B + cell % 9).unwrap_or(0)
}

pub const MINIMISED_SLOT_FLAG: i32 = 0x2000;

pub const ANTI_CHEAT_TOKEN: u16 = 0;

impl Request {
    pub fn opcode(&self) -> u16 {
        match self {
            Request::Move { .. } => OPCODE_MOVE,
            Request::Combine { .. } | Request::Use { .. } | Request::Warp { .. } => OPCODE_COMBINE,
            Request::Drink => OPCODE_DRINK,
            Request::Sell { .. } => OPCODE_SELL,
            Request::Buy { .. } => OPCODE_BUY,
            Request::Delete { .. } => OPCODE_DELETE,
        }
    }

    pub fn to_bytes(&self, client_id: u16, token: u16) -> Vec<u8> {
        let len = match self {
            Request::Move { .. } => MOVE_LEN,
            Request::Combine { .. } | Request::Use { .. } => COMBINE_LEN,
            Request::Warp { .. } => COMBINE_LEN,
            Request::Drink => DRINK_LEN,
            Request::Sell { .. } => SELL_LEN,
            Request::Buy { .. } => BUY_LEN,
            Request::Delete { .. } => DELETE_LEN,
        };

        let mut out = vec![0u8; len];

        out[0x04..0x06].copy_from_slice(&self.opcode().to_le_bytes());

        out[0x06..0x08].copy_from_slice(&client_id.to_le_bytes());

        match *self {
            Request::Move { src, dst } => {
                out[0x0C] = dst.container as u8;

                out[0x0D] = dst.slot as u8;

                out[0x0E] = src.container as u8;

                out[0x0F] = src.slot as u8;

                out[0x10..0x12].copy_from_slice(&token.to_le_bytes());
            }
            Request::Combine { src, dst } => {
                write_use_frame(&mut out, src.slot, dst.container, dst.slot, 0);
            }
            Request::Use { slot, at } => {
                let packed = i32::from(at.0) | i32::from(at.1) << 16;

                write_use_frame(&mut out, slot, 0, 0, packed);
            }
            Request::Delete { slot, item_id } => {
                out[0x0C..0x10].copy_from_slice(&slot.to_le_bytes());

                out[0x10..0x14].copy_from_slice(&i32::from(item_id).to_le_bytes());
            }
            Request::Sell { shop, at } => {
                out[0x0C..0x0E].copy_from_slice(&shop.to_le_bytes());

                let container = u16::try_from(at.container).unwrap_or(0);

                out[0x0E..0x10].copy_from_slice(&container.to_le_bytes());

                let slot = u16::try_from(at.slot).unwrap_or(0);

                out[0x10..0x12].copy_from_slice(&slot.to_le_bytes());
            }
            Request::Buy { shop, stock, slot } => {
                out[0x0C..0x0E].copy_from_slice(&shop.to_le_bytes());

                out[0x0E..0x10].copy_from_slice(&stock.to_le_bytes());

                let slot = u32::try_from(slot).unwrap_or(0) & 0xFFFF;

                out[0x10..0x14].copy_from_slice(&slot.to_le_bytes());
            }
            Request::Warp { slot, at, row } => {
                let packed = i32::from(at.0) | i32::from(at.1) << 16;

                write_use_frame(&mut out, slot, 0, 0, packed);

                out[0x20..0x24].copy_from_slice(&i32::from(row).to_le_bytes());
            }
            Request::Drink => {
                out[0x0C..0x10].copy_from_slice(&1i32.to_le_bytes());
            }
        }

        out
    }
}

fn write_use_frame(out: &mut [u8], src_slot: i32, dst_container: i32, dst_slot: i32, at: i32) {
    let words = [
        container::INVENTORY,
        src_slot,
        dst_container,
        dst_slot,
        at,
        0,
    ];

    for (index, word) in words.iter().enumerate() {
        let byte = HEADER_LEN + index * 4;

        out[byte..byte + 4].copy_from_slice(&word.to_le_bytes());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    Sent(Request),
    ConfirmCombine(CombinePrompt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombinePrompt {
    pub message: usize,
    pub src: Coord,
    pub dst: Coord,
    pub captured: Item,
}

pub fn closes_combine_prompt(source: Item, target: Item) -> bool {
    (source.index == 0x19d
        && ((0x1659..=0x165e).contains(&target.index) || (0x168b..=0x1690).contains(&target.index)))
        || (0x24f..=0x253).contains(&target.index)
}

pub fn confirm_combine(player: &mut Entered, items: &ItemList, prompt: CombinePrompt) -> Outcome {
    if !(0..60).contains(&prompt.src.slot)
        || !(0..60).contains(&prompt.dst.slot)
        || player.carry[prompt.dst.slot as usize] != prompt.captured
    {
        return Outcome::Nothing;
    }

    let Some(source) = item_at(player, prompt.src) else {
        return Outcome::Nothing;
    };

    let count = ui::icon::stack_count(&source, items);

    let Some(source) = item_mut(player, prompt.src) else {
        return Outcome::Nothing;
    };

    if count < 2 {
        *source = Default::default();
    } else {
        source.appearance = (count - 1) as u16;
    }

    Outcome::Sent(Request::Combine {
        src: prompt.src,
        dst: prompt.dst,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cooldowns {
    pub last: [u32; COOLDOWN_BUCKETS],
}

pub const COOLDOWN_BUCKETS: usize = 5;

pub const COOLDOWN_MS: [u32; COOLDOWN_BUCKETS] = [250, 500, 300, 300, 0];

pub fn cooldown_bucket(kind: i32) -> usize {
    match kind {
        1 => 0,
        0xF => 1,
        0xF2 | 0xF3 => 3,
        0xB8 => 4,
        _ => 2,
    }
}

impl Cooldowns {
    pub fn running(&self, kind: i32, now_ms: u32) -> bool {
        let bucket = cooldown_bucket(kind);

        let last = self.last[bucket];

        last != 0 && now_ms.wrapping_sub(last) < COOLDOWN_MS[bucket]
    }

    pub fn stamp(&mut self, kind: i32, now_ms: u32) {
        self.last[cooldown_bucket(kind)] = now_ms;
    }
}

pub mod tint {

    pub const EQUIP: u32 = 0x3300_00FF;

    pub const COMBINE: u32 = 0x3300_FF00;

    pub const REFUSED: u32 = 0x33FF_0000;

    pub const NONE: u32 = 0;
}

pub fn tint(player: &Entered, items: &ItemList, src: Coord, dst: Coord) -> u32 {
    if item_at(player, src).is_none() || item_at(player, dst).is_none() {
        return tint::NONE;
    }

    if equip::may_equip(player, items, src, dst) {
        return tint::EQUIP;
    }

    tint::REFUSED
}

pub fn item_at(player: &Entered, at: Coord) -> Option<Item> {
    let slot = usize::try_from(at.slot).ok()?;

    match at.container {
        container::EQUIPMENT => (slot < EQUIPMENT_SLOTS)
            .then(|| player.equipment.items.get(slot))
            .flatten(),
        container::INVENTORY => (slot < INVENTORY_SLOTS)
            .then(|| player.carry.get(slot))
            .flatten(),
        container::CARGO => (slot < CARGO_SLOTS)
            .then(|| player.warehouse.cargo.get(slot))
            .flatten(),
        _ => None,
    }
    .copied()
}

fn item_mut(player: &mut Entered, at: Coord) -> Option<&mut Item> {
    let slot = usize::try_from(at.slot).ok()?;

    match at.container {
        container::EQUIPMENT => (slot < EQUIPMENT_SLOTS)
            .then(|| player.equipment.items.get_mut(slot))
            .flatten(),
        container::INVENTORY => (slot < INVENTORY_SLOTS)
            .then(|| player.carry.get_mut(slot))
            .flatten(),
        container::CARGO => (slot < CARGO_SLOTS)
            .then(|| player.warehouse.cargo.get_mut(slot))
            .flatten(),
        _ => None,
    }
}

pub fn apply_swap(player: &mut Entered, a: Coord, b: Coord) -> Swapped {
    let (Some(first), Some(second)) = (item_at(player, a), item_at(player, b)) else {
        return Swapped::Refused;
    };

    if let Some(slot) = item_mut(player, a) {
        *slot = second;
    }

    if let Some(slot) = item_mut(player, b) {
        *slot = first;
    }

    Swapped::Moved { a, b }
}

pub fn apply_inbound(
    panels: &mut crate::scene::panels::Panels,
    player: &mut Entered,
    dst: (u8, u8),
    src: (u8, u8),
) -> Option<Swapped> {
    let at = |(container, slot): (u8, u8)| Coord {
        container: i32::from(container),
        slot: i32::from(slot),
    };

    tracing::debug!(
        target : "nwyd::body_state", event = "item_move_376", entity = player.client_id,
        ? dst, ? src, dst_item_before = ? item_at(player, at(dst)), src_item_before = ?
        item_at(player, at(src)), live_body = player.equipment.body().index,
        retained_body = player.account_body,
        "Server item-move acknowledgement before swap"
    );

    let swapped = apply_swap(player, at(dst), at(src));

    tracing::debug!(
        target : "nwyd::body_state", event = "item_move_376_applied", entity = player
        .client_id, ? swapped, equipment = ? player.equipment.items.map(| item | item
        .index), "Server item-move result"
    );

    if swapped == Swapped::Refused {
        return Some(swapped);
    }

    player.refresh_class();

    let entered = *player;

    let (items, icons, strings) = (
        panels.items.clone(),
        panels.icons.clone(),
        panels.strings.clone(),
    );

    for screen in &mut panels.screens {
        match screen.content {
            crate::scene::panels::CONTENT_INVENTORY => {
                crate::scene::screens::inventory_refresh(screen, &entered, &items, &icons, &strings)
            }
            crate::scene::panels::CONTENT_CARGO => {
                crate::scene::screens::cargo_refresh(screen, &entered, &items, &icons)
            }
            _ => {}
        }
    }

    for _ in [dst, src] {
        crate::scene::panels::refine_item_changed(panels, player);
    }

    Some(swapped)
}

pub fn first_free_carry_slot(player: &Entered) -> Option<i32> {
    (0..INVENTORY_SLOTS)
        .find(|&slot| player.carry.get(slot).is_none_or(Item::is_empty))
        .and_then(|slot| i32::try_from(slot).ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swapped {
    Moved { a: Coord, b: Coord },
    Refused,
}

pub const EFFECT_KIND: u8 = 0x26;

pub const KIND_SCROLL: i32 = 0xBE;

fn kind(effects: &Effects<'_>, item: &Item) -> i32 {
    effects.total_scaled(item, EFFECT_KIND)
}

fn equip_mask(items: &ItemList, item: &Item) -> u32 {
    let id = i32::from(item.index as i16);

    usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .map_or(0, |entry| entry.equip_mask)
}

pub fn drop_item(
    player: &mut Entered,
    items: &ItemList,
    cooldowns: &mut Cooldowns,
    now_ms: u32,
    src: Coord,
    dst: Coord,
) -> Outcome {
    let (Some(source), Some(target)) = (item_at(player, src), item_at(player, dst)) else {
        return Outcome::Nothing;
    };

    let effects = Effects::new(items, &[]);

    let src_kind = kind(&effects, &source);

    let dst_kind = kind(&effects, &target);

    let src_id = i32::from(source.index as i16);

    let dst_id = i32::from(target.index as i16);

    if src.container == dst.container && src.slot == dst.slot {
        return Outcome::Nothing;
    }

    let wash = tint(player, items, src, dst);

    if is_combine(src_kind, dst_kind, src_id, dst_id, dst.container, wash) {
        return combine(
            player, items, cooldowns, now_ms, src, dst, src_kind, src_id, dst_id,
        );
    }

    if src_kind == KIND_SCROLL && dst.container == container::INVENTORY && wash != tint::EQUIP {
        return Outcome::Nothing;
    }

    if !equip::may_equip(player, items, src, dst) {
        return Outcome::Nothing;
    }

    if src.container == container::EQUIPMENT {
        if dst.container == container::EQUIPMENT {
            return Outcome::Nothing;
        }

        if dst_id != 0
            && (src.slot == 0
                || equip_mask(items, &target) & (1 << (src.slot & 0x1F)) == 0
                || equip_mask(items, &target) < 2
                || equip_mask(items, &source) < 2)
        {
            return Outcome::Nothing;
        }
    } else if src.container == dst.container && src.slot == dst.slot {
        return Outcome::Nothing;
    }

    if dst.container != container::EQUIPMENT
        || src_id == 0
        || (dst.slot != 0
            && equip_mask(items, &source) & (1 << (dst.slot & 0x1F)) != 0
            && equip_mask(items, &source) > 1)
    {
        return Outcome::Sent(Request::Move { src, dst });
    }

    Outcome::Nothing
}

fn is_combine(
    src_kind: i32,
    dst_kind: i32,
    src_id: i32,
    dst_id: i32,
    dst_container: i32,
    wash: u32,
) -> bool {
    if dst_id == 0 {
        return false;
    }

    let material = (4..7).contains(&src_kind)
        || src_kind == 9
        || matches!(src_kind, 0xF | 0x10)
        || (0xB4..0xB8).contains(&src_kind)
        || (0xEB..0xEF).contains(&src_kind)
        || (0xEF..0xF1).contains(&src_kind)
        || (0x5A..0x5F).contains(&src_kind)
        || matches!(src_kind, 0xB3 | 0xBA | 0xC4);

    let mut skip_middle = false;

    if (material && dst_container == container::EQUIPMENT)
        || (src_kind == KIND_SCROLL && dst_container == container::INVENTORY && wash == tint::EQUIP)
    {
        if dst_kind == 0 {
            return true;
        }
    } else if src_kind == KIND_SCROLL {
        skip_middle = true;
    }

    if !skip_middle {
        if src_kind == 0xF1 {
            if dst_container == container::INVENTORY && dst_kind == 0x10 {
                return true;
            }
        } else if matches!(src_kind, 4 | 5)
            && dst_container == container::INVENTORY
            && wash == tint::COMBINE
        {
            return true;
        }
    }

    let refinable = (0x76D..=0x776).contains(&dst_id)
        || (0x4D2..=0x4D5).contains(&dst_id)
        || (0x559..=0x55C).contains(&dst_id)
        || (0x5EF..=0x5F2).contains(&dst_id)
        || (0x685..=0x688).contains(&dst_id)
        || dst_id == 0x6B2;

    let mut combine = src_id == 0xD89
        || (matches!(src_kind, 4 | 5) && refinable)
        || (matches!(src_id, 0x190C | 0x190F) && dst_container == container::EQUIPMENT);

    if src_id == 0x1719
        && (dst_id == 0xD88
            || (0x167D..=0x1683).contains(&dst_id)
            || (0x164B..=0x1651).contains(&dst_id)
            || (0x1644..=0x1648).contains(&dst_id)
            || (0x1676..=0x167C).contains(&dst_id))
    {
        combine = true;
    }

    combine || (dst_id == 0x1718 && matches!(src_id, 0x19D | 0x102D))
}

pub const BAG_CELLS: i32 = 15;

pub const PREMIUM_BAGS: [i32; 2] = [2, 3];

pub const BAG_TABS: i32 = 4;

#[allow(clippy::too_many_arguments)]
fn combine(
    player: &mut Entered,
    items: &ItemList,
    cooldowns: &mut Cooldowns,
    now_ms: u32,
    src: Coord,
    dst: Coord,
    src_kind: i32,
    src_id: i32,
    _dst_id: i32,
) -> Outcome {
    if cooldowns.running(src_kind, now_ms) {
        return Outcome::Nothing;
    }

    if src.container != container::INVENTORY {
        return Outcome::Nothing;
    }

    if (0xEF..0xF1).contains(&src_kind) {
        return Outcome::Nothing;
    }

    let tab = src.slot / BAG_CELLS;

    for (bag, &locked_tab) in PREMIUM_BAGS.iter().enumerate() {
        if tab == locked_tab && player.bag_extension(bag).is_none() {
            return Outcome::Nothing;
        }
    }

    let target = item_at(player, dst).unwrap_or_default();

    let refine = refine_level(&target, items) as i32;

    if let Some(message) = formats::data::retail::refinement_prompt(src_id as u16, &target, refine)
    {
        cooldowns.stamp(src_kind, now_ms);

        let captured = player
            .carry
            .get(dst.slot as usize)
            .copied()
            .unwrap_or_default();

        return Outcome::ConfirmCombine(CombinePrompt {
            src,
            dst,
            captured,
            message,
        });
    }

    cooldowns.stamp(src_kind, now_ms);

    let count = ui::icon::stack_count(&item_at(player, src).unwrap_or_default(), items);

    if let Some(source) = item_mut(player, src) {
        match count < 2 {
            true => *source = Item::default(),
            false => source.appearance = (count - 1) as u16,
        }
    }

    Outcome::Sent(Request::Combine { src, dst })
}
