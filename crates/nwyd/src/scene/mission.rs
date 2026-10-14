use formats::data::item_help::Help;
use formats::data::mission::{self, MissionItems};
use formats::net::account::Item;

pub const CONTENT: crate::frame::PanelId = 0x1E;

pub const MASTERY_GUARD: u16 = 0x43;

pub const ICON_COLUMNS: u32 = 10;

pub const ICON_CELL: u32 = 100;

#[derive(Debug, Clone)]

pub struct Tables {
    pub items: MissionItems,
    pub help: Vec<Help>,
    pub names: Vec<String>,
    pub overrides: std::collections::BTreeMap<(u32, u32), formats::data::mitem::Entry>,
}

impl Tables {
    pub fn fill_missing_help(
        &mut self,
        items: &formats::data::item_list::ItemList,
        icons: &formats::data::item_icon::IconTable,
        strings: &ui::Strings,
    ) {
        let text = |id| strings.strdef.as_ref().map_or("", |s| s.get(id));

        for (id, mission) in self.items.missions.iter().enumerate().take(5000) {
            if mission.is_empty() || self.names.get(id).is_some_and(|n| !n.is_empty()) {
                continue;
            }

            let (Some(name), Some(help)) = (self.names.get_mut(id), self.help.get_mut(id)) else {
                continue;
            };

            *name = format!("** Mission {id} **");

            let mut lines = vec![(String::from("                         "), 0xFFAA)];

            for row in 0..5 {
                let Some([kind, a, b, ..]) = mission.condition(8, row) else {
                    continue;
                };

                let line = match kind {
                    1 => format!("{}{a} ~ {b}", text(418)),
                    2 => {
                        format!(
                            "{}{}",
                            text(419),
                            match a {
                                0 => text(420),
                                1 => text(421),
                                2 => text(422),
                                _ => "",
                            }
                        )
                    }
                    3 => format!("{}{a}", text(423)),
                    5 => format!("{}{a}", text(424)),
                    _ => continue,
                };

                lines.push((line, 0xFFAA));
            }

            lines.push((String::from("------------------------------"), 0xFFAA));

            for row in 0..6 {
                let Some([kind, a, b, ..]) = mission.condition(9, row) else {
                    continue;
                };

                let line = match kind {
                    1 | 11 => {
                        let reward = mission.reward[0];

                        let stored = icons.index(usize::from(reward.index));

                        help.count = if stored > 0 { (stored - 1) as u16 } else { 0 };

                        let item_name = items.name(usize::from(reward.index)).unwrap_or_default();

                        let refine = if reward.effect[0].id == b'+' {
                            format!(" + {}", reward.effect[0].value)
                        } else {
                            String::new()
                        };

                        let quantity = if kind == 1 && b > 0 {
                            b
                        } else {
                            i32::from(reward.ex1 as i16).max(1)
                        };

                        format!("{}{item_name}{refine} ({quantity}{})", text(425), text(426))
                    }
                    2 => format!("{}{a}", text(427)),
                    3 => format!("{}{a}", text(428)),
                    4 => String::new(),
                    5 => text(429).to_string(),
                    6 => format!("{}{b}{}", text(430), text(431)),
                    7 => format!("{}{b}", text(430)),
                    8 => format!("{}{a}", text(435)),
                    9 => format!("{}{a}", text(436)),
                    _ => continue,
                };

                lines.push((line, 0xAAFF));
            }

            help.line = Default::default();

            help.color = [0; 9];

            help.color_full = [0; 9];

            for (slot, (line, color)) in lines.into_iter().take(9).enumerate() {
                help.line[slot] = line;

                help.color[slot] = color;

                help.color_full[slot] = i32::from(color as i16) as u32 & 0xFFFFFF;
            }
        }
    }

    pub fn tooltip(&self, id: i32) -> super::item_tip::Content {
        use ui::tooltip::Row;

        let mut rows = vec![Row::comment("")];

        if let Some(help) = usize::try_from(id).ok().and_then(|i| self.help.get(i))
            && help.color[0] != 0
        {
            rows.extend(
                help.line.iter().zip(help.color).map(|(line, color)| {
                    Row::new(line, i32::from(color as i16) as u32 | 0xFF000000)
                }),
            );
        }

        rows.push(Row::new("", 0xFFCCCCCC));

        super::item_tip::Content {
            title: Row::new(self.caption(id), 0xFFAAAAFF),
            rows,
            price: None,
            price_gap: false,
        }
    }

