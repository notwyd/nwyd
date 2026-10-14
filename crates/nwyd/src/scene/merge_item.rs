use super::{
    enter::Entered,
    msgbox,
    panels::{CONTENT_INVENTORY, Panels},
    screens,
};
use formats::{
    data::{
        item_effect::{Effects, stack_size},
        item_list::ItemList,
        mix_list::MixList,
    },
    mesh::appearance::refine_level,
    net::account::Item,
};

pub const CONTENT: u32 = 0x10068;

pub const SCRIPT: &[u8] = b"ui/Panel/game/MergeItem.pane";

#[derive(Debug, Clone, Default)]
pub struct State {
    pub pieces: [Option<usize>; 2],
    pub jewel: Option<usize>,
    pub message: Option<usize>,
    snapshot: Option<Box<Snapshot>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    carry: [Item; 60],
    gold: u32,
    pieces: [Option<usize>; 2],
    jewel: Option<usize>,
    message: Option<usize>,
}

fn requirement(list: &MixList, role: usize) -> Option<(i32, u16)> {
    let need = list.recipe(11)?.need.get(role)?;

    match need.kind {
        0 => Some((1, u16::try_from(need.value).ok()?)),
        1 => {
            let row = list.requirement(usize::try_from(need.value).ok()?)?;

            Some((row.required_count.max(1), row.result.index))
        }
        _ => None,
    }
}

fn accepts(list: &MixList, items: &ItemList, role: usize, item: &Item) -> bool {
    let Some(need) = list.recipe(11).and_then(|r| r.need.get(role)) else {
        return false;
    };

    if item.index == 0 {
        return false;
    }

    if need.kind == 0 {
        return i32::from(item.index) == need.value;
    }

    if need.kind != 1 {
        return false;
    }

    let Some(row) = usize::try_from(need.value)
        .ok()
        .and_then(|i| list.requirement(i))
    else {
        return false;
    };

    if row.result.index != 0 {
        return item.index == row.result.index;
    }

    row.items
        .chunks_exact(2)
        .any(|r| (r[0] > 0 || r[1] > 0) && (r[0]..=r[1]).contains(&i32::from(item.index)))
        && row
            .result
            .effect
            .iter()
            .all(|e| e.id != b'+' || refine_level(item, items) >= u32::from(e.value))
}

fn piece_allowed(items: &ItemList, item: &Item) -> bool {
    let id = item.index;

    if !(1..6500).contains(&id) {
        return false;
    }

    if matches!(
        id, 747 | 1901..= 1910 | 1234..= 1237 | 1369..= 1372 | 1519..= 1522 | 1669..=
        1672 | 1714
    ) {
        return false;
    }

    if Effects::new(items, &[]).total_scaled(item, 0x70) == 3 && !(3500..=3502).contains(&id) {
        return false;
    }

    items
        .get(usize::from(id))
        .is_some_and(|e| e.equip_mask as u8 != 0)
        && refine_level(item, items) == 9
}

pub fn compatible(items: &ItemList, a: &Item, b: &Item) -> bool {
    if !piece_allowed(items, a) || !piece_allowed(items, b) {
        return false;
    }

    if a.index != b.index && items.name(usize::from(a.index)) != items.name(usize::from(b.index)) {
        return false;
    }

    let Some(entry) = items.get(usize::from(a.index)) else {
        return false;
    };

    !matches!(entry.equip_mask, 0x40 | 0xc0) || (a.index == b.index && entry.tier >= 5)
}

fn item(entered: &Entered, slot: Option<usize>) -> Option<Item> {
    entered
        .carry
        .get(slot.filter(|&s| s < 60)?)
        .copied()
        .filter(|i| !i.is_empty())
}

fn catalyst(list: &MixList, entered: &Entered) -> Option<(usize, i32)> {
    let (required, id) = requirement(list, 2)?;

    let matching = entered.carry[..60]
        .iter()
        .enumerate()
        .filter(|(_, i)| i.index == id && id != 0);

    let mut total = 0;

    let mut best = None;

    for (slot, value) in matching {
        let count = stack_size(value).max(0);

        total += count;

        if best.is_none_or(|(_, n)| count > n) {
            best = Some((slot, count));
        }
    }

    (total >= required).then_some((best?.0, total))
}

pub fn ready(
    state: &State,
    list: &MixList,
    items: &ItemList,
    entered: &Entered,
) -> Option<[usize; 4]> {
    let a = item(entered, state.pieces[0])?;

    let b = item(entered, state.pieces[1])?;

    if !compatible(items, &a, &b) {
        return None;
    }

    let jewel = item(entered, state.jewel)?;

    let (needed, _) = requirement(list, 3)?;

    if !accepts(list, items, 3, &jewel) || stack_size(&jewel) < needed {
        return None;
    }

    let entry = items.get(usize::from(a.index))?;

    if matches!(entry.equip_mask, 0x40 | 0xc0)
        && i32::from(jewel.index) != i32::from(entry.tier) + 2436
    {
        return None;
    }

    if i64::from(entered.gold) < i64::from(list.recipe(11)?.gold_cost) {
        return None;
    }

    Some([
        state.pieces[0]?,
        state.pieces[1]?,
        catalyst(list, entered)?.0,
        state.jewel?,
    ])
}

