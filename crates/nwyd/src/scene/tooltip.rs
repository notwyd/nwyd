use crate::fonts::Chains;
use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::enter::Entered;
use crate::scene::item_tip::{self, Context as TipContext, Tables};
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::event_coupon::{self, Coupon};
use formats::data::item_help::Help;
use formats::data::item_list::ItemList;
use formats::data::mount::{self, MountData};
use formats::data::skill_data::{self, SkillData};
use formats::ui::pane::{self, Pane};
use ui::draw::Interaction;
use ui::input::Target;
use ui::{Panel, Resource, Strings, Tooltip};

pub const WINDOW_SCRIPT: &[u8] = b"UI\\Panel\\Tooltips\\ToolTip.pane";

pub const TEMPLATE_SCRIPT: &[u8] = b"UI\\Panel\\Tooltips\\ToolTip_Item.pane";

pub const MOUNT_DATA: &[u8] = b"MountData.bin";

pub const SKILL_DATA: &[u8] = b"SkillData.bin";

pub const EVENT_COUPON: &[u8] = b"EventCoupon.bin";

pub const TOOLTIP_PANEL_ID: PanelId = 0xF0;

pub struct Tip {
    window: Pane,
    template: Pane,
    ui: PanelUi,
    mounts: Vec<MountData>,
    help: Vec<Help>,
    skills: Option<SkillData>,
    coupons: Vec<Coupon>,
    pub shown: Option<Shown>,
    pub raised_by: Option<Target>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]

pub struct Shown {
    pub panel: Panel,
    pub origin: (i32, i32),
    pub z_order: i32,
    pub item: i32,
    pub source_content: Option<i32>,
}

pub fn build(game: &GameDir, strings: &Strings, chains: &Chains, scale: f32) -> Result<Tip> {
    let ui = PanelUi::build(game, WINDOW_SCRIPT, strings, chains, scale)
        .context("building ui/Panel/Tooltips/ToolTip.pane")?;

    let window = parse(game, WINDOW_SCRIPT)?;

    let template = parse(game, TEMPLATE_SCRIPT)?;

    let mut notes = Vec::new();

    let mounts = match game.read(MOUNT_DATA) {
        Ok(raw) => mount::parse_data(&raw).unwrap_or_else(|e| {
            notes.push(format!("MountData.bin: {e}"));

            Vec::new()
        }),
        Err(e) => {
            notes.push(format!("MountData.bin: {e}"));

            Vec::new()
        }
    };

    let help = match crate::ui::load_item_help(game) {
        Ok(help) => help,
        Err(e) => {
            notes.push(format!("itemhelp.dat: {e}"));

            Vec::new()
        }
    };

    let skills = match game.read(SKILL_DATA) {
        Ok(raw) => skill_data::parse(&raw)
            .map_err(|e| notes.push(format!("SkillData.bin: {e}")))
            .ok(),
        Err(e) => {
            notes.push(format!("SkillData.bin: {e}"));

            None
        }
    };

    let coupons = match game.read(EVENT_COUPON) {
        Ok(raw) => event_coupon::parse(&raw),
        Err(e) => {
            notes.push(format!("EventCoupon.bin: {e}"));

            event_coupon::parse(&[])
        }
    };

    Ok(Tip {
        window,
        template,
        ui,
        mounts,
        help,
        skills,
        coupons,
        shown: None,
        raised_by: None,
        notes,
    })
}

