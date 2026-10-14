use super::actors::Actor;
use super::cast;
use super::cc;
use super::nameplate;
use crate::fonts::Chains;
use crate::frame::{AmulSheet, PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::login_panels::{GuiTexture, PlacedPanel};
use crate::scene::login_static::Layout;
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_list::ItemList;
use formats::data::skill_data;
use formats::net::account::Status;
use formats::net::affect;
use formats::text::Codepage;
use formats::text::string_table::{
    EFFECT_STRING_MAX_LEN, EFFECT_STRING_SLOTS, EFFECT_SUBSTRING_SLOTS, parse_token_list,
};
use formats::ui::pane::ElementType;
use render::device::Gpu;
use render::preset::RenderMode;
use render::texture::AlphaPolicy;
use std::collections::BTreeMap;
use ui::Strings;
use ui::draw::{Interaction, Owner};
use ui::input::{Event, EventId, Manager, Mouse, PanelMut, PanelRef};
use ui::widget::cell_group;

pub const PANEL_HUD: &[u8] = b"ui/Panel/game/HUD.pane";

pub const PANEL_MY_INFO: &[u8] = b"ui/Panel/game/HudMyInfo.pane";

pub const PANEL_YOU_INFO: &[u8] = b"ui/Panel/game/HudYouInfo.pane";

pub const CONTENT_HUD: PanelId = 0x10;

pub const CONTENT_MY_INFO: PanelId = 0x11;

pub const CONTENT_YOU_INFO: PanelId = 0x12;

const HUD_INPUT_ORDER: [usize; 3] = [2, 0, 1];

pub const NUI_MODE: RenderMode = RenderMode::NuiPanel2d;

pub const CC_MODE_TAG: &str = "CCMode";

pub const MENU_TAG: &str = "Menu";

pub const HIDDEN_STATICS: [&str; 2] = ["QuestRemainTime", "Debug"];

pub const EXP_SEGMENTS: usize = 10;

pub const LEVEL_FORMAT: &str = "Lv. ";

pub const SYMBOLS: i32 = 12;

pub const SYMBOL_TIERS: i32 = 3;

pub const HP_BAR_SKINS: [&str; 4] = [
    "UI_Hud.TopBarHPBar",
    "UI_Hud.TopBarHPBar1",
    "UI_Hud.TopBarHPBar2",
    "UI_Hud.TopBarHPBar3",
];

pub const POISON_AFFECT: u16 = 0x301;

pub const AUTO_SKILLS: usize = 10;

pub const SKILL_CELLS: usize = formats::net::enter::QUICK_SLOTS;

pub const QUICK_CELLS: usize = 5;

pub const SKILL_TAB_TEXT: &str = "SkillTabText";

pub const PAGES: i32 = 2;

pub const PAGE_CELLS: usize = formats::net::enter::QUICK_PAGE_SLOTS;

pub const SKILL_ID_BASE: i32 = 5_000;

pub const SECOND_WINDOW_ID_BASE: i32 = 0x14AF;

pub const SECOND_WINDOW_CODE: u8 = 0x69;

pub fn skill_book_id(code: u8) -> i32 {
    match code < SECOND_WINDOW_CODE {
        true => i32::from(code) + SKILL_ID_BASE,
        false => i32::from(code) + SECOND_WINDOW_ID_BASE,
    }
}

pub fn skill_tab_text(page: i32) -> String {
    format!("{}", page + 1)
}

pub fn sweep_row(code: u8) -> usize {
    match code < SECOND_WINDOW_CODE {
        true => usize::from(code),
        false => usize::from(code.wrapping_add(0x5F)),
    }
}

pub const BUFF_CELLS: usize = affect::AFFECTS;

pub const EXP_LAMPS: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct EffectStrings {
    names: Vec<String>,
    subnames: Vec<String>,
}

impl EffectStrings {
    pub fn parse(main: &[u8], sub: &[u8], codepage: Codepage) -> EffectStrings {
        EffectStrings {
            names: parse_token_list(main, EFFECT_STRING_SLOTS, EFFECT_STRING_MAX_LEN, codepage),
            subnames: parse_token_list(
                sub,
                EFFECT_SUBSTRING_SLOTS,
                EFFECT_STRING_MAX_LEN,
                codepage,
            ),
        }
    }

    pub fn load(game: &GameDir) -> EffectStrings {
        let (main, main_cp) =
            crate::ui::read_language(game, "EffectString.txt").unwrap_or_default();

        let (sub, sub_cp) =
            crate::ui::read_language(game, "EffectSubString.txt").unwrap_or_default();

        EffectStrings {
            names: parse_token_list(&main, EFFECT_STRING_SLOTS, EFFECT_STRING_MAX_LEN, main_cp),
            subnames: parse_token_list(&sub, EFFECT_SUBSTRING_SLOTS, EFFECT_STRING_MAX_LEN, sub_cp),
        }
    }

    pub fn main(&self, kind: u8) -> &str {
        kind.checked_sub(1)
            .and_then(|index| self.names.get(index as usize))
            .map_or("", String::as_str)
    }

    pub fn sub(&self, bit: usize) -> &str {
        self.subnames.get(bit).map_or("", String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct AffectTooltip {
    pub kind: u8,
    pub value: u16,
    pub time_8s: i32,
    pub seconds: i32,
    pub slot: i16,
}

impl AffectTooltip {
    fn local(detail: &affect::Detail, slot: usize, seconds: i32) -> AffectTooltip {
        AffectTooltip {
            kind: detail.kind,
            value: detail.value,
            time_8s: detail.time_8s,
            seconds,
            slot: slot as i16,
        }
    }

    fn target(affect: &affect::Affect) -> AffectTooltip {
        AffectTooltip {
            kind: affect.kind,
            value: 0,
            time_8s: 0,
            seconds: 0,
            slot: -1,
        }
    }

    pub fn name(&self, strings: &EffectStrings) -> Option<String> {
        let mut text = String::new();

        if self.kind == 8 {
            for bit in 0..EFFECT_SUBSTRING_SLOTS {
                if self.value & (1 << bit) != 0 {
                    text.push(' ');

                    text.push_str(strings.sub(bit));
                }
            }
        } else {
            text.push_str(strings.main(self.kind));
        }

        (!text.is_empty()).then_some(text)
    }
}

pub const LEVEL_PARTS: i64 = 4;

pub const MENU_BUTTON: &str = "Menu";

pub const CC_MENU_BUTTON: &str = "CCMenu";

pub const CC_BUTTONS: [&str; 4] = ["CCMode", "CCModePo", "CCModeMo", "CCModeMove"];

pub const MENU_ARMS: [(&str, Option<PanelId>); 8] = [
    ("BtnChar", Some(0x14)),
    ("BtnInven", Some(0x17)),
    ("BtnQuest", Some(0x1F)),
    ("BtnAtrade", Some(0x1C)),
    ("BtnHelp", Some(0x21)),
    ("BtnConfig", Some(0x25)),
    ("BtnCommu", None),
    ("BtnCollect", Some(0x38)),
];

pub const SKIN_LAYERS: usize = 3;

pub const CC_MODES: i32 = 4;

pub const CC_MODE_TOOLTIPS: [&str; 4] = ["uu_229", "uu_226", "uu_227", "uu_230"];

pub const MOVE_MODES: i32 = 3;

pub const MOVE_TOOLTIPS: [&str; 3] = ["uu_232", "uu_233", ""];

pub const MOVE_SKIN_REVERSED: bool = true;

pub const THRESHOLD_STEP: i32 = 10;

pub const THRESHOLD_CEILING: i32 = 91;

pub const POTION_FLOOR: i32 = 0;

pub const FEED_FLOOR: i32 = 30;

pub const THRESHOLD_DEFAULT: i32 = 30;

pub const MOVE_DEFAULT: i32 = 2;

pub const MOVE_PLANS_A_PATH: i32 = 1;

pub const CC_MODE_DEFAULT: i32 = 0;

pub const LOCK_MS: u32 = 6_000;

pub const LOCK_STAMPS: usize = 8;

pub const DEBOUNCE_MS: u32 = 200;

pub const FIELD_STATE: i32 = 30_000;

pub const TOAST_CC_MENU: usize = 476;

pub const TOAST_FEED_OFF: usize = 477;

pub const TOAST_FEED_ON: usize = 478;

pub const TOAST_POTION_OFF: usize = 479;

pub const TOAST_POTION_ON: usize = 480;

pub const TOAST_COLOR: u32 = 0xFFFF_AAAA;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Experience {
    pub lamps: usize,
    pub tenth: i64,
    pub segments: [i64; EXP_SEGMENTS],
}

impl Experience {
    pub fn new(exp: i64, span: (i64, i64)) -> Experience {
        let (low, high) = span;

        let quarter = (high - low) / LEVEL_PARTS;

        let progress = (exp - low).max(0);

        let part = match quarter {
            0 => 1,
            q => (1 + progress / q).min(LEVEL_PARTS),
        };

        let tenth = (quarter as f64 * 0.1) as i64;

        let mut remainder = progress - (part - 1) * quarter;

        let mut segments = [0i64; EXP_SEGMENTS];

        for segment in &mut segments {
            if remainder <= tenth {
                *segment = remainder.clamp(0, tenth);

                remainder = 0;
            } else {
                remainder -= tenth;

                *segment = tenth;
            }
        }

        Experience {
            lamps: (part - 1).clamp(0, EXP_LAMPS as i64) as usize,
            tenth,
            segments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Combat {
    pub mode: i32,
    pub potion: i32,
    pub feed: i32,
    pub movement: i32,
}

impl Default for Combat {
    fn default() -> Combat {
        Combat {
            mode: CC_MODE_DEFAULT,
            potion: THRESHOLD_DEFAULT,
            feed: THRESHOLD_DEFAULT,
            movement: MOVE_DEFAULT,
        }
    }
}

impl Combat {
    pub fn cycle_mode(&mut self) -> i32 {
        self.mode += 1;

        if self.mode >= CC_MODES {
            self.mode = 0;
        }

        self.mode
    }

    pub fn cycle_potion(&mut self) -> i32 {
        self.potion = Combat::advance(self.potion, POTION_FLOOR);

        self.potion
    }

    pub fn cycle_feed(&mut self) -> i32 {
        self.feed = Combat::advance(self.feed, FEED_FLOOR);

        self.feed
    }

    fn advance(percent: i32, floor: i32) -> i32 {
        let next = percent + THRESHOLD_STEP;

        match next >= THRESHOLD_CEILING {
            true => floor,
            false => next,
        }
    }

    pub fn cycle_movement(&mut self) -> i32 {
        self.movement += 1;

        if self.movement >= MOVE_MODES {
            self.movement = 0;
        }

        self.movement
    }

    pub fn mode_skin(&self) -> Option<(String, &'static str)> {
        let key = *CC_MODE_TOOLTIPS.get(usize::try_from(self.mode).ok()?)?;

        Some((format!("UI_Hud.CCMode{}", self.mode), key))
    }

    pub fn movement_skin(&self) -> Option<(String, &'static str)> {
        let index = usize::try_from(self.movement).ok()?;

        let key = *MOVE_TOOLTIPS.get(index)?;

        let picture = match MOVE_SKIN_REVERSED {
            true => MOVE_MODES - 1 - self.movement,
            false => self.movement,
        };

        Some((format!("UI_Hud.CCModeMove{picture}"), key))
    }

    pub fn threshold_text(percent: i32) -> String {
        format!("{percent}%")
    }

    pub fn plans_a_path(&self) -> bool {
        self.movement == MOVE_PLANS_A_PATH
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct InputLock {
    pub stamps: [u32; LOCK_STAMPS],
}

impl InputLock {
    pub fn allows(&self, now_ms: u32) -> bool {
        (0..self.stamps.len()).all(|slot| !self.locked(slot, now_ms))
    }

    pub fn locked(&self, slot: usize, now_ms: u32) -> bool {
        self.stamps
            .get(slot)
            .is_some_and(|&stamp| stamp != 0 && now_ms < stamp.wrapping_add(LOCK_MS))
    }

    pub fn stamp(&mut self, slot: usize, now_ms: u32) {
        if let Some(entry) = self.stamps.get_mut(slot) {
            *entry = now_ms;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Cycle {
    Mode(i32),
    Potion(i32),
    Feed(i32),
    Movement(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Command {
    SelectSkill(usize),
    SelectQuick(usize),
    UnboundSkill,
    UnboundQuick,
    Menu(bool),
    CcMenu(bool),
    Cycle(Cycle),
    Screen {
        button: &'static str,
        content: Option<PanelId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Plate {
    pub name: String,
    pub level: u32,
    pub hp: (u32, u32),
    pub mp: (u32, u32),
    pub symbol: i32,
    pub hp_skin: usize,
    pub mount: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct TargetPlate {
    pub actor: usize,
    pub name: String,
    pub hp: (u32, u32),
    pub hp_color: u32,
}

impl TargetPlate {
    pub fn select(actors: &[Actor], player: Option<usize>) -> Option<TargetPlate> {
        let player = player?;

        actors.get(player)?;

        let mut selected = None;

        for (index, actor) in actors.iter().enumerate().rev() {
            if index == player || !actor.target_hud.eligible {
                continue;
            }

            selected = Some(index);

            if actor.highlight {
                break;
            }
        }

        let actor_index = selected?;

        let actor = &actors[actor_index];

        let name = actor
            .name
            .split_once('^')
            .map_or(actor.name.as_str(), |(name, _)| name);

        Some(TargetPlate {
            actor: actor_index,
            name: name.to_string(),
            hp: nameplate::hp_pair(actor),
            hp_color: actor.target_hud.hp_color,
        })
    }
}

impl Plate {
    pub fn new(
        name: String,
        status: &Status,
        class: usize,
        evolution: i32,
        poison: Option<i32>,
        mount: Option<(i32, i32)>,
    ) -> Plate {
        Plate {
            name,
            level: status.shown_level(),
            hp: (status.hp, status.max_hp),
            mp: (status.mp, status.max_mp),
            symbol: (evolution + SYMBOL_TIERS * class as i32).clamp(0, SYMBOLS - 1),
            hp_skin: hp_skin(poison.unwrap_or(0)),
            mount,
        }
    }

    pub fn value_text(pair: (u32, u32)) -> String {
        format!("{} / {:>7}", pair.0, pair.1)
    }

    pub fn level_text(&self) -> String {
        format!("{LEVEL_FORMAT}{} ", self.level)
    }

    pub fn symbol_alias(&self) -> String {
        format!("UI_Hud.Symbol{}", self.symbol)
    }
}

pub fn hp_skin(seconds: i32) -> usize {
    match seconds {
        s if s >= 7 => 1,
        s if s >= 4 => 2,
        s if s >= 1 => 3,
        _ => 0,
    }
}

pub struct Hud {
    quick_owners: [Option<ui::element::Instance>; QUICK_CELLS],
    bar_refresh_owners: BTreeMap<String, ui::element::Instance>,
    buff_owners: [Option<ui::element::Instance>; BUFF_CELLS],
    target_owners: BTreeMap<String, ui::element::Instance>,
    command_toggles: BTreeMap<ui::element::Instance, bool>,
    plate_owners: [Option<ui::element::Instance>; 8],
    pub layout: Layout,
    pub bar: PlacedPanel,
    pub plate: PlacedPanel,
    pub target: PlacedPanel,
    pub shown: Option<Plate>,
    pub guimat_textures: BTreeMap<i32, GuiTexture>,
    pub buff_sheet: Option<formats::texture::Texture>,
    pub skill_sheet: Option<formats::texture::Texture>,
    pub page: i32,
    pub missing: Vec<i32>,
    pub captions: bool,
    pub notes: Vec<String>,
    pub manager: Manager,
    pub menu_shown: bool,
    pub cc_shown: bool,
    pub debounce: [u32; 4],
    pub strings: Strings,
    pub effect_strings: EffectStrings,
    pub affect_time: super::affect_time::Clock,
    pub affect_raw_ms: u32,
    pub affect_game_ms: u32,
    pub affect_stamps: [u32; 32],
    pub affect_live_times: [i32; 32],
    pub player_affect_tooltips: [Option<AffectTooltip>; BUFF_CELLS],
    pub target_affect_tooltips: [Option<AffectTooltip>; BUFF_CELLS],
    pub combat_shown: Option<Combat>,
    pub toasts: Vec<String>,
}

impl Hud {
    pub(super) fn relocalize(&mut self, game: &GameDir, strings: &Strings, effects: EffectStrings) {
        self.bar.ui.relocalize(game, strings);

        self.plate.ui.relocalize(game, strings);

        self.target.ui.relocalize(game, strings);

        self.strings = strings.clone();

        self.effect_strings = effects;
    }

    pub(super) fn input_pane(&self, index: usize) -> Option<&PlacedPanel> {
        match index {
            0 => Some(&self.bar),
            1 => Some(&self.plate),
            2 => Some(&self.target),
            _ => None,
        }
    }

    pub(super) fn input_pane_mut(&mut self, index: usize) -> Option<&mut PlacedPanel> {
        match index {
            0 => Some(&mut self.bar),
            1 => Some(&mut self.plate),
            2 => Some(&mut self.target),
            _ => None,
        }
    }

    pub(super) fn skill_receiver_mut(&mut self, slot: usize) -> Option<&mut ui::Element> {
        Self::bound_element(
            &mut self.bar,
            &self.bar_refresh_owners,
            &format!("Skill{slot}"),
            ElementType::IconButton,
        )
    }

    pub(super) fn recount_quick(
        &mut self,
        matching: u32,
        carry: &[formats::net::account::Item],
        items: &ItemList,
    ) -> ([u32; QUICK_CELLS], Option<usize>) {
        let mut ids = [0; QUICK_CELLS];

        let mut last = None;

        for (slot, owner) in self.quick_owners.iter().enumerate() {
            let Some(owner) = owner else {
                continue;
            };

            let Some(button) = self
                .bar
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == *owner)
                .and_then(|e| e.widget.button_mut())
            else {
                continue;
            };

            let Some(id) = button
                .tooltip_binding()
                .and_then(|b| u32::try_from(b.index).ok())
            else {
                continue;
            };

            if id > 0x38A3 {
                continue;
            }

            ids[slot] = id;

            if let Some(contents) = button.icon_contents.as_mut() {
                contents.count = quick_stack_total(id, carry, items);
            }

            if id == matching {
                last = Some(slot);
            }
        }

        (ids, last)
    }

    fn bound_element<'a>(
        panel: &'a mut PlacedPanel,
        owners: &BTreeMap<String, ui::element::Instance>,
        name: &str,
        kind: ElementType,
    ) -> Option<&'a mut ui::Element> {
        let owner = owners.get(name)?;

        panel
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == *owner && e.element_type == kind)
    }

    fn draw_order(&mut self) -> [(PanelId, &mut PlacedPanel); 3] {
        [
            (CONTENT_HUD, &mut self.bar),
            (CONTENT_MY_INFO, &mut self.plate),
            (CONTENT_YOU_INFO, &mut self.target),
        ]
    }

    fn element_mut<'a>(
        panel: &'a mut PlacedPanel,
        name: &str,
        element_type: ElementType,
    ) -> Option<&'a mut ui::Element> {
        panel.ui.panel.element_mut(name, element_type)
    }

    pub fn affect_tooltip_at(&self, target: ui::input::Target) -> Option<String> {
        let Owner::Element(owner) = target.owner else {
            return None;
        };

        let panel = match target.panel {
            1 => &self.plate,
            2 => &self.target,
            _ => return None,
        };

        if !panel.ui.panel.visible {
            return None;
        }

        let element = panel.ui.panel.elements.get(owner)?;

        if !element.visible || element.element_type != ElementType::IconButton {
            return None;
        }

        self.affect_tooltip_for(element)
    }

    pub fn affect_tooltip_for(&self, element: &ui::Element) -> Option<String> {
        if element.element_type != ElementType::IconButton {
            return None;
        }

        let button = element.widget.button()?;

        let contents = button.icon_contents?;

        if contents.kind != ui::icon::AFFECT_KIND || !(0..=54).contains(&contents.index) {
            return None;
        }

        let detail = button.affect_detail.unwrap_or(affect::Detail {
            kind: contents.index as u8,
            level: 0,
            value: 0,
            time_8s: 0,
        });

        let tip = AffectTooltip {
            kind: detail.kind,
            value: detail.value,
            time_8s: detail.time_8s,
            slot: button.icon_slot_index,
            seconds: contents.refine,
        };

        if tip.slot == -1 {
            return tip.name(&self.effect_strings);
        }

        let slot = usize::try_from(tip.slot).ok()?;

        let live = *self.affect_live_times.get(slot)?;

        let seconds = tip.time_8s.wrapping_mul(8).wrapping_sub(
            (self.affect_game_ms.wrapping_sub(self.affect_stamps[slot]) / 1000) as i32,
        );

        let mut name = tip.name(&self.effect_strings).unwrap_or_default();

        name.push_str(" : ");

        name.push_str(
            &self
                .affect_time
                .duration(tip.time_8s, live, seconds, self.affect_raw_ms)
                .text(&self.strings),
        );

        Some(name)
    }

    pub fn refresh_plate(&mut self, plate: &Plate) {
        let ui = &mut self.plate.ui;

        for (i, (name, kind)) in [
            ("Symbol", ElementType::Image),
            ("Level", ElementType::Static),
            ("Name", ElementType::Static),
            ("HP", ElementType::GaugeBar),
            ("MP", ElementType::GaugeBar),
            ("FP", ElementType::GaugeBar),
            ("HPValue", ElementType::Static),
            ("MPValue", ElementType::Static),
        ]
        .into_iter()
        .enumerate()
        {
            let owner = ui.panel.element(name, kind).map(|e| e.instance);

            if self.plate_owners[i] != owner {
                self.shown = None;

                self.plate_owners[i] = owner;
            }
        }

        if self.shown.as_ref() == Some(plate) {
            return;
        }

        let resource = self.plate.ui.resource.clone();

        let panel = &mut self.plate;

        if let Some(name) = Hud::element_mut(panel, "Name", ElementType::Static) {
            name.set_display(&plate.name);
        }

        if let Some(level) = Hud::element_mut(panel, "Level", ElementType::Static) {
            level.set_display(&plate.level_text());
        }

        if let Some(value) = Hud::element_mut(panel, "HPValue", ElementType::Static) {
            value.set_display(&Plate::value_text(plate.hp));
        }

        if let Some(value) = Hud::element_mut(panel, "MPValue", ElementType::Static) {
            value.set_display(&Plate::value_text(plate.mp));
        }

        for (name, (current, maximum)) in [("HP", plate.hp), ("MP", plate.mp)] {
            if let Some(gauge) = Hud::element_mut(panel, name, ElementType::GaugeBar)
                .and_then(|e| e.widget.gauge_mut())
            {
                gauge.set_range_and_pos(0, maximum as i32, current as i32);
            }
        }

        if let Some(gauge) =
            Hud::element_mut(panel, "FP", ElementType::GaugeBar).and_then(|e| e.widget.gauge_mut())
        {
            gauge.set_range_and_pos(0, 1, 0);

            if let Some((current, maximum)) = plate.mount {
                gauge.set_range_and_pos(0, maximum, current);
            }
        }

        if let Some(symbol) = Hud::element_mut(panel, "Symbol", ElementType::Image) {
            symbol.set_layer_texture_alias(0, &plate.symbol_alias(), &resource);
        }

        if self.shown.as_ref().map(|p| p.hp_skin) != Some(plate.hp_skin) {
            let skin = HP_BAR_SKINS[plate.hp_skin.min(HP_BAR_SKINS.len() - 1)];

            if let Some(bar) = Hud::element_mut(panel, "HP", ElementType::GaugeBar) {
                bar.set_layer_texture_alias(1, skin, &resource);
            }
        }

        self.shown = Some(plate.clone());
    }

    pub(super) fn prepare_plate_refresh(&mut self) {
        let ui = &mut self.plate.ui;

        for (name, kind) in [
            ("Symbol", ElementType::Image),
            ("Level", ElementType::Static),
            ("Name", ElementType::Static),
            ("HP", ElementType::GaugeBar),
            ("MP", ElementType::GaugeBar),
            ("FP", ElementType::GaugeBar),
            ("HPValue", ElementType::Static),
            ("MPValue", ElementType::Static),
        ] {
            if ui.panel.element(name, kind).is_none() {
                ui.panel.create_element(name, kind, &ui.resource);
            }
        }
    }

    pub(super) fn prepare_target_refresh(&mut self) {
        let ui = &mut self.plate.ui;

        for (name, kind) in [("Name", ElementType::Static), ("HP", ElementType::GaugeBar)] {
            if ui.panel.element(name, kind).is_none() {
                ui.panel.create_element(name, kind, &ui.resource);
            }
        }
    }

    pub fn refresh_target(
        &mut self,
        target: Option<&TargetPlate>,
        affects: &[affect::Affect; affect::AFFECTS],
    ) {
        let Some(target) = target else {
            self.target.ui.panel.visible = false;

            return;
        };

        self.target.ui.panel.visible = true;

        if let Some(name) = Self::bound_element(
            &mut self.target,
            &self.target_owners,
            "Name",
            ElementType::Static,
        ) {
            name.set_display(&target.name);
        }

        if let Some(hp) = Self::bound_element(
            &mut self.target,
            &self.target_owners,
            "HP",
            ElementType::GaugeBar,
        ) {
            hp.set_layer_texture_color(1, target.hp_color);

            if let Some(gauge) = hp.widget.gauge_mut() {
                gauge.set_range_and_pos(0, target.hp.1 as i32, target.hp.0 as i32);
            }
        }

        let mut filled = 0usize;

        for affect in affects.iter().filter(|affect| affect.is_set()) {
            let name = format!("Buf{filled}");

            let contents = ui::icon::set_affect_icon(affect.kind, 0, 0);

            if let Some(element) = Self::bound_element(
                &mut self.target,
                &self.target_owners,
                &name,
                ElementType::IconButton,
            ) {
                element.visible = true;

                if let Some(state) = element.set_icon_contents(contents) {
                    state.icon_slot_index = -1;
                }
            }

            self.target_affect_tooltips[filled] = contents.map(|_| AffectTooltip::target(affect));

            filled += 1;
        }

        for cell in filled..BUFF_CELLS {
            let name = format!("Buf{cell}");

            if let Some(element) = Self::bound_element(
                &mut self.target,
                &self.target_owners,
                &name,
                ElementType::IconButton,
            ) {
                element.visible = false;

                element.set_icon_contents(None);
            }

            self.target_affect_tooltips[cell] = None;
        }
    }

    pub fn refresh_experience(&mut self, exp: &Experience, hold: bool) {
        let panel = &mut self.bar;

        if let Some(element) =
            Self::bound_element(panel, &self.bar_refresh_owners, "Hold", ElementType::Image)
        {
            element.visible = hold;
        }

        for i in 0..EXP_LAMPS {
            if let Some(lamp) = Self::bound_element(
                panel,
                &self.bar_refresh_owners,
                &format!("ExpLamp{i}"),
                ElementType::Image,
            ) {
                lamp.visible = i < exp.lamps;
            }
        }

        for (i, &filled) in exp.segments.iter().enumerate() {
            if let Some(gauge) = Self::bound_element(
                panel,
                &self.bar_refresh_owners,
                &format!("Exp{i}"),
                ElementType::GaugeBar,
            )
            .and_then(|e| e.widget.gauge_mut())
            {
                gauge.set_range(0, exp.tenth as i32);

                gauge.set_pos(filled as i32);
            }
        }
    }

    pub fn refresh_quest_timer(&mut self, update: &super::quest_timer::Update) {
        let Some(label) = Self::bound_element(
            &mut self.bar,
            &self.bar_refresh_owners,
            "QuestRemainTime",
            ElementType::Static,
        ) else {
            return;
        };

        if let Some(visible) = update.visible {
            label.visible = visible;
        }

        if let Some(text) = &update.text {
            label.set_display(text);
        }
    }

    pub fn refresh_buffs(
        &mut self,
        details: &[affect::Detail; affect::AFFECTS],
        stamps: &[u32; affect::AFFECTS],
        now_ms: u32,
    ) {
        self.affect_game_ms = now_ms;

        self.affect_stamps = *stamps;

        self.affect_live_times = details.map(|detail| detail.time_8s);

        let mut filled = 0usize;

        for (slot, detail) in details.iter().enumerate() {
            if !detail.is_set() {
                continue;
            }

            let seconds = affect::seconds_left(detail, now_ms, stamps[slot]);

            let contents =
                ui::icon::set_affect_icon(detail.kind, slot as i32, seconds).map(|mut contents| {
                    contents.blink_off = affect::blinking_off(seconds, now_ms);

                    contents
                });

            if let Some(owner) = self.buff_owners[filled]
                && let Some(element) = self
                    .plate
                    .ui
                    .panel
                    .elements
                    .iter_mut()
                    .find(|e| e.instance == owner)
            {
                element.visible = true;

                if let Some(state) = element.set_icon_contents(contents) {
                    state.icon_slot_index = slot as i16;

                    state.affect_detail = contents.map(|_| *detail);

                    state.affect_countdown = contents.map(|_| {
                        self.affect_time
                            .duration(detail.time_8s, detail.time_8s, seconds, self.affect_raw_ms)
                            .compact_text()
                    });
                }
            }

            self.player_affect_tooltips[filled] =
                contents.map(|_| AffectTooltip::local(detail, slot, seconds));

            filled += 1;
        }

        for cell in filled..BUFF_CELLS {
            if let Some(owner) = self.buff_owners[cell]
                && let Some(element) = self
                    .plate
                    .ui
                    .panel
                    .elements
                    .iter_mut()
                    .find(|e| e.instance == owner)
            {
                element.visible = false;

                element.set_icon_contents(None);
            }

            self.player_affect_tooltips[cell] = None;
        }
    }

    pub fn refresh_quick(
        &mut self,
        ids: &[u32; QUICK_CELLS],
        carry: &[formats::net::account::Item],
        items: &ItemList,
        icons: &formats::data::item_icon::IconTable,
    ) {
        let resource = self.bar.ui.resource.clone();

        for (cell, &id) in ids.iter().enumerate() {
            let stub = formats::net::account::Item {
                index: id as u16,
                ..Default::default()
            };

            let contents = (1..=0x38A3)
                .contains(&id)
                .then(|| ui::icon::set_icon(&stub, items, icons, &resource))
                .flatten()
                .map(|mut c| {
                    c.count = quick_stack_total(id, carry, items);

                    c
                });

            if let Some(owner) = self.quick_owners[cell]
                && let Some(element) = self
                    .bar
                    .ui
                    .panel
                    .elements
                    .iter_mut()
                    .find(|e| e.instance == owner)
                && let Some(button) = element.set_icon_contents(contents)
                && contents.is_none()
                && (1..=0x38A3).contains(&id)
            {
                button.set_tooltip_binding(Some(ui::icon::TooltipBinding {
                    kind: 1,
                    index: id as i32,
                    item: stub,
                    help_id: 0,
                }));
            }
        }
    }

    pub fn refresh_skills(&mut self, quick: &[u8; SKILL_CELLS], items: &ItemList) {
        if self.page < 0 || self.page >= PAGES {
            self.page = 0;
        }

        let page = self.page;

        if let Some(text) = Self::bound_element(
            &mut self.bar,
            &self.bar_refresh_owners,
            SKILL_TAB_TEXT,
            ElementType::Static,
        ) {
            text.set_display(&skill_tab_text(page));
        }

        for (slot, &code) in quick.iter().enumerate() {
            let name = format!("Skill{slot}");

            let Some(element) = Self::bound_element(
                &mut self.bar,
                &self.bar_refresh_owners,
                &name,
                ElementType::IconButton,
            ) else {
                continue;
            };

            element.visible = (slot / PAGE_CELLS) as i32 == page;

            let Some(state) = element.widget.button_mut() else {
                continue;
            };

            if code == formats::net::enter::QUICK_EMPTY {
                element.set_icon_contents(None);

                continue;
            }

            let id = skill_book_id(code);

            if state
                .icon_contents
                .is_some_and(|contents| contents.index == id)
            {
                continue;
            }

            element.set_icon_contents(ui::icon::set_skill_icon(id, items));
        }
    }

    pub fn refresh_cooldowns(
        &mut self,
        quick: &[u8; SKILL_CELLS],
        skills: Option<&skill_data::SkillData>,
        stamps: &[u32; cc::SKILL_STAMPS],
        cooldown_refined: bool,
        cooldown_affected: bool,
        evolution: i32,
        level: u16,
        now_ms: u32,
    ) {
        for (slot, &code) in quick.iter().enumerate() {
            let row = sweep_row(code);

            let Some(stamp) = stamps.get(row).copied() else {
                continue;
            };

            let elapsed = now_ms.wrapping_sub(stamp);

            if elapsed == 0 || stamp == 0 {
                continue;
            }

            let cooldown_s = match skills.and_then(|table| table.get(row)) {
                Some(skill) => cast::cooldown_s(skill, cooldown_refined, cooldown_affected),
                None => cast::COOLDOWN_FLOOR_S,
            };

            let soul = evolution == cc::SOUL_DISCOUNT_TIER
                && row == usize::from(cast::SOUL_CODE)
                && level >= cc::SOUL_DISCOUNT_LEVEL;

            let cooldown_s = match soul {
                true => cc::COOLDOWN_0X66_EVOLVED_S as i32,
                false => cooldown_s,
            };

            let sweep = ui::icon::Sweep {
                elapsed_ms: elapsed,
                cooldown_s: u32::try_from(cooldown_s).unwrap_or(1),
            };

            let served = sweep.served();

            let sweep = (served > 0.0 && served < 1.0).then_some(sweep);

            let name = format!("Skill{slot}");

            if let Some(contents) = Self::bound_element(
                &mut self.bar,
                &self.bar_refresh_owners,
                &name,
                ElementType::IconButton,
            )
            .and_then(|element| element.widget.button_mut())
            .and_then(|state| state.icon_contents.as_mut())
            {
                contents.sweep = sweep;
            }
        }
    }

    pub fn flip_page(&mut self, quick: &[u8; SKILL_CELLS], items: &ItemList) -> i32 {
        self.page = i32::from(self.page == 0);

        self.refresh_skills(quick, items);

        self.page
    }

    pub fn select_skill_only(&mut self, slot: usize) {
        check_cells(self, "Skill", SKILL_CELLS, Some(slot));
    }

    pub fn refresh_flags(&mut self, pk: bool, armed: bool, auto_skills: usize) {
        let panel = &mut self.bar;

        if let Some(element) =
            Self::bound_element(panel, &self.bar_refresh_owners, "PK", ElementType::Image)
        {
            element.visible = pk;
        }

        for i in 0..AUTO_SKILLS {
            if let Some(marker) = Self::bound_element(
                panel,
                &self.bar_refresh_owners,
                &format!("AutoSkill{i}"),
                ElementType::Image,
            ) {
                marker.visible = armed && AUTO_SKILLS - 1 - i < auto_skills;
            }
        }
    }

    pub fn refresh_combat(&mut self, combat: &Combat) {
        if self.combat_shown == Some(*combat) {
            return;
        }

        let Hud {
            bar,
            strings,
            bar_refresh_owners,
            ..
        } = self;

        let resource = bar.ui.resource.clone();

        if let Some((alias, tooltip)) = combat.mode_skin()
            && let Some(button) =
                Self::bound_element(bar, bar_refresh_owners, CC_BUTTONS[0], ElementType::Button)
        {
            for layer in 0..SKIN_LAYERS {
                button.set_layer_texture_alias(layer, &alias, &resource);
            }

            button.set_tooltip(tooltip, strings);
        }

        for (name, percent) in [(CC_BUTTONS[1], combat.potion), (CC_BUTTONS[2], combat.feed)] {
            if let Some(button) =
                Self::bound_element(bar, bar_refresh_owners, name, ElementType::Button)
            {
                button.set_display(&Combat::threshold_text(percent));
            }
        }

        if let Some((alias, tooltip)) = combat.movement_skin()
            && let Some(button) =
                Self::bound_element(bar, bar_refresh_owners, CC_BUTTONS[3], ElementType::Button)
        {
            for layer in 0..SKIN_LAYERS {
                button.set_layer_texture_alias(layer, &alias, &resource);
            }

            button.set_tooltip(tooltip, strings);

            if tooltip.is_empty() {
                button.set_tooltip_text("");
            }
        }

        self.combat_shown = Some(*combat);
    }

    pub(super) fn owns_combat_tooltip(&self, instance: ui::element::Instance) -> bool {
        [CC_BUTTONS[0], CC_BUTTONS[3]]
            .into_iter()
            .any(|name| self.bar_refresh_owners.get(name) == Some(&instance))
    }

    pub(super) fn owns_offset_tooltip(&self, instance: ui::element::Instance) -> bool {
        [MENU_BUTTON, CC_MENU_BUTTON]
            .into_iter()
            .chain(CC_BUTTONS)
            .chain(MENU_ARMS.map(|(name, _)| name))
            .any(|name| self.bar_refresh_owners.get(name) == Some(&instance))
    }

    pub fn toggle_tag(&mut self, tag: &str, shown: bool) -> bool {
        let next = !shown;

        self.bar.ui.panel.set_visible_by_tagname(tag, next);

        next
    }

    pub fn panel_refs(&self) -> Vec<PanelRef<'_>> {
        [&self.bar, &self.plate, &self.target]
            .into_iter()
            .map(|p| PanelRef {
                panel: &p.ui.panel,
                origin: p.origin,
            })
            .collect()
    }

    pub fn mouse(&mut self, event: Mouse, x: i32, y: i32) -> (Vec<Event>, bool) {
        let consumed = self
            .panel_refs()
            .iter()
            .any(|p| p.panel.visible && p.contains(x, y));

        let mut manager = std::mem::take(&mut self.manager);

        let mut refs = [
            PanelMut {
                panel: &mut self.bar.ui.panel,
                origin: &mut self.bar.origin,
            },
            PanelMut {
                panel: &mut self.plate.ui.panel,
                origin: &mut self.plate.origin,
            },
            PanelMut {
                panel: &mut self.target.ui.panel,
                origin: &mut self.target.origin,
            },
        ];

        let events = manager.mouse_in_order(&mut refs, &HUD_INPUT_ORDER, event, x, y);

        self.manager = manager;

        (events, consumed)
    }

    fn accepts_cycle(&mut self, button: usize, now_ms: u32) -> bool {
        match self.debounce.get_mut(button) {
            Some(deadline) if now_ms >= *deadline => {
                *deadline = now_ms.wrapping_add(DEBOUNCE_MS);

                true
            }
            _ => false,
        }
    }

    pub fn toast_record(&mut self, record: usize, values: &[i32]) {
        self.toast(record, values);
    }

    fn toast(&mut self, record: usize, values: &[i32]) {
        let Some(strdef) = self.strings.strdef.as_ref() else {
            return;
        };

        let template = strdef.get(record);

        if template.is_empty() {
            return;
        }

        let mut text = String::new();

        let mut rest = template;

        for value in values {
            match rest.split_once("%d") {
                Some((before, after)) => {
                    text.push_str(before);

                    text.push_str(&value.to_string());

                    rest = after;
                }
                None => break,
            }
        }

        text.push_str(rest);

        self.toasts.push(text.replace("%%", "%"));
    }

    pub fn reposition(&mut self, backbuffer: (u32, u32)) {
        self.layout = Layout::new(backbuffer.0, backbuffer.1);

        let screen = self.layout.virtual_screen();

        let scale = self.layout.ui_scale();

        for (_, panel) in self.draw_order() {
            panel.ui.scale = scale;

            panel.origin = panel.ui.reposition(screen);
        }
    }
}

fn bind_named(
    panel: &mut ui::Panel,
    resource: &ui::Resource,
    names: Vec<(String, ElementType)>,
) -> BTreeMap<String, ui::element::Instance> {
    names
        .into_iter()
        .map(|(name, kind)| {
            if panel.element(&name, kind).is_none() {
                panel.create_element(&name, kind, resource);
            }

            let owner = panel
                .element(&name, kind)
                .expect("typed control was created")
                .instance;

            (name, owner)
        })
        .collect()
}

fn bind_buffs(
    panel: &mut ui::Panel,
    resource: &ui::Resource,
) -> [Option<ui::element::Instance>; BUFF_CELLS] {
    let owners = bind_named(
        panel,
        resource,
        (0..32)
            .map(|i| (format!("Buf{i}"), ElementType::IconButton))
            .collect(),
    );

    std::array::from_fn(|i| owners.get(&format!("Buf{i}")).copied())
}

fn bind_bar(
    panel: &mut ui::Panel,
    resource: &ui::Resource,
) -> BTreeMap<String, ui::element::Instance> {
    let mut names = vec![
        ("PK".to_owned(), ElementType::Image),
        (SKILL_TAB_TEXT.to_owned(), ElementType::Static),
        ("Hold".to_owned(), ElementType::Image),
    ];

    for (prefix, count, kind) in [
        ("ExpLamp", 3, ElementType::Image),
        ("Exp", 10, ElementType::GaugeBar),
        ("AutoSkill", 10, ElementType::Image),
        ("Skill", 20, ElementType::IconButton),
        ("Quick", 5, ElementType::IconButton),
    ] {
        names.extend((0..count).map(|i| (format!("{prefix}{i}"), kind)));
    }

    names.extend(
        CC_BUTTONS
            .into_iter()
            .chain([CC_MENU_BUTTON, MENU_BUTTON])
            .chain(MENU_ARMS.map(|(name, _)| name))
            .map(|name| (name.to_owned(), ElementType::Button)),
    );

    names.extend(HIDDEN_STATICS.map(|name| (name.to_owned(), ElementType::Static)));

    let owners = bind_named(panel, resource, names);

    for i in 0..10 {
        panel
            .element_mut(&format!("Exp{i}"), ElementType::GaugeBar)
            .unwrap()
            .widget
            .gauge_mut()
            .unwrap()
            .set_pos(0);

        panel
            .element_mut(&format!("AutoSkill{i}"), ElementType::Image)
            .unwrap()
            .visible = false;
    }

    for (prefix, count, group) in [
        ("Skill", 20, cell_group::HUD_SKILL),
        ("Quick", 5, cell_group::QUICK),
    ] {
        for i in 0..count {
            let e = panel
                .element_mut(&format!("{prefix}{i}"), ElementType::IconButton)
                .unwrap();

            let button = e.set_icon_contents(None).unwrap();

            button.cell_group = group;

            button.icon_slot_index = i as i16;

            button.toggle = true;
        }
    }

    panel.set_visible_by_tagname(CC_MODE_TAG, false);

    for name in HIDDEN_STATICS {
        panel
            .element_mut(name, ElementType::Static)
            .unwrap()
            .visible = false;
    }

    owners
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    strings: &Strings,
    chains: &Chains,
    captions: bool,
) -> Result<Hud> {
    let layout = Layout::new(backbuffer.0, backbuffer.1);

    let screen = layout.virtual_screen();

    let zoom = layout.ui_scale();

    let place = |script: &[u8]| -> Result<PlacedPanel> {
        let ui = PanelUi::build(game, script, strings, chains, zoom)
            .with_context(|| String::from_utf8_lossy(script).into_owned())?;

        let origin = ui.origin(screen);

        Ok(PlacedPanel { ui, origin })
    };

    let mut bar = place(PANEL_HUD)?;

    let mut plate = place(PANEL_MY_INFO)?;

    let mut target = place(PANEL_YOU_INFO)?;

    target.ui.panel.visible = false;

    let bar_refresh_owners = bind_bar(&mut bar.ui.panel, &bar.ui.resource);

    let buff_owners = bind_buffs(&mut plate.ui.panel, &plate.ui.resource);

    let mut target_names = vec![
        ("Name".to_owned(), ElementType::Static),
        ("HP".to_owned(), ElementType::GaugeBar),
    ];

    target_names.extend((0..32).map(|i| (format!("Buf{i}"), ElementType::IconButton)));

    let target_owners = bind_named(&mut target.ui.panel, &target.ui.resource, target_names);

    let quick_owners =
        std::array::from_fn(|i| bar_refresh_owners.get(&format!("Quick{i}")).copied());

    let mut hud = Hud {
        quick_owners,
        bar_refresh_owners,
        buff_owners,
        target_owners,
        command_toggles: BTreeMap::new(),
        plate_owners: [None; 8],
        layout: Layout::new(backbuffer.0, backbuffer.1),
        bar,
        plate,
        target,
        shown: None,
        guimat_textures: BTreeMap::new(),
        buff_sheet: None,
        skill_sheet: None,
        page: 0,
        missing: Vec::new(),
        captions,
        notes: Vec::new(),
        manager: Manager::default(),
        menu_shown: false,
        cc_shown: false,
        debounce: [0; 4],
        strings: strings.clone(),
        effect_strings: EffectStrings::load(game),
        affect_time: super::affect_time::Clock::default(),
        affect_raw_ms: 0,
        affect_game_ms: 0,
        affect_stamps: [0; 32],
        affect_live_times: [0; 32],
        player_affect_tooltips: [None; BUFF_CELLS],
        target_affect_tooltips: [None; BUFF_CELLS],
        combat_shown: None,
        toasts: Vec::new(),
    };

    let interaction = Interaction::default();

    let hidden = hud.target.ui.panel.visible;

    hud.target.ui.panel.visible = true;

    for tag in [MENU_TAG, CC_MODE_TAG] {
        hud.bar.ui.panel.set_visible_by_tagname(tag, true);
    }

    let mut referenced: Vec<(usize, i32)> = Vec::new();

    for (index, panel) in [&hud.bar, &hud.plate, &hud.target].into_iter().enumerate() {
        for slot in panel.ui.referenced_textures(&interaction) {
            referenced.push((index, slot));
        }
    }

    let symbols = (0..SYMBOLS).map(|tier| format!("UI_Hud.Symbol{tier}"));

    let flashes = HP_BAR_SKINS.iter().map(|skin| (*skin).to_string());

    for name in symbols.chain(flashes) {
        if let Some(alias) = hud.plate.ui.resource.alias(&name) {
            referenced.push((1, alias.texture_index));
        }
    }

    let modes = (0..CC_MODES).map(|mode| format!("UI_Hud.CCMode{mode}"));

    let movements = (0..MOVE_MODES).map(|mode| format!("UI_Hud.CCModeMove{mode}"));

    for name in modes.chain(movements) {
        if let Some(alias) = hud.bar.ui.resource.alias(&name) {
            referenced.push((0, alias.texture_index));
        }
    }

    for tag in [MENU_TAG, CC_MODE_TAG] {
        hud.bar.ui.panel.set_visible_by_tagname(tag, false);
    }

    hud.target.ui.panel.visible = hidden;

    let load = |path: &str| -> Result<Option<formats::texture::Texture>> {
        match game.read(path.as_bytes()) {
            Ok(bytes) => Ok(Some(
                formats::texture::decode(&bytes).with_context(|| format!("decoding {path}"))?,
            )),
            Err(_) => Ok(None),
        }
    };

    hud.buff_sheet = load(formats::net::affect::ICON_SHEET_ON)?;

    hud.skill_sheet = load(formats::net::affect::ICON_SHEET)?;

    for (panel, slot) in referenced {
        if hud.guimat_textures.contains_key(&slot) || hud.missing.contains(&slot) {
            continue;
        }

        let ui = &[&hud.bar, &hud.plate, &hud.target][panel].ui;

        match crate::scene::login_panels::load_guimat_texture(game, ui, slot)? {
            Some(loaded) => {
                hud.guimat_textures.insert(slot, loaded);
            }
            None => hud.missing.push(slot),
        }
    }

    hud.notes = [&hud.bar, &hud.plate, &hud.target]
        .into_iter()
        .flat_map(|p| p.ui.notes.iter().cloned())
        .collect();

    Ok(hud)
}

pub fn handle(
    hud: &mut Hud,
    events: &[Event],
    now_ms: u32,
    lock: &InputLock,
    combat: &mut Combat,
) -> Vec<Command> {
    handle_in_field(hud, events, now_ms, lock, combat, true)
}

pub fn handle_in_field(
    hud: &mut Hud,
    events: &[Event],
    now_ms: u32,
    lock: &InputLock,
    combat: &mut Combat,
    player_present: bool,
) -> Vec<Command> {
    let mut out = Vec::new();

    for event in events {
        if event.id.command_id() != EventId::Click {
            continue;
        }

        let Some(owner) = event.owner else {
            continue;
        };

        let Some(panel) = hud.input_pane(event.panel) else {
            continue;
        };

        let Some(receiver) = ui::input::retained_element(&panel.ui.panel, owner) else {
            continue;
        };

        let instance = receiver.instance;

        if receiver.element_type == ElementType::IconButton {
            if let Some(group) = receiver.widget.button().map(|state| state.cell_group) {
                match group {
                    cell_group::HUD_SKILL => {
                        let slot = hud_cell(hud, instance, "Skill", SKILL_CELLS);

                        check_cells(hud, "Skill", SKILL_CELLS, slot);

                        check_cells(hud, "Quick", QUICK_CELLS, None);

                        out.push(slot.map_or(Command::UnboundSkill, Command::SelectSkill));
                    }
                    cell_group::QUICK => {
                        let slot = hud_cell(hud, instance, "Quick", QUICK_CELLS);

                        check_cells(hud, "Quick", QUICK_CELLS, slot);

                        check_cells(hud, "Skill", SKILL_CELLS, None);

                        out.push(slot.map_or(Command::UnboundQuick, Command::SelectQuick));
                    }
                    _ => {}
                }
            }

            continue;
        }

        if !player_present || !lock.allows(now_ms) {
            continue;
        }

        let name = event.name.as_str();

        if name == MENU_BUTTON {
            let shown = hud.command_toggles.get(&instance).copied().unwrap_or(false);

            hud.menu_shown = hud.toggle_tag(MENU_TAG, shown);

            hud.command_toggles.insert(instance, hud.menu_shown);

            out.push(Command::Menu(hud.menu_shown));

            continue;
        }

        if name == CC_MENU_BUTTON {
            let shown = hud.command_toggles.get(&instance).copied().unwrap_or(false);

            hud.cc_shown = hud.toggle_tag(CC_MODE_TAG, shown);

            hud.command_toggles.insert(instance, hud.cc_shown);

            if hud.cc_shown {
                hud.toast(TOAST_CC_MENU, &[combat.potion, combat.feed]);
            }

            out.push(Command::CcMenu(hud.cc_shown));

            continue;
        }

        if let Some(button) = CC_BUTTONS.iter().position(|&b| b == name) {
            if !hud.accepts_cycle(button, now_ms) {
                continue;
            }

            let cycle = match button {
                0 => Cycle::Mode(combat.cycle_mode()),
                1 => {
                    let percent = combat.cycle_potion();

                    let record = match percent {
                        0 => TOAST_POTION_OFF,
                        _ => TOAST_POTION_ON,
                    };

                    hud.toast(record, &[percent]);

                    Cycle::Potion(percent)
                }
                2 => {
                    let percent = combat.cycle_feed();

                    let record = match percent {
                        0 => TOAST_FEED_OFF,
                        _ => TOAST_FEED_ON,
                    };

                    hud.toast(record, &[percent]);

                    Cycle::Feed(percent)
                }
                _ => Cycle::Movement(combat.cycle_movement()),
            };

            hud.refresh_combat(combat);

            out.push(Command::Cycle(cycle));

            continue;
        }

        if let Some(&(button, content)) = MENU_ARMS.iter().find(|(b, _)| *b == name) {
            out.push(Command::Screen { button, content });
        }
    }

    out
}

fn hud_cell(
    hud: &Hud,
    instance: ui::element::Instance,
    prefix: &str,
    count: usize,
) -> Option<usize> {
    (0..count)
        .find(|slot| hud.bar_refresh_owners.get(&format!("{prefix}{slot}")) == Some(&instance))
}

fn check_cells(hud: &mut Hud, prefix: &str, count: usize, selected: Option<usize>) {
    for slot in 0..count {
        if let Some(state) = Hud::bound_element(
            &mut hud.bar,
            &hud.bar_refresh_owners,
            &format!("{prefix}{slot}"),
            ElementType::IconButton,
        )
        .and_then(|element| element.widget.button_mut())
        {
            state.checked = selected == Some(slot);
        }
    }
}

pub fn upload(hud: &Hud, gpu: &Gpu, renderer: &mut SceneRenderer) {
    for (slot, loaded) in &hud.guimat_textures {
        renderer.upload_once(
            gpu,
            TextureKey::GuiMat(*slot),
            &loaded.texture,
            AlphaPolicy::Stored,
            &format!("{} (guimat slot {slot})", loaded.path),
        );
    }

    let pair = [
        (
            AmulSheet::On,
            &hud.buff_sheet,
            formats::net::affect::ICON_SHEET_ON,
        ),
        (
            AmulSheet::Plain,
            &hud.skill_sheet,
            formats::net::affect::ICON_SHEET,
        ),
    ];

    for (key, sheet, path) in pair {
        if let Some(sheet) = sheet {
            renderer.upload_once(gpu, TextureKey::Amul(key), sheet, AlphaPolicy::Stored, path);
        }
    }
}

pub fn sync(hud: &mut Hud, gpu: &Gpu, renderer: &mut SceneRenderer) {
    let captions = hud.captions;

    for (id, panel) in hud.draw_order() {
        if panel.ui.panel.visible {
            renderer.sync_named_icons(gpu, id, &panel.ui);

            if captions {
                renderer.sync_captions(gpu, id, &panel.ui);
            }
        }
    }
}

pub fn frame_draws(hud: &mut Hud) -> Vec<PlannedQuad> {
    for panel in [&mut hud.bar, &mut hud.plate, &mut hud.target] {
        panel
            .ui
            .panel
            .set_combo_screen(hud.layout.virtual_screen(), panel.origin);
    }

    let captions = hud.captions;

    let mut out = Vec::new();

    let interactions: Vec<Interaction> = [&hud.bar, &hud.plate, &hud.target]
        .into_iter()
        .enumerate()
        .map(|(i, p)| hud.manager.interaction_for(i, &p.ui.panel, p.origin))
        .collect();

    for (index, (id, panel)) in hud.draw_order().into_iter().enumerate() {
        if !panel.ui.panel.visible {
            continue;
        }

        let interaction = &interactions[index];

        for draw in panel.ui.frame(interaction, panel.origin) {
            if !captions && matches!(draw.source, Source::Caption(..)) {
                continue;
            }

            let texture = match draw.source {
                Source::Texture(slot) => TextureKey::GuiMat(slot),
                Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                Source::Amul(sheet) => TextureKey::Amul(sheet),
                Source::Caption(owner, run) => TextureKey::Caption(id, owner, run),
                Source::NamedIcon(owner) => TextureKey::NamedIcon(id, owner),
                Source::Solid => TextureKey::White,
            };

            out.push(PlannedQuad {
                mode: draw.mode(),
                texture,
                quad: draw.quad,
                uv: draw.uv,
            });
        }
    }

    out
}

fn quick_stack_total(id: u32, carry: &[formats::net::account::Item], items: &ItemList) -> i32 {
    carry
        .iter()
        .take(formats::net::enter::CARRY_SHOWN)
        .filter(|item| i32::from(item.index as i16) == id as i32)
        .map(|item| match ui::icon::stack_count(item, items) {
            0 => 1,
            n => n,
        })
        .sum()
}