pub fn stage(panels: &mut Panels, entered: &Entered, slot: usize) {
    let Some(value) = item(entered, Some(slot)) else {
        return;
    };

    let Some(list) = panels.mixes.as_ref() else {
        return;
    };

    if panels.merge_item.pieces.contains(&Some(slot)) || panels.merge_item.jewel == Some(slot) {
        return;
    }

    let state = &mut panels.merge_item;

    state.message = None;

    if accepts(list, &panels.items, 3, &value) {
        if let Some(current) = item(entered, state.jewel)
            && (current.index != value.index || stack_size(&current) >= stack_size(&value))
        {
            return;
        }

        if let Some(a) = item(entered, state.pieces[0])
            && let Some(entry) = panels.items.get(usize::from(a.index))
            && matches!(entry.equip_mask, 0x40 | 0xc0)
            && (entry.tier < 5 || i32::from(value.index) != i32::from(entry.tier) + 2436)
        {
            state.message = Some(1660);

            return;
        }

        state.jewel = Some(slot);
    } else if accepts(list, &panels.items, 2, &value) {
        return;
    } else if let Some(role) = state.pieces.iter().position(Option::is_none) {
        if !accepts(list, &panels.items, role, &value) || !piece_allowed(&panels.items, &value) {
            state.message = Some(1660);
        } else if role == 1
            && !item(entered, state.pieces[0])
                .is_some_and(|a| compatible(&panels.items, &a, &value))
        {
            state.message = Some(1661);
        } else {
            state.pieces[role] = Some(slot);
        }
    }

    refresh(panels, entered);
}

fn is_refinement_effect(id: u8) -> bool {
    id == b'+' || (115..=126).contains(&id)
}

fn ordered(mut value: Item, first: bool) -> Item {
    let normalize = |value: &mut Item| {
        if let Some(at) = value
            .effects
            .iter()
            .position(|e| is_refinement_effect(e.id))
        {
            let source = value
                .effects
                .iter()
                .find(|e| e.id == b'+')
                .or_else(|| value.effects.iter().find(|e| is_refinement_effect(e.id)))
                .unwrap()
                .value;

            value.effects[at].value = source;

            for i in 0..3 {
                if i != at && is_refinement_effect(value.effects[i].id) {
                    value.effects[i] = Default::default();
                }
            }
        }
    };

    let order = |value: &mut Item| {
        for i in 1..3 {
            if (value.effects[0].id != b'+' && value.effects[i].id == b'+')
                || (!(115..=126).contains(&value.effects[0].id)
                    && (115..=126).contains(&value.effects[i].id))
            {
                value.effects.swap(0, i);
            }
        }
    };

    if first {
        order(&mut value);

        normalize(&mut value);
    } else {
        normalize(&mut value);

        order(&mut value);
    }

    value
}

pub fn preview(a: Item, b: Item, jewel: u16, variant: usize) -> Item {
    let mut a = ordered(a, true);

    let b = ordered(b, false);

    for i in 1..3 {
        if variant & (1 << (i - 1)) != 0 && !is_refinement_effect(b.effects[i].id) {
            a.effects[i] = b.effects[i];
        }
    }

    let level = if (2441..=2444).contains(&jewel) {
        jewel - 2431
    } else {
        10
    };

    if let Some(at) = a
        .effects
        .iter()
        .position(|e| e.id == b'+')
        .or_else(|| a.effects.iter().position(|e| is_refinement_effect(e.id)))
        .or_else(|| a.effects.iter().position(|e| e.id == 0))
    {
        if a.effects[at].id == 0 {
            a.effects[at].id = b'+';
        }

        a.effects[at].value = (level + 220) as u8;
    }

    a
}