fn parse(game: &GameDir, script: &[u8]) -> Result<Pane> {
    let name = String::from_utf8_lossy(script).into_owned();

    let raw = game.read(script).with_context(|| name.clone())?;

    pane::load(&name, &raw).with_context(|| format!("parsing {name}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Cell {
    pub index: i32,
    pub item: formats::net::account::Item,
    pub rect: [i32; 4],
    pub owner_origin: (i32, i32),
    pub owner_z: i32,
    pub owner_content: Option<i32>,
    pub asking_price: i32,
}

impl Tip {
    pub(super) fn relocalize(&mut self, game: &GameDir, strings: &Strings, help: Vec<Help>) {
        self.ui.relocalize(game, strings);

        self.help = help;

        self.hide();
    }

    #[cfg(test)]

    pub(crate) fn for_test() -> Self {
        Self {
            window: Pane::default(),
            template: Pane::default(),
            ui: PanelUi::from_resource(
                Panel::default(),
                Resource::default(),
                &Chains::default(),
                1.0,
            ),
            mounts: Vec::new(),
            help: Vec::new(),
            skills: None,
            coupons: Vec::new(),
            shown: None,
            raised_by: None,
            notes: Vec::new(),
        }
    }

    pub(super) fn needs_zone_clock(&self, id: i32, item_id: u16) -> bool {
        (1..14500).contains(&id)
            && !matches!(id, 0x15E3 | 0xD73 | 0xD74)
            && !formats::data::skill_data::is_skill_book(id)
            && item_tip::ZONE_ENTRY_IDS.contains(&i32::from(item_id))
            && self
                .help
                .get(id as usize)
                .is_none_or(|help| help.count == 0)
    }

    pub fn hide(&mut self) {
        self.shown = None;

        self.raised_by = None;
    }

    pub fn visible(&self) -> bool {
        self.shown.is_some()
    }

    pub fn update(
        &mut self,
        hovered: Option<(Target, Cell)>,
        tables: &Hosts<'_>,
        screen: (i32, i32),
        shift: bool,
    ) {
        if shift && self.shown.is_some() {
            self.raised_by = None;

            return;
        }

        let Some((target, cell)) = hovered else {
            self.hide();

            return;
        };

        self.raised_by = Some(target);

        self.shown = Some(self.render(&cell, tables, screen));
    }

    fn render(&mut self, cell: &Cell, tables: &Hosts<'_>, screen: (i32, i32)) -> Shown {
        let mut content = item_tip::build_for(
            cell.index,
            &cell.item,
            &tables.tables(
                &self.mounts,
                &self.help,
                self.skills.as_ref(),
                &self.coupons,
            ),
            &tables.wearer(),
            &tables.context(cell),
        );

        if let Some((config, calendar, portuguese)) = tables.expiry
            && let Some(row) = item_tip::configured_expiry(
                &cell.item,
                config,
                tables.strings,
                calendar,
                portuguese,
            )
        {
            content.rows.push(row);
        }

        self.render_content(cell, tables.strings, screen, &content)
    }

    pub fn update_item_link(
        &mut self,
        hovered: (Target, Cell, i32),
        tables: &Hosts<'_>,
        help: &super::mix::HelpTables,
        screen: (i32, i32),
        shift: bool,
    ) {
        if shift && self.shown.is_some() {
            self.raised_by = None;

            return;
        }

        let (target, cell, id) = hovered;

        let mut shown = if id < 14500 {
            self.render(&cell, tables, screen)
        } else {
            self.render_content(&cell, tables.strings, screen, &help.tooltip(id as usize))
        };

        shown.item = id;

        let width = shown.panel.right - shown.panel.left;

        shown.origin.0 = (cell.owner_origin.0 + cell.rect[2] - width).max(0);

        self.raised_by = Some(target);

        self.shown = Some(shown);
    }

    pub fn update_pet_card(
        &mut self,
        hovered: (Target, Cell),
        tables: &Hosts<'_>,
        screen: (i32, i32),
        shift: bool,
    ) {
        if shift && self.shown.is_some() {
            self.raised_by = None;

            return;
        }

        let (target, cell) = hovered;

        let content = item_tip::pet_card(
            &cell.item,
            &tables.tables(
                &self.mounts,
                &self.help,
                self.skills.as_ref(),
                &self.coupons,
            ),
        );

        self.shown = Some(self.render_content(&cell, tables.strings, screen, &content));

        self.raised_by = Some(target);
    }

    pub fn update_mission(
        &mut self,
        hovered: (Target, Cell, i32),
        missions: &super::mission::Tables,
        strings: &Strings,
        screen: (i32, i32),
        shift: bool,
    ) {
        if shift && self.shown.is_some() {
            self.raised_by = None;

            return;
        }

        let (target, cell, id) = hovered;

        let content = missions.tooltip(id);

        let mut shown = self.render_content(&cell, strings, screen, &content);

        shown.item = id;

        self.raised_by = Some(target);

        self.shown = Some(shown);
    }

    pub fn update_mix_help(
        &mut self,
        hovered: (Target, Cell, usize),
        help: &super::mix::HelpTables,
        strings: &Strings,
        screen: (i32, i32),
        shift: bool,
    ) {
        if shift && self.shown.is_some() {
            self.raised_by = None;

            return;
        }

        let (target, cell, id) = hovered;

        let mut shown = self.render_content(&cell, strings, screen, &help.tooltip(id));

        shown.item = id as i32;

        self.raised_by = Some(target);

        self.shown = Some(shown);
    }

    fn render_content(
        &self,
        cell: &Cell,
        strings: &Strings,
        screen: (i32, i32),
        content: &item_tip::Content,
    ) -> Shown {
        let mut tip = Tooltip::new(&self.window, &self.template, &self.ui.resource, strings);

        tip.title(&content.title);

        for row in &content.rows {
            tip.push(row, &self.ui.resource);
        }

        if content.price_gap {
            tip.gap(PRICE_GAP);
        }

        if let Some(price) = &content.price {
            for line in price.text.lines() {
                let mut row = price.clone();

                row.text = line.to_owned();

                tip.push(&row, &self.ui.resource);
            }
        }

        let mut panel = tip.finish();

        panel.visible = true;

        panel.background.color = ui::tooltip::PANEL_COLOR;

        let size = (panel.width(), panel.height());

        let _parked = ui::tooltip::park(size, screen);

        let origin = ui::tooltip::anchor(size, cell.rect, cell.owner_origin, screen);

        Shown {
            panel,
            origin,
            z_order: ui::tooltip::z_order(cell.owner_z),
            item: cell.index,
            source_content: cell.owner_content,
        }
    }

    pub fn frame_draws(&mut self, captions: bool) -> Vec<PlannedQuad> {
        let Some(shown) = &self.shown else {
            return Vec::new();
        };

        self.ui.panel = shown.panel.clone();

        let interaction = Interaction::default();

        let mut out = Vec::new();

        for draw in self.ui.frame(&interaction, shown.origin) {
            if !captions && matches!(draw.source, Source::Caption(..)) {
                continue;
            }

            let texture = match draw.source {
                Source::Texture(slot) => TextureKey::GuiMat(slot),
                Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                Source::Amul(sheet) => TextureKey::Amul(sheet),
                Source::Caption(owner, run) => TextureKey::Caption(TOOLTIP_PANEL_ID, owner, run),
                Source::NamedIcon(owner) => TextureKey::NamedIcon(TOOLTIP_PANEL_ID, owner),
                Source::Solid => TextureKey::White,
            };

            out.push(PlannedQuad {
                mode: draw.mode(),
                texture,
                quad: draw.quad,
                uv: draw.uv,
            });
        }

        out
    }

    pub fn sync(&mut self, gpu: &render::Gpu, renderer: &mut SceneRenderer, captions: bool) {
        if self.shown.is_some() {
            renderer.sync_named_icons(gpu, TOOLTIP_PANEL_ID, &self.ui);
        }

        if captions && self.shown.is_some() {
            renderer.sync_captions(gpu, TOOLTIP_PANEL_ID, &self.ui);
        }
    }

    pub fn resource(&self) -> &Resource {
        &self.ui.resource
    }

    pub fn referenced_textures(&mut self, strings: &Strings) -> Vec<i32> {
        let mut tip = Tooltip::new(&self.window, &self.template, &self.ui.resource, strings);

        tip.title(&ui::tooltip::Row::new("", 0));

        self.ui.panel = tip.finish();

        self.ui.panel.visible = true;

        self.ui.referenced_textures(&Interaction::default())
    }

    pub fn ui(&self) -> &PanelUi {
        &self.ui
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut PanelUi)) {
        visit(&mut self.ui);
    }
}

pub const PRICE_GAP: i32 = 13;

pub struct Hosts<'a> {
    pub expiry: Option<(
        &'a formats::net::client_config::Config,
        crate::clock::Calendar,
        bool,
    )>,
    pub items: &'a ItemList,
    pub strings: &'a Strings,
    pub entered: &'a Entered,
    pub shop: item_tip::Shop,
    pub zone_clock: Option<i64>,
    pub sealed: Option<item_tip::Sealed>,
}