    pub fn caption(&self, id: i32) -> String {
        let raw = usize::try_from(id)
            .ok()
            .and_then(|i| self.names.get(i))
            .map(String::as_str);

        match raw {
            Some(name) if !name.is_empty() => {
                let trimmed = name.trim_start_matches('*').trim_end_matches('*');

                let trimmed = trimmed.strip_prefix('_').unwrap_or(trimmed);

                let trimmed = trimmed.strip_suffix('_').unwrap_or(trimmed);

                trimmed.trim().replace('_', " ")
            }
            _ => format!("** Mission {id}"),
        }
    }

    pub fn icon(&self, id: i32) -> u16 {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.help.get(i))
            .map_or(0, |h| h.count)
    }

    pub fn icon_sheet(&self, id: i32) -> Option<(i32, [i32; 4])> {
        let icon = i32::from(self.icon(id));

        if icon == 0 {
            return None;
        }

        let cell = icon % 100;

        let cols = ICON_COLUMNS as i32;

        let edge = ICON_CELL as i32;

        let (u, v) = ((cell % cols) * edge, (cell / cols) * edge);

        Some((icon / 100 + 1, [u, v, u + edge, v + edge]))
    }

    pub fn carried(&self, id: i32) -> Item {
        let reward = usize::try_from(id)
            .ok()
            .and_then(|i| self.items.get(i))
            .map(|m| m.reward[0])
            .unwrap_or_default();

        if reward.is_empty() {
            return Item {
                index: id as u16,
                ..Item::default()
            };
        }

        Item {
            index: reward.index,
            appearance: reward.ex1,
            reserved: reward.ex2,
            effects: [
                formats::net::account::Effect {
                    id: reward.effect[0].id,
                    value: reward.effect[0].value,
                },
                formats::net::account::Effect {
                    id: reward.effect[1].id,
                    value: reward.effect[1].value,
                },
                formats::net::account::Effect {
                    id: reward.effect[2].id,
                    value: reward.effect[2].value,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Click {
    Slot(i32),
    Material(usize),
    Back,
    Confirm,
}

pub fn confirmation() -> super::msgbox::Request {
    super::msgbox::Request {
        caption: None,
        kind: 0x1E,
        context: 0x1A,
        body: super::msgbox::Body::StrDef(319),
        flags: 0x20000001,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Refused {
    NothingSelected,
    ShortOfMaterials { group: usize, have: i32, need: i32 },
    NoSourceSlot { group: usize },
    UnbuiltRule { group: usize, kind: i32 },
    NotEnoughMoney { have: i32, need: i32 },
    WrongLevel { level: i32, min: i32, max: i32 },
    WrongEvolution { have: i32, need: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Slot {
    pub id: i32,
    pub caption: String,
    pub carried: Item,
    pub leaf: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Details {
    pub material: [Item; 8],
    pub counts: [Option<(i32, i32)>; 8],
    pub captions: [Option<String>; 8],
    pub money: Option<i32>,
    pub locked: Vec<usize>,
    staged: [Option<usize>; 8],
    pub unbuilt: Vec<(usize, i32)>,
}

fn alternative_matches(held: &Item, template: &formats::data::Item) -> bool {
    (template.index == 0xFFFF || template.index == held.index)
        && (template.ex1 == 0xFFFF || template.ex1 == held.appearance)
        && (template.ex2 >> 8 == 0xFF || template.ex2 >> 8 == held.reserved >> 8)
        && template
            .effect
            .iter()
            .zip(held.effects)
            .all(|(wanted, got)| {
                (wanted.id == 0xFF || wanted.id == got.id)
                    && (wanted.value == 0xFF || wanted.value == got.value)
            })
}

fn generic_staging_count(
    held: &Item,
    template: &Item,
    items: &formats::data::item_list::ItemList,
) -> usize {
    if held.is_empty() || held.index != template.index || !(1..0x38A4).contains(&template.index) {
        return 0;
    }

    let effect = template.effects[0].id;

    let same_refine = || {
        formats::mesh::appearance::refine_level(template, items)
            == formats::mesh::appearance::refine_level(held, items)
    };

    if effect == 0xFF || effect == held.effects[0].id {
        usize::from(if effect == b'+' {
            same_refine()
        } else {
            (template.appearance as i16).max(1) == (held.appearance as i16).max(1)
        })
    } else if template.appearance != 0 {
        if super::consume::stored_count(template) != super::consume::stored_count(held) {
            0
        } else {
            1 + usize::from(effect == b'+' && same_refine())
        }
    } else {
        usize::from(effect == b'+' && same_refine())
    }
}

impl Details {
    fn select_alternative(&mut self, row: &mission::Mission, carry: &[Item], slot: usize) -> bool {
        let Some(held) = carry.get(slot).filter(|held| slot < 60 && !held.is_empty()) else {
            return false;
        };

        if row.rule(0).is_none_or(|rule| rule.kind != 9) {
            return false;
        }

        let alternatives: Vec<_> = (0..8)
            .map_while(|choice| row.alternative(choice))
            .take_while(|(template, _)| template.index != 0)
            .collect();

        let mut selected = None;

        let mut last = None;

        let mut remaining = 0i32;

        let mut contributors = [false; 60];

        for (choice, (template, need)) in alternatives.iter().enumerate() {
            if !alternative_matches(held, template) {
                continue;
            }

            last = Some(choice);

            let have = super::consume::stored_count(held);

            remaining = (*need as i32).saturating_sub(have);

            contributors[slot] = have.min(*need as i32) != 0;

            if remaining <= 0 {
                selected = Some(choice);
            }
        }

        if selected.is_none() {
            let Some(choice) = last else {
                return false;
            };

            let template = &alternatives[choice].0;

            for (other, item) in carry.iter().take(60).enumerate() {
                if other == slot || !alternative_matches(item, template) {
                    continue;
                }

                contributors[other] = true;

                remaining -= super::consume::stored_count(item);

                if remaining <= 0 {
                    selected = Some(choice);

                    break;
                }
            }
        }

        let Some(choice) = selected else {
            return false;
        };

        let (template, need) = alternatives[choice];

        self.locked.retain(|&old| {
            !alternatives
                .iter()
                .any(|(template, _)| alternative_matches(&carry[old], template))
        });

        for (other, &used) in contributors.iter().enumerate() {
            if used && !self.locked.contains(&other) {
                self.locked.push(other);
            }
        }

        self.staged[0] = Some(slot);

        self.counts[0] = Some((need as i32, need as i32));

        self.material[0] = Item {
            index: held.index,
            appearance: template.ex1,
            reserved: template.ex2,
            effects: template.effect.map(|e| formats::net::account::Effect {
                id: e.id,
                value: e.value,
            }),
        };

        true
    }

    fn alternatives(&mut self, row: &mission::Mission, carry: &[Item]) {
        let mut best = self.counts[0].map_or(0, |(have, _)| have) as u32;

        for choice in 0..8 {
            let Some((template, need)) = row.alternative(choice) else {
                break;
            };

            if template.index == 0 {
                break;
            }

            let mut candidate = Vec::new();

            let mut have = 0u32;

            let mut complete = false;

            for (slot, held) in carry.iter().take(60).enumerate() {
                if self.locked.contains(&slot) || !alternative_matches(held, &template) {
                    continue;
                }

                candidate.push(slot);

                have += super::consume::stored_count(held) as u32;

                if have as i32 >= need as i32 {
                    complete = true;

                    break;
                }
            }

            if complete || have > best {
                if complete || best != 0 {
                    self.locked.clear();
                }

                best = have.min(need);

                self.locked.extend(candidate.iter().copied());

                if let Some(&first) = candidate.first() {
                    self.staged[0] = Some(first);

                    self.material[0] = Item {
                        index: carry[first].index,
                        appearance: template.ex1,
                        reserved: template.ex2,
                        effects: template.effect.map(|e| formats::net::account::Effect {
                            id: e.id,
                            value: e.value,
                        }),
                    };
                }

                self.counts[0] = Some((best as i32, need as i32));
            }

            if complete {
                break;
            }
        }

        if self.counts[0].is_none() {
            self.counts[0] = Some((0, row.alternative(0).map_or(1, |(_, n)| n as i32)));
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct State {
    pub key: i32,
    pub npc: u16,
    pub slot: Vec<Slot>,
    pub trail: Vec<i32>,
    pub selected: Option<i32>,
    manual_material: Option<usize>,
}

impl State {
    pub fn open(tables: &Tables, key: i32, npc: u16) -> State {
        let mut state = State {
            key,
            npc,
            ..State::default()
        };

        state.fill(
            tables,
            tables.items.menu(key).iter().map(|m| m.id).collect(),
        );

        state
    }

    pub fn drill(&mut self, tables: &Tables, id: i32) -> bool {
        let Some(row) = usize::try_from(id).ok().and_then(|i| tables.items.get(i)) else {
            return false;
        };

        if row.children().next().is_none() {
            return false;
        }

        let ids: Vec<i32> = tables
            .items
            .children_of(id as usize)
            .iter()
            .map(|m| m.id)
            .collect();

        self.trail.push(id);

        self.selected = None;

        self.fill(tables, ids);

        true
    }

    pub fn back(&mut self, tables: &Tables) -> bool {
        let Some(first) = self
            .slot
            .first()
            .and_then(|s| usize::try_from(s.id).ok())
            .filter(|&id| (1..5000).contains(&id))
        else {
            return false;
        };

        self.selected = Some(first as i32);

        if tables.items.get(first).is_none_or(|m| m.parent == -1) {
            return false;
        }

        self.trail.pop();

        self.selected = None;

        let ids = tables
            .items
            .siblings_of_parent(first)
            .iter()
            .map(|m| m.id)
            .collect();

        self.fill(tables, ids);

        true
    }

    pub fn choose(&mut self, tables: &Tables, id: i32) -> bool {
        if self.slot.iter().all(|s| s.id != id) {
            return false;
        }

        if self.drill(tables, id) {
            return true;
        }

        self.manual_material = None;

        self.selected = Some(id);

        true
    }

    fn select_material(
        &mut self,
        tables: &Tables,
        carry: &[Item],
        items: &formats::data::item_list::ItemList,
        slot: usize,
    ) -> bool {
        let Some(row) = self
            .selected
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| tables.items.get(id))
        else {
            return false;
        };

        let mut details = self.details(tables, carry, items);

        if details.locked.contains(&slot) || !details.select_alternative(row, carry, slot) {
            return false;
        }

        self.manual_material = Some(slot);

        true
    }

    pub fn details(
        &self,
        tables: &Tables,
        carry: &[Item],
        items: &formats::data::item_list::ItemList,
    ) -> Details {
        let mut out = Details::default();

        let Some(row) = self
            .selected
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| tables.items.get(id))
        else {
            return out;
        };

        let mut fallback = false;

        for cell in 0..8 {
            if let Some([3, value, ..]) = row.condition(8, cell) {
                out.money = Some(value);
            }

            let mut material = row.material[cell];

            if !(1..0x38A4).contains(&material.index)
                && !fallback
                && let Some((alternative, _)) = row.alternative(cell)
                && (1..0x38A4).contains(&alternative.index)
            {
                material = alternative;

                fallback = true;
            }

            if (1..0x38A4).contains(&material.index) {
                out.material[cell] = Item {
                    index: material.index,
                    appearance: material.ex1,
                    reserved: material.ex2,
                    effects: material.effect.map(|e| formats::net::account::Effect {
                        id: e.id,
                        value: e.value,
                    }),
                };
            }
        }

        for group in 0..8 {
            let Some(rule) = row.rule(group) else {
                break;
            };

            if rule.kind == 9 {
                out.alternatives(row, carry);

                continue;
            }

            if rule.kind == 2 {
                let template = row.material[group];

                let template = Item {
                    index: template.index,
                    appearance: template.ex1,
                    reserved: template.ex2,
                    effects: template.effect.map(|e| formats::net::account::Effect {
                        id: e.id,
                        value: e.value,
                    }),
                };

                let selected = carry.iter().take(60).enumerate().find_map(|(slot, held)| {
                    if out.locked.contains(&slot) {
                        return None;
                    }

                    let copies = generic_staging_count(held, &template, items);

                    (copies != 0).then_some((slot, copies))
                });

                out.counts[group] = Some((i32::from(selected.is_some()), 1));

                if let Some((slot, copies)) = selected {
                    out.locked.push(slot);

                    for dest in out.staged[group..]
                        .iter_mut()
                        .filter(|s| s.is_none())
                        .take(copies)
                    {
                        *dest = Some(slot);
                    }
                }

                continue;
            }

            if rule.kind != 6 {
                out.unbuilt.push((group, rule.kind));

                continue;
            }

            let mut have = 0;

            for (slot, held) in carry.iter().take(60).enumerate() {
                if out.locked.contains(&slot)
                    || held.is_empty()
                    || held.index != row.material[group].index
                {
                    continue;
                }

                have += super::consume::stored_count(held);

                out.locked.push(slot);

                out.staged[group] = Some(slot);

                if have >= rule.count {
                    have = rule.count;

                    break;
                }
            }

            out.counts[group] = Some((have, rule.count));
        }

        if let Some(slot) = self.manual_material {
            out.select_alternative(row, carry, slot);
        }

        for cell in 0..8 {
            if out.staged[cell].is_some()
                || (out.material[cell].index != 0
                    && out.material[cell].index != row.material[cell].index)
            {
                continue;
            }

            if let Some(entry) = self
                .selected
                .and_then(|id| tables.overrides.get(&(id as u32, cell as u32)))
            {
                if entry.text.is_empty() {
                    continue;
                }

                out.material[cell].index = entry.icon;

                out.captions[cell] =
                    Some(formats::text::codepage::decode(&entry.text, entry.codepage));
            }
        }

        out
    }

    pub fn materials(
        &self,
        tables: &Tables,
        carry: &[Item],
        items: &formats::data::item_list::ItemList,
    ) -> Result<(Vec<formats::data::Item>, Vec<i8>), Refused> {
        let convert = |held: &Item| formats::data::Item::parse(&held.to_bytes());

        let mut item = vec![formats::data::Item::default(); mission::MATERIALS];

        let mut position = vec![-1i8; mission::MATERIALS];

        let Some(id) = self.selected else {
            return Err(Refused::NothingSelected);
        };

        let Some(row) = usize::try_from(id).ok().and_then(|i| tables.items.get(i)) else {
            return Err(Refused::NothingSelected);
        };

        let details = self.details(tables, carry, items);

        if let Some(&(group, kind)) = details.unbuilt.first() {
            return Err(Refused::UnbuiltRule { group, kind });
        }

        for group in 0..mission::MATERIALS {
            if let Some((have, need)) = details.counts[group] {
                if have < need {
                    return Err(Refused::ShortOfMaterials { group, have, need });
                }

                if details.staged[group].is_none() {
                    return Err(Refused::NoSourceSlot { group });
                }
            }

            let have = details.counts[group].map_or(0, |(have, _)| have);

            for condition in 0..20 {
                if let Some([6, _, need, ..]) = row.condition(group, condition)
                    && have < need
                {
                    return Err(Refused::ShortOfMaterials { group, have, need });
                }
            }

            if let Some(slot) = details.staged[group] {
                item[group] = convert(&carry[slot]);

                position[group] = slot as i8;
            }
        }

        Ok((item, position))
    }

    pub fn confirm(
        &self,
        tables: &Tables,
        carry: &[Item],
        items: &formats::data::item_list::ItemList,
    ) -> Result<formats::net::mission::Confirm, Refused> {
        let Some(mission) = self.selected else {
            return Err(Refused::NothingSelected);
        };

        let (item, position) = self.materials(tables, carry, items)?;

        let mut frame = formats::net::mission::Confirm {
            mission,
            ..Default::default()
        };

        frame.item.copy_from_slice(&item[..mission::MATERIALS]);

        frame
            .position
            .copy_from_slice(&position[..mission::MATERIALS]);

        Ok(frame)
    }

    fn admission(&self, tables: &Tables, level: i32, tier: i32, gold: i32) -> Result<(), Refused> {
        let Some(row) = self
            .selected
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| tables.items.get(id))
        else {
            return Err(Refused::NothingSelected);
        };

        for condition in 0..20 {
            match row.condition(8, condition) {
                Some([1, min, max, ..]) if level < min || level > max => {
                    return Err(Refused::WrongLevel { level, min, max });
                }
                Some([2, need, ..]) if tier != need => {
                    return Err(Refused::WrongEvolution { have: tier, need });
                }
                Some([3, need, ..]) if gold < need => {
                    return Err(Refused::NotEnoughMoney { have: gold, need });
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn fill(&mut self, tables: &Tables, ids: Vec<i32>) {
        self.manual_material = None;

        self.slot = ids
            .into_iter()
            .take(mission::OPTIONS)
            .map(|id| Slot {
                id,
                caption: tables.caption(id),
                carried: tables.carried(id),
                leaf: usize::try_from(id)
                    .ok()
                    .and_then(|i| tables.items.get(i))
                    .is_none_or(|m| m.children().next().is_none()),
            })
            .collect();
    }
}

fn wire(confirm: &formats::net::mission::Confirm) -> Vec<u8> {
    let mut out = vec![0u8; formats::net::mission::FRAME_LEN];

    protocol::header::set_opcode(&mut out, formats::net::mission::CONFIRM);

    out[protocol::header::LEN..].copy_from_slice(&confirm.to_bytes());

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Outcome {
    Navigated { to: i32 },
    MaterialSelected { slot: usize },
    Sent { mission: i32 },
    Refused(Refused),
}

impl Outcome {
    pub fn describe(&self) -> String {
        match self {
            Outcome::Navigated { to } => format!("NWYD: mission menu -> {to}"),
            Outcome::MaterialSelected { slot } => {
                format!("NWYD: mission material selected from inventory {slot}")
            }
            Outcome::Sent { mission } => {
                format!("NWYD: mission {mission} confirmed (0x2e7)")
            }
            Outcome::Refused(why) => {
                format!("NWYD: mission not sent — {}", why.describe())
            }
        }
    }
}

impl Refused {
    fn banner(&self) -> Option<usize> {
        match self {
            Self::ShortOfMaterials { have, .. } => Some(if *have == 0 { 412 } else { 413 }),
            Self::NoSourceSlot { .. } => Some(412),
            Self::NotEnoughMoney { .. } => Some(414),
            Self::WrongLevel { .. } => Some(415),
            Self::WrongEvolution { .. } => Some(416),
            Self::NothingSelected | Self::UnbuiltRule { .. } => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Refused::NothingSelected => String::from("no option is selected"),
            Refused::ShortOfMaterials { group, have, need } => {
                format!("group {group} holds {have} of the {need} it needs")
            }
            Refused::NoSourceSlot { group } => {
                format!("group {group} found no bag slot — Position[{group}] is still -1")
            }
            Refused::UnbuiltRule { group, kind } => {
                format!("group {group} wants rule kind {kind}, which this port does not fill")
            }
            Refused::NotEnoughMoney { have, need } => {
                format!("gold {have} is below {need}")
            }
            Refused::WrongLevel { level, min, max } => {
                format!("level {level} is outside {min}..{max}")
            }
            Refused::WrongEvolution { have, need } => {
                format!("evolution {have} is not {need}")
            }
        }
    }
}

pub fn panel_clicks(scene: &mut super::field::FieldScene) -> Vec<Outcome> {
    if let Some(panels) = scene.panels.as_mut() {
        if std::mem::take(&mut panels.mission_reset) {
            scene.mission = None;
        }

        if !panels.is_open(CONTENT) {
            panels.mission_clicks.clear();

            return Vec::new();
        }
    }

    let pending: Vec<Click> = match scene.panels.as_mut() {
        Some(panels) => std::mem::take(&mut panels.mission_clicks),
        None => return Vec::new(),
    };

    let mut out = Vec::new();

    let tier = super::cc::evolution_tier(scene);

    for click in pending {
        let Some(panels) = scene.panels.as_mut() else {
            break;
        };

        let Some(tables) = panels.missions.clone() else {
            break;
        };

        let Some(state) = scene.mission.as_mut() else {
            break;
        };

        match click {
            Click::Slot(_) | Click::Back | Click::Material(_) => {
                let id = match click {
                    Click::Slot(id) => {
                        if !state.choose(&tables, id) {
                            continue;
                        }

                        id
                    }
                    Click::Back => {
                        if !state.back(&tables) {
                            continue;
                        }

                        state.slot.first().map_or(0, |s| s.id)
                    }
                    Click::Material(slot) => {
                        if !state.select_material(
                            &tables,
                            &scene.entered.carry,
                            &panels.items,
                            slot,
                        ) {
                            continue;
                        }

                        state.selected.unwrap_or(0)
                    }
                    Click::Confirm => unreachable!(),
                };

                out.push(match click {
                    Click::Material(slot) => Outcome::MaterialSelected { slot },
                    _ => Outcome::Navigated { to: id },
                });

                let state = state.clone();

                if let Some(panels) = scene.panels.as_mut() {
                    if !matches!(click, Click::Material(_)) {
                        panels.refresh_mission(&state);
                    }

                    let details = state.details(&tables, &scene.entered.carry, &panels.items);

                    if let Some(bag) = panels.screen_mut(super::panels::CONTENT_INVENTORY) {
                        for slot in 0..60 {
                            super::screens::inventory_lock(
                                bag,
                                slot,
                                details.locked.contains(&slot),
                            );
                        }
                    }

                    let super::panels::Panels {
                        screens,
                        items,
                        icons,
                        ..
                    } = panels;

                    if let Some(screen) = screens.iter_mut().find(|s| s.content == CONTENT) {
                        super::screens::mission_details(screen, &details, items, icons);
                    }
                }
            }
            Click::Confirm => {
                let carry = scene.entered.carry;

                let result = state
                    .confirm(&tables, &carry, &panels.items)
                    .and_then(|frame| {
                        state
                            .admission(
                                &tables,
                                i32::from(scene.entered.status.level as i16) + 1,
                                tier,
                                scene.entered.gold as i32,
                            )
                            .map(|()| frame)
                    });

                match result {
                    Ok(frame) => {
                        let mission = frame.mission;

                        scene.outbox.push(wire(&frame));

                        out.push(Outcome::Sent { mission });
                    }
                    Err(why) => {
                        if let Some(record) = why.banner() {
                            let text = panels
                                .strings
                                .strdef
                                .as_ref()
                                .map_or("", |s| s.get(record))
                                .to_string();

                            super::notice::post(panels, &text, 2000, true, panels.now_ms);
                        }

                        out.push(Outcome::Refused(why));
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]

mod tests {

    use super::*;
    use formats::{
        data::{item_list, mitem},
        text::Codepage,
    };

    #[test]

    fn material_override_captions_use_each_entry_codepage() {
        let row = mission::Mission {
            gold_cost: 0,
            key: 0,
            id: 1,
            parent: -1,
            options: [0; mission::OPTIONS],
            material: [formats::data::Item::default(); mission::MATERIALS],
            reward: [formats::data::Item::default(); mission::REWARDS],
            tail: Vec::new(),
        };

        let mut empty = row.clone();

        empty.id = 0;

        let mut overrides = std::collections::BTreeMap::new();

        mitem::overlay(&mut overrides, b"M-ITEM 1 0 21 Pre\xE7o \x80\n");

        mitem::overlay_with_codepage(
            &mut overrides,
            b"M-ITEM 1 1 22 Za\xBF\xF3\xB3\xE6\n",
            Codepage::Windows1250,
        );

        mitem::overlay_with_codepage(
            &mut overrides,
            b"M-ITEM 1 2 23 \xC7\xD1\xB1\xB9\xBE\xEE\n",
            Codepage::Windows949,
        );

        let tables = Tables {
            items: MissionItems {
                missions: vec![empty, row],
            },
            help: Vec::new(),
            names: Vec::new(),
            overrides,
        };

        let state = State {
            selected: Some(1),
            ..Default::default()
        };

        let items =
            item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252);

        let details = state.details(&tables, &[], &items);

        for (cell, caption) in ["Preço €", "Zażółć", "한국어"].into_iter().enumerate() {
            assert_eq!(details.captions[cell].as_deref(), Some(caption));

            assert_eq!(details.material[cell].index, 21 + cell as u16);
        }

        assert!(details.captions[3..].iter().all(Option::is_none));
    }
}