pub fn refresh(panels: &mut Panels, entered: &Entered) {
    if !panels.is_open(CONTENT) {
        return;
    }

    let snapshot = Snapshot {
        carry: entered.carry[..60].try_into().unwrap(),
        gold: entered.gold,
        pieces: panels.merge_item.pieces,
        jewel: panels.merge_item.jewel,
        message: panels.merge_item.message,
    };

    if panels.merge_item.snapshot.as_deref() == Some(&snapshot) {
        return;
    }

    panels.merge_item.snapshot = Some(Box::new(snapshot));

    let state = panels.merge_item.clone();

    let Some(list) = panels.mixes.as_ref() else {
        return;
    };

    let valid = ready(&state, list, &panels.items, entered);

    let mut cells = Vec::new();

    for i in 0..2 {
        cells.push((
            format!("item{}", i + 1),
            item(entered, state.pieces[i]).unwrap_or_default(),
        ));
    }

    let (count, id) = requirement(list, 2).unwrap_or((1, 0));

    cells.push((
        "jewel0".into(),
        Item {
            index: id,
            appearance: count as u16,
            ..Default::default()
        },
    ));

    let jewel = item(entered, state.jewel);

    let count = jewel
        .map_or(0, |i| stack_size(&i))
        .min(requirement(list, 3).map_or(1, |r| r.0));

    for i in 0..4 {
        cells.push((
            format!("jewel{}", i + 1),
            if (i as i32) < count {
                Item {
                    appearance: 1,
                    ..jewel.unwrap()
                }
            } else {
                Item::default()
            },
        ));
    }

    for i in 0..4 {
        cells.push((
            format!("p{}", i + 1),
            valid.map_or(Item::default(), |_| {
                preview(
                    item(entered, state.pieces[0]).unwrap(),
                    item(entered, state.pieces[1]).unwrap(),
                    jewel.unwrap().index,
                    i,
                )
            }),
        ));
    }

    let cost = list.recipe(11).map_or(0, |r| r.gold_cost);

    let message = state.message.unwrap_or(if valid.is_some() {
        1653
    } else if compatible(
        &panels.items,
        &item(entered, state.pieces[0]).unwrap_or_default(),
        &item(entered, state.pieces[1]).unwrap_or_default(),
    ) && count < requirement(list, 3).map_or(1, |r| r.0)
    {
        1657
    } else {
        1656
    });

    let catalyst_missing = catalyst(list, entered).is_none();

    let locks = [
        state.pieces[0],
        state.pieces[1],
        catalyst(list, entered).map(|c| c.0),
        state.jewel,
    ];

    let Panels {
        screens: all,
        items,
        icons,
        strings,
        ..
    } = panels;

    if let Some(screen) = all.iter_mut().find(|s| s.content == CONTENT) {
        let resource = screen.placed.ui.resource.clone();

        for (name, value) in cells {
            screens::set_cell_icon(screen, &name, &value, items, icons, &resource);

            if let Some(element) =
                screen.element_mut(&name, formats::ui::pane::ElementType::IconButton)
            {
                element.visible = true;

                element.enabled = true;

                element.msg_receive = true;

                let Some(button) = element.widget.button_mut() else {
                    continue;
                };

                button.icon_locked = if name == "jewel0" {
                    catalyst_missing
                } else {
                    name.starts_with("jewel") && jewel.is_some() && value.is_empty()
                };
            }
        }

        for (name, id) in [
            ("TitleLabel", 1652),
            ("CostLabel", 1654),
            ("Item1Label", 1659),
            ("Item2Label", 1659),
            ("ConfirmBtn", 1655),
            ("PossibilityLabel", message),
        ] {
            if let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.name == name)
            {
                element.set_display(&strings.translate(&format!("ss_{id}")));

                if name == "ConfirmBtn" {
                    element.visible = true;

                    element.enabled = true;

                    element.msg_receive = true;
                }
            }
        }

        screens::text(
            screen,
            "cost",
            &screens::thousands(cost.max(0) as u64).replace(',', "."),
        );
    }

    if let Some(screen) = all.iter_mut().find(|s| s.content == CONTENT_INVENTORY) {
        for slot in 0..60 {
            screens::inventory_lock(screen, slot, locks.contains(&Some(slot)));
        }
    }
}

pub fn open(panels: &mut Panels, entered: &Entered) -> bool {
    if panels.screen(CONTENT).is_none() {
        return false;
    }

    panels.merge_item = State::default();

    panels.set_open(CONTENT, true);

    panels.pair_inventory(CONTENT, true);

    panels.set_open(CONTENT_INVENTORY, true);

    refresh(panels, entered);

    true
}

pub fn command(panels: &mut Panels, entered: &Entered, name: &str) {
    if !panels.is_open(CONTENT) {
        return;
    }

    match name {
        "item1" => panels.merge_item.pieces[0] = None,
        "item2" => panels.merge_item.pieces[1] = None,
        "jewel1" | "jewel2" | "jewel3" | "jewel4" => panels.merge_item.jewel = None,
        "ConfirmBtn" => {
            if panels
                .mixes
                .as_ref()
                .and_then(|l| ready(&panels.merge_item, l, &panels.items, entered))
                .is_some()
            {
                panels.raise_box(
                    &msgbox::Request {
                        caption: None,
                        kind: CONTENT as i32,
                        context: 0x5a,
                        body: msgbox::Body::StrDef(1658),
                        flags: msgbox::FLAGS,
                    },
                    None,
                );
            }

            return;
        }
        _ => return,
    }

    panels.merge_item.message = None;

    refresh(panels, entered);
}

pub fn confirm(panels: &mut Panels, entered: &Entered) {
    if !panels.is_open(CONTENT) {
        return;
    }

    let Some(slots) = panels
        .mixes
        .as_ref()
        .and_then(|l| ready(&panels.merge_item, l, &panels.items, entered))
    else {
        return;
    };

    let mut frame = vec![0; 132];

    frame[4..6].copy_from_slice(&[0xb5, 3]);

    frame[12..14].copy_from_slice(&11u16.to_le_bytes());

    for (i, slot) in slots.into_iter().enumerate() {
        frame[14 + i * 12..26 + i * 12].copy_from_slice(&entered.carry[slot].to_bytes());

        frame[110 + i] = slot as u8;
    }

    panels.wire.push(frame);

    panels.merge_item = State::default();

    refresh(panels, entered);
}