impl<'a> Hosts<'a> {
    fn tables(
        &self,
        mounts: &'a [MountData],
        help: &'a [Help],
        skills: Option<&'a SkillData>,
        coupons: &'a [Coupon],
    ) -> Tables<'a> {
        Tables {
            items: self.items,
            mounts,
            help,
            strings: self.strings,
            skills,
            coupons,
        }
    }

    fn context(&self, cell: &Cell) -> TipContext {
        let shop = item_tip::Shop {
            owner: cell.owner_content,
            asking: i64::from(cell.asking_price),
            ..self.shop
        };

        TipContext {
            price: item_tip::Price::choose(&shop),
            currency: shop.currency,
            tax_percent: shop.tax_percent,
            buy_back: shop.buy_back,
            owner_content: cell.owner_content,
            learner: learner(self.entered),
            zone_clock: self.zone_clock,
            sealed: self.sealed,
            ..TipContext::default()
        }
    }

    fn wearer(&self) -> formats::data::item_effect::Wearer {
        let status = &self.entered.status;

        formats::data::item_effect::Wearer {
            body: i32::from(self.entered.equipment.body().index),
            level: i32::from(status.level),
            strength: i32::from(status.stat[0]),
            intelligence: i32::from(status.stat[1]),
            dexterity: i32::from(status.stat[2]),
            condition: i32::from(status.stat[3]),
            learned_skills: self.entered.skill_mask[0],
        }
    }
}

pub fn learner(entered: &Entered) -> item_tip::Learner {
    item_tip::Learner {
        class: i32::from(entered.class),
        level: i32::from(entered.status.level),
        mastery: entered.status.mastery.map(i32::from),
        mana_saving: i32::from(entered.derived.mana_saving),
        skill_points: i32::from(entered.derived.skill_points),
        live_damage: None,
    }
}
