use crate::fonts;
use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::actors::{self, Doll};
use crate::scene::connect;
use crate::scene::create;
use crate::scene::delete;
use crate::scene::keypad;
use crate::scene::lock;
use crate::scene::login_panels::{self, GuiTexture, PlacedPanel};
use crate::scene::login_static::{self, Layout, SceneTime};
use crate::scene::msgbox;
use crate::scene::pick::{self, Ray};
use crate::scene::rng::Lcg;
use crate::scene::sel_info;
use crate::scene::world::{self, World, WorldFrame, WorldSpec};
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_list::ItemList;
use formats::mesh::appearance::{self, Look};
use formats::net::account::{self, Block};
use formats::scene::cam_action::{CamAction, Keyframe};
use formats::text::Codepage;
use formats::ui::pane::ElementType;
use render::device::Gpu;
use render::texture::AlphaPolicy;
use std::collections::BTreeMap;
use ui::Strings;
use ui::draw::Interaction;
use ui::input::{Event, Manager, Mouse, PanelMut, PanelRef};

pub const CAMERA_SELECT: &str = world::SELECT_CHAR_CAMERA;

pub const CAMERA_DESELECT: &str = r"UI\SelCamAction3.bin";

pub const CAMERA_CREATE: &str = r"UI\CreCamAction.bin";

#[cfg(test)]
mod language_tests;

#[derive(Debug, Clone, PartialEq)]

pub struct Cameras {
    pub select: CamAction,
    pub deselect: CamAction,
    pub create: CamAction,
}

impl Cameras {
    pub fn load(game: &GameDir) -> Result<Cameras> {
        Ok(Cameras {
            select: world::load_camera(game, CAMERA_SELECT)?,
            deselect: world::load_camera(game, CAMERA_DESELECT)?,
            create: world::load_camera(game, CAMERA_CREATE)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum Mode {
    #[default]
    Select,
    Create,
}

impl Mode {
    pub fn camera(self) -> &'static str {
        match self {
            Mode::Select => CAMERA_SELECT,
            Mode::Create => CAMERA_CREATE,
        }
    }
}

pub const SELECT_POSE: glam::Vec3 = glam::Vec3::new(2044.0, 2.0, 2044.0);

pub const SELECT_DOLL_HEIGHT: f32 = 1.3;

pub const CREATE_POSE: glam::Vec3 = glam::Vec3::new(2072.0, 5.5, 2072.0);

pub const CREATE_DOLL_HEIGHT: f32 = 5.2;

pub const APPROACH_MS: u32 = 400;

pub const APPROACH_BACKOFF: f32 = 4.0;

pub const APPROACH_YAW: f32 = f32::from_bits(0x4233_126F);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Approach {
    Toward,
    Away,
}

pub fn approach(
    mode: Mode,
    doll: glam::Vec2,
    forward: glam::Vec3,
    right: glam::Vec2,
    direction: Approach,
) -> CamAction {
    let (fixed, height) = match mode {
        Mode::Select => (SELECT_POSE, SELECT_DOLL_HEIGHT),
        Mode::Create => (CREATE_POSE, CREATE_DOLL_HEIGHT),
    };

    let near = glam::Vec3::new(
        (doll.x - forward.x * APPROACH_BACKOFF) + right.x * 0.5,
        height,
        (doll.y - forward.z * APPROACH_BACKOFF) + right.y * 0.5,
    );

    let key = |time_ms: u32, eye: glam::Vec3| Keyframe {
        time_ms,
        relative: 0,
        x: eye.x,
        y: eye.y,
        z: eye.z,
        yaw: APPROACH_YAW,
        pitch: 0.0,
    };

    let (first, second) = match direction {
        Approach::Toward => (fixed, near),
        Approach::Away => (near, fixed),
    };

    CamAction {
        keyframes: vec![key(0, first), key(APPROACH_MS, second)],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Pane {
    pub script: &'static [u8],
    pub nui_id: u32,
    pub visible: bool,
    pub z_order: i32,
}

pub const PANE_MENU: usize = 0;

pub const PANE_SEL_INFO: usize = 1;

pub const PANE_CREATE: usize = 2;

pub const PANE_PASSWORD: usize = 3;

pub const PANES: [Pane; 4] = [
    Pane {
        script: br"ui/Panel/login/CharSelect_Menu.pane",
        nui_id: 9,
        visible: true,
        z_order: 900,
    },
    Pane {
        script: br"ui/Panel/login/CharSelect_SelInfo.pane",
        nui_id: 10,
        visible: false,
        z_order: 600,
    },
    Pane {
        script: br"ui/Panel/login/CharSelect_Create.pane",
        nui_id: 11,
        visible: false,
        z_order: 600,
    },
    Pane {
        script: br"ui/Panel/login/CharSelect_Password.pane",
        nui_id: 12,
        visible: false,
        z_order: 600,
    },
];

pub use super::lock::{
    AWAITING as LOCK_AWAITING, CHANGE_CONFIRM as LOCK_CHANGE_CONFIRM,
    CHANGE_NEW as LOCK_CHANGE_NEW, CHANGE_SENT as LOCK_CHANGE_SENT,
    CONFIRM_DELETE as LOCK_CONFIRM_DELETE, LOCKED as LOCK_LOCKED, UNLOCKED as LOCK_UNLOCKED,
};

#[derive(Debug)]

pub struct Panels {
    pub panes: [PlacedPanel; 4],
    delete_owner: ui::element::Instance,
    pub msgbox: msgbox::BoxPanel,
    pub guimat_textures: BTreeMap<i32, GuiTexture>,
    pub missing: Vec<i32>,
    pub notes: Vec<String>,
    pub manager: Manager,
    pub strings: Strings,
    pub clipboard: ui::text_entry::Local,
}

impl Panels {
    pub fn relocalize(&mut self, game: &GameDir, strings: &Strings) {
        for pane in &mut self.panes {
            pane.ui.relocalize(game, strings);
        }

        self.msgbox.relocalize(game, strings);

        self.strings = strings.clone();
    }

    fn show_delete(&mut self, visible: bool) {
        if let Some(e) = self.panes[PANE_MENU]
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == self.delete_owner)
        {
            e.visible = visible;
        }
    }

    fn mode_panes(&mut self, mode: Mode) {
        self.show_delete(false);

        match mode {
            Mode::Select => sel_info::bind(&mut self.panes[PANE_SEL_INFO].ui),
            Mode::Create => create::bind(&mut self.panes[PANE_CREATE].ui),
        }

        self.panes[PANE_SEL_INFO].ui.panel.visible = false;

        self.panes[PANE_CREATE].ui.panel.visible = false;
    }

    pub fn menu(&self) -> &PlacedPanel {
        &self.panes[PANE_MENU]
    }

    pub fn sel_info(&self) -> &PlacedPanel {
        &self.panes[PANE_SEL_INFO]
    }

    pub fn create(&self) -> &PlacedPanel {
        &self.panes[PANE_CREATE]
    }

    pub fn password(&self) -> &PlacedPanel {
        &self.panes[PANE_PASSWORD]
    }

    pub fn stacking(&self) -> [usize; 4] {
        let mut order = [PANE_MENU, PANE_SEL_INFO, PANE_CREATE, PANE_PASSWORD];

        order.sort_by_key(|&i| PANES[i].z_order);

        order
    }

    pub fn layers(&self) -> Vec<Option<usize>> {
        let mut out: Vec<Option<usize>> = Vec::with_capacity(5);

        if self.msgbox.placed.ui.panel.visible {
            debug_assert!(msgbox::Z_ORDER < PANES.iter().map(|p| p.z_order).min().unwrap_or(0));

            out.push(None);
        }

        out.extend(self.stacking().into_iter().map(Some));

        out
    }

    pub fn panel_refs(&self) -> Vec<PanelRef<'_>> {
        self.layers()
            .into_iter()
            .map(|layer| match layer {
                None => PanelRef {
                    panel: &self.msgbox.placed.ui.panel,
                    origin: self.msgbox.placed.origin,
                },
                Some(i) => PanelRef {
                    panel: &self.panes[i].ui.panel,
                    origin: self.panes[i].origin,
                },
            })
            .collect()
    }

    pub fn mouse(&mut self, event: Mouse, x: i32, y: i32) -> Vec<Event> {
        let order = self.layers();

        let mut manager = std::mem::take(&mut self.manager);

        let [a, b, c, d] = &mut self.panes;

        let mut by_index = [Some(a), Some(b), Some(c), Some(d)];

        let mut box_panel = Some(&mut self.msgbox.placed);

        let mut refs: Vec<PanelMut<'_>> = Vec::with_capacity(order.len());

        for layer in order {
            let pane = match layer {
                None => box_panel
                    .take()
                    .expect("the box appears at most once in the stacking"),
                Some(index) => by_index[index]
                    .take()
                    .expect("each pane appears once in the stacking"),
            };

            refs.push(PanelMut {
                panel: &mut pane.ui.panel,
                origin: &mut pane.origin,
            });
        }

        let events = manager.mouse(&mut refs, event, x, y);

        if let Some(target) = manager.capture {
            let layer = self.layers().get(target.panel).copied();

            let placed = match layer {
                Some(Some(i)) => Some(&mut self.panes[i]),
                Some(None) => Some(&mut self.msgbox.placed),
                None => None,
            };

            if let Some(placed) = placed {
                placed
                    .ui
                    .edit_pointer(target.owner, event, x - placed.origin.0);
            }
        }

        self.manager = manager;

        events
    }

    pub fn create_name(&self) -> String {
        self.panes[PANE_CREATE]
            .ui
            .panel
            .element(CREATE_INPUT, ElementType::ImeEditBox)
            .and_then(|element| element.widget.edit())
            .map(|edit| edit.text.clone())
            .unwrap_or_default()
    }

    pub fn focused_edit(&mut self) -> Option<&mut ui::widget::EditState> {
        if self.msgbox.placed.ui.panel.visible {
            return None;
        }

        let target = self.manager.focused?;

        let pane = self.layers().get(target.panel).copied().flatten()?;

        let panel = &mut self.panes[pane].ui.panel;

        self.manager.reconcile_panel(target.panel, panel);

        let target = self.manager.focused?;

        let (element, _) = ui::input::owner_element_mut(panel, target.owner)?;

        element.widget.edit_mut()
    }

    pub fn character(&mut self, ch: char) -> Vec<ui::text_entry::Raised> {
        if self.msgbox.placed.ui.panel.visible {
            return self
                .msgbox
                .character(ch, &mut self.clipboard)
                .unwrap_or_default();
        }

        let mut clipboard = std::mem::take(&mut self.clipboard);

        let raised = match self.focused_edit() {
            Some(edit) => ui::text_entry::character(edit, ch, &mut clipboard),
            None => Vec::new(),
        };

        self.clipboard = clipboard;

        raised
    }

    pub fn key_prefix(&mut self, vk: u32) {
        if self.msgbox.placed.ui.panel.visible {
            ui::key_prefix::visit(
                &mut [&mut self.msgbox.placed.ui.panel],
                &[0],
                self.manager.focused.filter(|t| t.panel == 0),
                vk,
            );

            return;
        }

        let layers = self.layers();

        let mut refs: Vec<_> = self
            .panes
            .iter_mut()
            .enumerate()
            .map(|(i, p)| {
                (
                    layers.iter().position(|p| *p == Some(i)).unwrap(),
                    &mut p.ui.panel,
                )
            })
            .collect();

        refs.sort_by_key(|(i, _)| *i);

        let mut refs: Vec<_> = refs.into_iter().map(|(_, p)| p).collect();

        let order: Vec<_> = (0..refs.len()).collect();

        ui::key_prefix::visit_manager(&mut refs, &order, &self.manager, vk);
    }

    pub fn key(
        &mut self,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
    ) -> Vec<ui::text_entry::Raised> {
        self.key_prefix(super::combo_box::edit_virtual_key(key));

        if self.msgbox.placed.ui.panel.visible {
            return self
                .msgbox
                .key(key, mods, &mut self.clipboard)
                .unwrap_or_default();
        }

        let mut clipboard = std::mem::take(&mut self.clipboard);

        let raised = match self.focused_edit() {
            Some(edit) => ui::text_entry::key(edit, key, mods, &mut clipboard),
            None => Vec::new(),
        };

        self.clipboard = clipboard;

        raised
    }

    pub fn reanchor(&mut self, backbuffer: (u32, u32)) {
        let screen =
            crate::scene::login_static::Layout::new(backbuffer.0, backbuffer.1).virtual_screen();

        for pane in &mut self.panes {
            pane.origin = pane.ui.origin(screen);
        }
    }
}

pub const ACCOUNT_ROW_ORIGIN: glam::Vec2 = glam::Vec2::new(2053.0, 2048.2);

pub const CLASS_ROW_ORIGIN: glam::Vec2 = glam::Vec2::new(2082.0, 2077.2);

pub const DOLL_STEP: f32 = 0.333;

pub const DOLL_STEP_SCALE: f32 = 4.800049;

pub const DOLL_YAW_ASKED: f32 = -45.0;

pub const DOLL_YAW: f32 = actors::ORIGIN_YAW;

pub fn doll_position(origin: glam::Vec2, slot: usize) -> glam::Vec3 {
    let step = slot as f32 * DOLL_STEP;

    let z = origin.y + step * DOLL_STEP_SCALE;

    glam::Vec3::new(
        origin.x - step * DOLL_STEP_SCALE,
        actors::scene_ground_height(world::SCENE_SELECT_CHAR, z, 0.0),
        z,
    )
}

pub const CLASS_PREVIEW: [[i32; appearance::CLASS_PREVIEW_COLUMNS]; 4] = [
    [6, 0, 1245, 0, 0, 3612, 9],
    [16, 0, 1380, 0, 0, 3740, 9],
    [26, 0, 1530, 0, 0, 3792, 9],
    [36, 0, 1680, 0, 0, 3632, 9],
];

pub fn dolls(account: Option<&Block>, items: &ItemList) -> Vec<Doll> {
    let mut out = Vec::with_capacity(8);

    let yaw = DOLL_YAW;

    for slot in 0..account::SLOTS {
        let Some(character) = account.map(|b| b.slots[slot]).filter(|s| !s.is_empty()) else {
            continue;
        };

        let equip = &character.equipment;

        let Some(look) = Look::for_character(equip, items) else {
            continue;
        };

        out.push(Doll {
            look,
            pet_item: equip.items[13].index,
            position: doll_position(ACCOUNT_ROW_ORIGIN, slot),
            yaw,
            scale: doll_scale(character.status.stat[CON_STAT]),
            hands: (
                i32::from(equip.items[7].index),
                i32::from(equip.items[6].index),
            ),
            record: slot,
            entity: 0,
            name: character.name(),
            status: character.status,
            name_level: 0,
            level: character.status.level,
            guild: 0,
            guild_name: String::new(),
            face: i32::from(equip.body().index),
            affects: formats::net::affect::NO_AFFECTS,
            pick_action: None,
            shop_title: String::new(),
            green_name: false,
        });
    }

    for (slot, row) in CLASS_PREVIEW.iter().enumerate() {
        let Some(look) = Look::for_class_preview(row, items) else {
            continue;
        };

        out.push(Doll {
            look,
            pet_item: 0,
            position: doll_position(CLASS_ROW_ORIGIN, slot),
            yaw,
            scale: 1.0,
            hands: (row[4], row[5]),
            record: account::SLOTS + slot,
            entity: 0,
            name: String::new(),
            status: account::Status::default(),
            name_level: 0,
            level: 0,
            guild: 0,
            guild_name: String::new(),
            face: row[0],
            affects: formats::net::affect::NO_AFFECTS,
            pick_action: Some(create::PICK_ACTION[slot]),
            shop_title: String::new(),
            green_name: false,
        });
    }

    out
}

pub const CON_STAT: usize = 3;

pub const CON_CLAMP: f32 = 500.0;

pub const CON_DIVISOR: f32 = 4000.0;

pub const SHRUNK_BODIES: [i16; 2] = [0x16B, 0x4B];

pub fn doll_scale(con: u16) -> f32 {
    doll_scale_for(con, 0)
}

pub fn doll_scale_for(con: u16, body: i16) -> f32 {
    let con = match SHRUNK_BODIES.contains(&body) {
        true => -1000.0,
        false => f32::from(con).min(CON_CLAMP),
    };

    (con / CON_DIVISOR + 1.0) * 0.9
}

pub const DELETE_CONFIRM_STRDEF: usize = 18;

pub const BACK_CONFIRM_STRDEF: usize = 11;

pub const NOTHING_SELECTED_STRDEF: usize = 14;

pub const MENU_CREATE: &str = "Create";

pub const MENU_DELETE: &str = "Delete";

pub const MENU_BACK: &str = "Back";

pub const SEL_INFO_CONNECT: &str = "Connect";

pub const SEL_INFO_BACK: &str = "Back";

pub const CREATE_INPUT: &str = "Input";

pub const CREATE_SUBMIT: &str = "Create";

pub const CREATE_BACK: &str = "Back";

pub const KEEPALIVE_OPCODE: u16 = 0x3A0;

pub const KEEPALIVE_LEN: usize = 0x0C;

pub const KEEPALIVE_MS: u32 = 300_000;

pub const REFRESH_SOUND: i64 = 0x39;

pub fn restore_selection_bodies(block: &mut Block, substitute: i16) {
    for (slot, character) in block.slots.iter_mut().enumerate() {
        let body = character.equipment.items[0].index;

        if !character.is_empty()
            && matches!(
                i32::from(body as i16) % appearance::ITEM_ID_MODULUS,
                22..=25 | 32
            )
        {
            character.equipment.items[0].index = substitute as u16;

            tracing::debug!(
                target : "nwyd::body_state", event = "selection_body_restored", slot,
                previous_body = body, account_body = substitute,
                "Retail selection initializer restored account body"
            );
        }
    }
}

#[derive(Debug)]

pub struct SelectChar {
    pub selection_body_write: Option<i16>,
    pub legacy_boxes: super::legacy_message_box::Dialogs,
    pub name_dialog: super::character_name::Dialog,
    pub notice: super::notice::Overlay,
    pub message_panel: super::message_panel::Overlay,
    pub layout: Layout,
    pub time: SceneTime,
    pub world: World,
    pub cameras: Cameras,
    pub mode: Mode,
    pub cursor: Option<(i32, i32)>,
    pub panels: Panels,
    pub flag_sets: formats::texture::texture_set_list::TextureSetList,
    pub flag_textures: formats::texture::texture_list::TextureList,
    pub lock: u8,
    pub launcher_login: bool,
    pub captions: bool,
    pub account: Option<Block>,
    pub reserved_names: create::Reserved,
    pub create_enabled: bool,
    pub connect_armed_ms: u32,
    pub create_armed_ms: u32,
    pub boxes: msgbox::Boxes,
    pub last_packet_ms: u32,
    pub selected: Option<usize>,
    pub info: Option<sel_info::SelInfo>,
    pub descriptions: [String; 4],
    pub gauge_height: create::GaugeHeight,
    pub create_pane: Option<create::Pane>,
    pub class_picked: Option<usize>,
    pub pad: keypad::Pad,
    pub last_send_ms: u32,
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    time: &SceneTime,
    captions: bool,
) -> Result<SelectChar> {
    build_at_lock(game, backbuffer, time, captions, LOCK_LOCKED, None)
}

pub fn build_at_lock(
    game: &GameDir,
    backbuffer: (u32, u32),
    time: &SceneTime,
    captions: bool,
    lock: u8,
    account: Option<&Block>,
) -> Result<SelectChar> {
    let mut world = world::build_spec(game, &WorldSpec::SELECT_CHAR, &time.calendar)
        .context("building the character-select world")?;

    let cameras = Cameras::load(game).context("loading the character-select camera actions")?;

    world.play_camera(cameras.select.clone(), 0);

    let items = super::item_table::load(game)?;

    world
        .actors
        .place_dolls(game, &dolls(account, &items))
        .context("placing the character-select dolls")?;

    world
        .objects
        .load_held(game, &mut world.actors)
        .context("loading the character-select dolls' weapons")?;

    let mut panels =
        build_panels(game, backbuffer, lock).context("building the character-select panes")?;

    let mut pad = keypad::Pad::default();

    pad.shuffle(&mut Lcg::new(time.now_ms));

    pad.label(&mut panels.panes[PANE_PASSWORD].ui);

    let descriptions = create::descriptions(game, Codepage::Windows1252)
        .context("reading the four class descriptions")?;

    let gauge_height = create::gauge_height(&panels.panes[PANE_CREATE].ui);

    let flag_sets =
        formats::texture::texture_set_list::parse(&game.read(login_static::TEXTURE_SET_LIST)?)?;

    let flag_textures =
        formats::texture::texture_list::parse(&game.read(login_static::TEXTURE_LIST)?)?;

    Ok(SelectChar {
        legacy_boxes: super::legacy_message_box::Dialogs::build(game, backbuffer.0)?,
        name_dialog: super::character_name::Dialog::build(game, backbuffer.0)?,
        notice: super::notice::Overlay::build(game, backbuffer, &panels.strings)?,
        message_panel: super::message_panel::Overlay::build(game, backbuffer.0)?,
        layout: Layout::new(backbuffer.0, backbuffer.1),
        time: *time,
        world,
        cameras,
        mode: Mode::Select,
        cursor: None,
        panels,
        flag_sets,
        flag_textures,
        lock,
        launcher_login: false,
        captions,
        account: account.copied(),
        selection_body_write: None,
        reserved_names: create::Reserved::load(game),
        create_enabled: account.is_none_or(|b| b.first_free().is_some()),
        selected: None,
        info: None,
        descriptions,
        gauge_height,
        create_pane: None,
        class_picked: None,
        pad,
        last_send_ms: 0,
        connect_armed_ms: 0,
        create_armed_ms: 0,
        boxes: msgbox::Boxes::new(),
        last_packet_ms: 0,
    })
}

fn build_panels(game: &GameDir, backbuffer: (u32, u32), lock: u8) -> Result<Panels> {
    let strings = login_panels::strings(game);

    let chains = fonts::load(game, fonts::system_font_dir().as_deref());

    let layout = crate::scene::login_static::Layout::new(backbuffer.0, backbuffer.1);

    let zoom = layout.ui_scale();

    let screen = layout.virtual_screen();

    let mut built: Vec<PlacedPanel> = Vec::with_capacity(PANES.len());

    for pane in PANES {
        let ui = PanelUi::build(game, pane.script, &strings, &chains, zoom)
            .with_context(|| String::from_utf8_lossy(pane.script).into_owned())?;

        debug_assert_eq!(
            ui.panel.z_order,
            pane.z_order,
            "{}",
            String::from_utf8_lossy(pane.script)
        );

        let origin = ui.origin(screen);

        built.push(PlacedPanel { ui, origin });
    }

    let mut panes: [PlacedPanel; 4] = built
        .try_into()
        .map_err(|_| anyhow::anyhow!("four panes"))
        .expect("PANES has four entries");

    for (pane, spec) in panes.iter_mut().zip(PANES) {
        pane.ui.panel.visible = spec.visible;
    }

    panes[PANE_PASSWORD].ui.panel.visible = lock == LOCK_LOCKED;

    let msgbox = msgbox::BoxPanel::build(game, &strings, &chains, zoom, layout.virtual_extent())?;

    let ui = &mut panes[PANE_MENU].ui;

    let delete_owner = ui
        .panel
        .element_or_create(MENU_DELETE, ElementType::Button, &ui.resource)
        .instance;

    let mut panels = Panels {
        panes,
        delete_owner,
        msgbox,
        guimat_textures: BTreeMap::new(),
        missing: Vec::new(),
        notes: Vec::new(),
        manager: Manager {
            screen: layout.virtual_screen(),
            ..Manager::default()
        },
        strings,
        clipboard: ui::text_entry::Local::default(),
    };

    panels.mode_panes(Mode::Select);

    panels.reanchor(backbuffer);

    census(game, &mut panels)?;

    panels.notes = panels
        .panes
        .iter()
        .flat_map(|p| p.ui.notes.iter().cloned())
        .collect();

    Ok(panels)
}

fn census(game: &GameDir, panels: &mut Panels) -> Result<()> {
    let interaction = Interaction::default();

    let was: Vec<bool> = panels.panes.iter().map(|p| p.ui.panel.visible).collect();

    let box_was = panels.msgbox.placed.ui.panel.visible;

    for pane in &mut panels.panes {
        pane.ui.panel.visible = true;
    }

    panels.msgbox.placed.ui.panel.visible = true;

    let referenced: Vec<(Option<usize>, i32)> = panels
        .panes
        .iter()
        .enumerate()
        .map(|(i, p)| (Some(i), p))
        .chain(std::iter::once((None, &panels.msgbox.placed)))
        .flat_map(|(i, p)| {
            p.ui.referenced_textures(&interaction)
                .into_iter()
                .map(move |slot| (i, slot))
        })
        .collect();

    for (pane, visible) in panels.panes.iter_mut().zip(was) {
        pane.ui.panel.visible = visible;
    }

    panels.msgbox.placed.ui.panel.visible = box_was;

    for (index, slot) in referenced {
        if panels.guimat_textures.contains_key(&slot) || panels.missing.contains(&slot) {
            continue;
        }

        let ui = match index {
            None => &panels.msgbox.placed.ui,
            Some(index) => &panels.panes[index].ui,
        };

        match login_panels::load_guimat_texture(game, ui, slot)? {
            Some(loaded) => {
                panels.guimat_textures.insert(slot, loaded);
            }
            None => panels.missing.push(slot),
        }
    }

    for loaded in panels.msgbox.textures(game, &mut panels.missing)? {
        panels.guimat_textures.entry(loaded.slot).or_insert(loaded);
    }

    Ok(())
}

pub fn upload(scene: &SelectChar, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.upload(gpu, renderer);

    scene.name_dialog.upload(gpu, renderer);

    scene.notice.upload(gpu, renderer);

    scene.message_panel.upload(gpu, renderer);

    renderer.upload_world(gpu, &scene.world);

    for (slot, loaded) in &scene.panels.guimat_textures {
        renderer.upload_once(
            gpu,
            TextureKey::GuiMat(*slot),
            &loaded.texture,
            AlphaPolicy::Stored,
            &format!("{} (guimat slot {slot})", loaded.path),
        );
    }
}

pub fn sync(scene: &mut SelectChar, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.sync(gpu, renderer);

    scene.name_dialog.sync(gpu, renderer);

    scene.notice.sync(gpu, renderer);

    scene.message_panel.sync(gpu, renderer);

    for (index, pane) in scene.panels.panes.iter().enumerate() {
        if pane.ui.panel.visible {
            renderer.sync_named_icons(gpu, caption_panel(index), &pane.ui);

            if scene.captions {
                renderer.sync_captions(gpu, caption_panel(index), &pane.ui);
            }
        }
    }

    for (slot, placed) in scene.panels.msgbox.visible() {
        let id = msgbox::caption_id(msgbox::NUI_ID, slot);

        renderer.sync_named_icons(gpu, id, &placed.ui);

        if scene.captions {
            renderer.sync_captions(gpu, id, &placed.ui);
        }
    }
}

pub fn frame_draws(scene: &mut SelectChar) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    append_frame_draws(scene, &mut out);

    out
}

pub(crate) fn append_frame_draws(scene: &mut SelectChar, out: &mut Vec<PlannedQuad>) {
    let captions = scene.captions;

    out.extend(
        scene
            .message_panel
            .frame_draws(&scene.layout, &scene.panels.strings, captions),
    );

    let layers = scene.panels.layers();

    for (slot, layer) in layers.iter().copied().enumerate().rev() {
        let interaction = scene.panels.manager.interaction(slot);

        let (id, placed) = match layer {
            None => {
                out.extend(
                    scene
                        .panels
                        .msgbox
                        .frame_draws(msgbox::NUI_ID, captions, &interaction),
                );

                continue;
            }
            Some(index) => (caption_panel(index), &mut scene.panels.panes[index]),
        };

        let interaction =
            scene
                .panels
                .manager
                .interaction_for(slot, &placed.ui.panel, placed.origin);

        if !placed.ui.panel.visible {
            continue;
        }

        placed
            .ui
            .panel
            .set_combo_screen(scene.layout.virtual_screen(), placed.origin);

        for draw in placed.ui.frame(&interaction, placed.origin) {
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

    scene.sync_name_focus();

    out.extend(scene.notice.frame_draws(captions));

    out.extend(
        scene
            .name_dialog
            .frame_draws(&scene.layout, &scene.panels.strings, captions),
    );

    out.extend(
        scene
            .legacy_boxes
            .frame_draws(&scene.layout, &scene.panels.strings, captions),
    );
}

pub fn caption_panel(pane: usize) -> PanelId {
    PANES[pane].nui_id
}

pub fn world_frame(scene: &mut SelectChar, backbuffer: (u32, u32)) -> WorldFrame {
    let frame = scene.world.frame(scene.time.now_ms, backbuffer);

    scene.update_hover(backbuffer);

    frame
}

#[derive(Debug)]

pub struct LanguageUpdate {
    game: GameDir,
    strings: Strings,
    notice: super::notice::LanguageUpdate,
    descriptions: [String; 4],
    info: Option<sel_info::SelInfo>,
    create_pane: Option<create::Pane>,
}

impl LanguageUpdate {
    pub fn prepare(scene: &SelectChar, game: &GameDir, strings: &Strings) -> Result<Self> {
        let descriptions = create::descriptions(game, Codepage::for_language(game.language()))?;

        let mut info = scene.info.clone();

        if let (Some(info), Some(block), Some(slot)) = (&mut info, &scene.account, scene.selected) {
            sel_info::relocalize(info, block, slot, strings);
        }

        let mut create_pane = scene.create_pane.clone();

        if let (Some(pane), Some(doll)) = (&mut create_pane, scene.class_picked)
            && let Some(&record) = create::CLASS_STRDEF.get(doll)
        {
            pane.class = strings.translate(&format!("ss_{record}"));

            pane.desc = descriptions[doll].clone();
        }

        Ok(Self {
            game: game.clone(),
            strings: strings.clone(),
            notice: super::notice::LanguageUpdate::prepare(&scene.notice, game)?,
            descriptions,
            info,
            create_pane,
        })
    }
}

impl SelectChar {
    pub fn apply_language(&mut self, update: LanguageUpdate) {
        let LanguageUpdate {
            game,
            strings,
            notice,
            descriptions,
            info,
            create_pane,
        } = update;

        self.panels.relocalize(&game, &strings);

        self.legacy_boxes.relocalize(&game, &strings);

        self.name_dialog.relocalize(&game, &strings);

        self.notice.relocalize(&game, &strings);

        self.notice.apply_language(notice);

        self.message_panel.relocalize(&game, &strings);

        if let Some(info) = &info {
            sel_info::apply(info, &mut self.panels.panes[PANE_SEL_INFO].ui);
        }

        if let Some(pane) = &create_pane {
            create::apply(pane, &mut self.panels.panes[PANE_CREATE].ui);
        }

        let title = match self.lock {
            LOCK_CHANGE_NEW => keypad::NEW_UISTRING,
            LOCK_CHANGE_CONFIRM | LOCK_CHANGE_SENT => keypad::CONFIRM_UISTRING,
            _ => keypad::TITLE_UISTRING,
        };

        self.pad
            .set_title_string(&mut self.panels.panes[PANE_PASSWORD].ui, &strings, title);

        self.descriptions = descriptions;

        self.info = info;

        self.create_pane = create_pane;
    }

    pub(super) fn resize(&mut self, chains: &crate::fonts::Chains, size: (u32, u32)) {
        let next = Layout::new(size.0, size.1);

        self.legacy_boxes.resize(chains, &self.layout, &next);

        self.name_dialog.resize(chains, &self.layout, &next);

        self.message_panel.resize(chains, size.0);

        self.notice.resize(&next);

        self.panels.msgbox.reposition(&next);

        for placed in &mut self.panels.panes {
            placed.ui.scale = next.ui_scale();

            placed.origin = placed.ui.reposition(next.virtual_screen());
        }

        self.panels.manager.screen = next.virtual_screen();

        self.layout = next;

        self.pointer(None);
    }

    pub fn pointer(&mut self, at: Option<(i32, i32)>) {
        self.cursor = at;

        if at.is_none() {
            self.world.actors.hovered_doll = None;
        }
    }

    fn update_hover(&mut self, backbuffer: (u32, u32)) {
        self.world.actors.hovered_doll = self
            .cursor
            .filter(|&(x, y)| {
                x >= 0 && y >= 0 && (x as u32) < backbuffer.0 && (y as u32) < backbuffer.1
            })
            .and_then(|(x, y)| {
                let ray = self.ray(x, y, backbuffer);

                let eye = (self.world.pose.eye.x, self.world.pose.eye.z);

                self.world
                    .actors
                    .actors
                    .iter()
                    .filter(|a| self.doll_row().contains(&a.record))
                    .filter(|a| {
                        !super::actors::ActorScene::hidden(a, None)
                            && !a.is_dead
                            && !super::hover::CORPSE_ACTIONS.contains(&a.action)
                            && super::hover::distance(eye, (a.position.x, a.position.z))
                                >= super::hover::PROXIMITY
                    })
                    .find(|a| self.doll_hit(&ray, a))
                    .map(|a| a.incarnation)
            });
    }

    pub fn clear_color(&self) -> wgpu::Color {
        self.world.clear_color()
    }

    pub fn set_mode(&mut self, mode: Mode, now_ms: u32) {
        self.mode = mode;

        self.world.actors.hovered_doll = None;

        let action = match mode {
            Mode::Select => self.cameras.select.clone(),
            Mode::Create => self.cameras.create.clone(),
        };

        self.world.play_camera(action, now_ms);

        self.panels.mode_panes(mode);
    }

    pub fn deselect_camera(&mut self, now_ms: u32) {
        self.world
            .play_camera(self.cameras.deselect.clone(), now_ms);
    }

    pub fn ray(&self, x: i32, y: i32, backbuffer: (u32, u32)) -> Ray {
        let projection =
            render::camera::Projection::new(backbuffer, (0, 0), self.world.environment.far_plane);

        Ray::unproject(
            &self.world.pose.view().matrix(),
            &projection.matrix(),
            (x, y),
            backbuffer,
        )
    }

    pub fn pick_doll(&self, ray: &Ray) -> Option<usize> {
        self.world
            .actors
            .actors
            .iter()
            .filter(|a| self.doll_row().contains(&a.record))
            .find(|a| self.doll_hit(ray, a))
            .map(|a| a.record)
    }

    fn doll_row(&self) -> std::ops::Range<usize> {
        match self.mode {
            Mode::Select => 0..account::SLOTS,
            Mode::Create => account::SLOTS..account::SLOTS * 2,
        }
    }

    fn doll_hit(&self, ray: &Ray, actor: &super::actors::Actor) -> bool {
        let model = self.world.actors.models[actor.model].model_id;

        pick::hits(ray, &pick::Body::new(actor.position, model, actor.scale))
    }

    pub fn click(
        &mut self,
        x: i32,
        y: i32,
        backbuffer: (u32, u32),
        items: &ItemList,
        strings: &Strings,
        now_ms: u32,
    ) -> bool {
        if self.lock == LOCK_LOCKED {
            return false;
        }

        let pane = match self.mode {
            Mode::Select => PANE_SEL_INFO,
            Mode::Create => PANE_CREATE,
        };

        if self.panels.panes[pane].ui.panel.visible {
            return false;
        }

        let ray = self.ray(x, y, backbuffer);

        let hit = self.pick_doll(&ray);

        self.apply_doll_pick(hit, items, strings, now_ms)
    }

    pub fn select_slot(
        &mut self,
        slot: usize,
        items: &ItemList,
        strings: &Strings,
        now_ms: u32,
    ) -> bool {
        if self.lock == LOCK_LOCKED
            || self.mode != Mode::Select
            || slot >= account::SLOTS
            || self.panels.panes[PANE_SEL_INFO].ui.panel.visible
            || !self.world.actors.actors.iter().any(|a| a.record == slot)
        {
            return false;
        }

        self.apply_doll_pick(Some(slot), items, strings, now_ms)
    }

    fn apply_doll_pick(
        &mut self,
        hit: Option<usize>,
        items: &ItemList,
        strings: &Strings,
        now_ms: u32,
    ) -> bool {
        match (self.mode, hit) {
            (Mode::Select, Some(record)) => {
                self.panels.show_delete(true);

                self.selected = Some(record);

                self.info = self
                    .account
                    .as_ref()
                    .and_then(|block| sel_info::fill(block, record, items, strings));

                if let Some(info) = &self.info {
                    if let Some(body) = info.gm_body_substitute {
                        self.selection_body_write = Some(body);

                        tracing::debug!(
                            target : "nwyd::body_state", event =
                            "selection_body_remembered", slot = record, body,
                            "Retail selection panel remembered body"
                        );
                    }

                    sel_info::apply(info, &mut self.panels.panes[PANE_SEL_INFO].ui);
                }

                self.refresh_selected_flag(record, items);

                self.panels.panes[PANE_SEL_INFO].ui.panel.visible = true;

                self.approach_doll(record, Approach::Toward, now_ms);

                self.world.play_sound(0x39);

                self.name_prompt(now_ms);

                true
            }
            (Mode::Select, None) => {
                self.panels.show_delete(false);

                self.selected = None;

                self.info = None;

                self.panels.panes[PANE_SEL_INFO].ui.panel.visible = false;

                false
            }
            (Mode::Create, hit) => {
                self.class_picked = None;

                let Some(record) = hit else {
                    return false;
                };

                let doll = record - account::SLOTS;

                let Some(filled) =
                    create::fill(doll, &self.descriptions, self.gauge_height, strings)
                else {
                    return false;
                };

                self.class_picked = Some(record);

                self.approach_doll(record, Approach::Toward, now_ms);

                self.panels.panes[PANE_CREATE].ui.panel.visible = true;

                create::apply(&filled, &mut self.panels.panes[PANE_CREATE].ui);

                self.world.actors.play(record, filled.action, now_ms);

                for slot in &filled.sounds {
                    self.world.play_sound(*slot);
                }

                self.create_pane = Some(filled);

                true
            }
        }
    }

    pub fn create_pressed(&self, typed: &str, reserved: &create::Reserved) -> create::Pressed {
        create::pressed(
            typed,
            self.class_picked.map(|record| record - account::SLOTS),
            self.account
                .as_ref()
                .map_or(Some(0), |block| block.first_free()),
            Codepage::Windows1252,
            reserved,
        )
    }

    pub fn refresh_selected_flag(&mut self, record: usize, items: &ItemList) {
        let actor = self.world.actors.actors.iter().find(|a| a.record == record);

        let loaded = actor
            .and_then(|a| a.cape_flag)
            .filter(|c| c.mesh != super::cape_flag::HIDDEN_MESH);

        let path = loaded
            .zip(actor)
            .and_then(|(loaded, actor)| {
                super::cape_flag::texture_slot(
                    loaded,
                    actor.helmet_item as u16,
                    items,
                    &self.flag_sets,
                    &self.flag_textures,
                )
            })
            .and_then(|slot| self.flag_textures.get(slot as usize))
            .map(|e| e.path_str())
            .unwrap_or_default();

        if let Some(info) = &mut self.info {
            info.flag = loaded.is_some();
        }

        let ui = &mut self.panels.panes[PANE_SEL_INFO].ui;

        let element = ui
            .panel
            .element_or_create("Flag", ElementType::Image, &ui.resource);

        element.visible = loaded.is_some();

        if loaded.is_some()
            && let ui::widget::Widget::Image(state) = &mut element.widget
        {
            state.external_image_file = path;
        }
    }

    pub fn create_clicked(&mut self, typed: &str, now_ms: u32) -> create::Pressed {
        let pressed = self.create_pressed(typed, &self.reserved_names);

        if matches!(pressed, create::Pressed::Send(_)) {
            self.create_armed_ms = now_ms;

            self.packet_sent(now_ms);

            self.set_button(PANE_MENU, MENU_CREATE, false);
        }

        pressed
    }

    pub fn create_back(&mut self, now_ms: u32) -> bool {
        let Some(record) = self.class_picked else {
            return false;
        };

        self.approach_doll(record, Approach::Away, now_ms);

        self.class_picked = None;

        self.create_pane = None;

        self.panels.panes[PANE_CREATE].ui.panel.visible = false;

        true
    }

    pub fn back(&mut self, now_ms: u32) -> bool {
        self.panels.show_delete(false);

        let Some(record) = self.selected else {
            return false;
        };

        self.approach_doll(record, Approach::Away, now_ms);

        self.selected = None;

        self.info = None;

        self.panels.panes[PANE_SEL_INFO].ui.panel.visible = false;

        true
    }

    pub fn connect_pressed(&mut self, rng: &mut Lcg, now_ms: u32) -> connect::Pressed {
        let has_doll = self
            .selected
            .is_some_and(|slot| self.world.actors.actors.iter().any(|a| a.record == slot));

        let pressed = connect::pressed(self.selected, has_doll, self.account.as_ref(), rng);

        if matches!(pressed, connect::Pressed::Send(_)) {
            self.connect_armed_ms = now_ms;

            self.packet_sent(now_ms);

            self.set_connect_buttons(false);
        }

        pressed
    }

    pub fn tick_buttons(&mut self, now_ms: u32) {
        if now_ms.wrapping_sub(self.connect_armed_ms) > connect::DISABLE_MS {
            self.set_connect_buttons(true);
        }

        if now_ms.wrapping_sub(self.create_armed_ms) > connect::CREATE_DISABLE_MS {
            self.set_button(PANE_MENU, MENU_CREATE, self.create_enabled);
        }

        if self.name_dialog.pending {
            self.set_connect_buttons(false);

            self.set_button(PANE_MENU, MENU_CREATE, false);
        }
    }

    pub(super) fn tick_create_alt(&mut self, now_ms: u32) {
        if !self.panels.manager.keys.alt || !self.panels.panes[PANE_CREATE].ui.panel.visible {
            return;
        }

        let layer = self
            .panels
            .layers()
            .iter()
            .position(|p| *p == Some(PANE_CREATE));

        if !self.create_back(now_ms) {
            self.set_mode(Mode::Select, now_ms);
        }

        if let Some(layer) = layer {
            let panel = &mut self.panels.panes[PANE_CREATE].ui.panel;

            if self
                .panels
                .manager
                .focused
                .is_some_and(|target| target.panel == layer)
            {
                self.panels.manager.set_focus(None, [(layer, &mut *panel)]);
            }

            self.panels.manager.leave_hovered_panel(layer, panel);
        }

        if let Some(edit) = self.panels.panes[PANE_CREATE]
            .ui
            .panel
            .element_mut(CREATE_INPUT, ElementType::ImeEditBox)
            .and_then(|e| e.widget.edit_mut())
        {
            edit.text.clear();

            edit.preedit = None;

            edit.caret = 0;

            edit.anchor = 0;

            edit.scroll = 0;

            edit.selection_held = false;

            edit.lose_caret_focus();
        }
    }

    fn set_connect_buttons(&mut self, enabled: bool) {
        self.set_button(PANE_SEL_INFO, SEL_INFO_CONNECT, enabled);

        self.set_button(PANE_SEL_INFO, SEL_INFO_BACK, enabled);

        self.set_button(PANE_MENU, MENU_DELETE, enabled);
    }

    fn set_button(&mut self, pane: usize, name: &str, enabled: bool) {
        let ui = &mut self.panels.panes[pane].ui;

        let element = if pane == PANE_MENU && name == MENU_DELETE {
            ui.panel
                .elements
                .iter_mut()
                .find(|e| e.instance == self.panels.delete_owner)
        } else {
            ui.panel.element_mut(name, ElementType::Button)
        };

        if let Some(element) = element {
            element.enabled = enabled;
        }
    }

    pub fn button_enabled(&self, pane: usize, name: &str) -> Option<bool> {
        self.panels.panes[pane]
            .ui
            .panel
            .element(name, ElementType::Button)
            .map(|e| e.enabled)
    }

    pub fn menu_back(&mut self, now_ms: u32) -> Option<msgbox::Request> {
        if self.mode == Mode::Create {
            self.set_mode(Mode::Select, now_ms);

            return None;
        }

        if self.launcher_login {
            return None;
        }

        let request = msgbox::Request::character_select(msgbox::CONTEXT_BACK, BACK_CONFIRM_STRDEF);

        self.raise(&request);

        Some(request)
    }

    pub fn escape(&mut self, now_ms: u32) -> Option<msgbox::Request> {
        if self.panels.panes[PANE_SEL_INFO].ui.panel.visible {
            if !self.back(now_ms) {
                self.set_mode(Mode::Select, now_ms);
            }

            return None;
        }

        if self.panels.panes[PANE_CREATE].ui.panel.visible {
            if !self.create_back(now_ms) {
                self.set_mode(Mode::Select, now_ms);
            }

            return None;
        }

        self.menu_back(now_ms)
    }

    pub(super) fn character_escape(&mut self) {
        if self.panels.msgbox.placed.ui.panel.visible {
            return;
        }

        let edit = self.panels.focused_edit().is_some();

        let consumes = edit
            && self.panels.manager.focused.is_some_and(|target| {
                self.panels
                    .layers()
                    .get(target.panel)
                    .copied()
                    .flatten()
                    .is_some_and(|pane| {
                        let panel = &self.panels.panes[pane].ui.panel;

                        panel.visible
                            && ui::input::owner_element(panel, target.owner)
                                .is_some_and(|(e, _)| e.visible && e.enabled)
                    })
            });

        if consumes {
            return;
        }

        self.escape(self.time.now_ms);
    }

    pub fn packet_sent(&mut self, now_ms: u32) {
        self.last_packet_ms = now_ms;
    }

    pub fn keepalive(&mut self, now_ms: u32) -> Option<[u8; KEEPALIVE_LEN]> {
        if now_ms.wrapping_sub(self.last_packet_ms) <= KEEPALIVE_MS {
            return None;
        }

        self.last_packet_ms = now_ms;

        let mut out = [0u8; KEEPALIVE_LEN];

        out[connect::OPCODE_OFF..connect::OPCODE_OFF + 2]
            .copy_from_slice(&KEEPALIVE_OPCODE.to_le_bytes());

        Some(out)
    }

    pub fn delete_pressed(&self) -> bool {
        self.lock != LOCK_LOCKED
    }

    pub fn delete_box(&self) -> Option<msgbox::Request> {
        self.delete_pressed()
            .then(|| msgbox::Request::character_select(delete::CONTEXT, delete::CONFIRM_STRDEF))
    }

    pub fn raise(&mut self, request: &msgbox::Request) -> Option<usize> {
        debug_assert!(
            self.panels.msgbox.covers(request),
            "a raiser named another .pane"
        );

        let strings = self.panels.strings.clone();

        let slot = self.boxes.raise(request, &strings)?;

        self.panels.msgbox.raised(&self.boxes, slot);

        Some(slot)
    }

    pub fn answer_box(&mut self, slot: usize, yes: bool) -> Option<(i32, u32, i32)> {
        let answered = self.boxes.answer(slot, yes)?;

        self.panels.msgbox.follow(&self.boxes);

        Some(answered)
    }

    pub fn box_hit(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let slot = self.panels.msgbox.slot?;

        if !self.panels.msgbox.placed.ui.panel.visible {
            return None;
        }

        self.panels.msgbox.hit(x, y).map(|yes| (slot, yes))
    }

    pub fn box_key(&mut self, key: u16) -> Option<(i32, u32, i32)> {
        let slot = self.panels.msgbox.slot?;

        let yes = match key {
            0x0D | 0x59 | 0x79 => true,
            0x4E | 0x6E => false,
            _ => return None,
        };

        let name = if yes {
            msgbox::ELEMENT_OK
        } else {
            msgbox::ELEMENT_CANCEL
        };

        let live = self
            .panels
            .msgbox
            .placed
            .ui
            .panel
            .element(name, ElementType::Button)
            .is_some_and(|e| e.visible && e.enabled);

        if !live {
            return None;
        }

        self.answer_box(slot, yes)
    }

    pub fn message_box_answer(&mut self, context: u32, answer: i32, rng: &mut Lcg) -> bool {
        if context != msgbox::CONTEXT_DELETE || !msgbox::answered(answer) {
            return false;
        }

        if !delete::may_confirm(self.selected) {
            return false;
        }

        self.lock = LOCK_CONFIRM_DELETE;

        self.reopen_pad(keypad::TITLE_UISTRING, rng);

        true
    }

    pub fn pin_press(&mut self, button: usize, now_ms: u32) -> bool {
        if !self.pad.debounce(now_ms) {
            return false;
        }

        let ui = &mut self.panels.panes[PANE_PASSWORD].ui;

        self.pad.press(button, ui)
    }

    pub fn pin_ok(&mut self, now_ms: u32) -> lock::Ok {
        let pressed = lock::ok(
            self.lock,
            self.selected,
            self.account.as_ref(),
            &self.pad.pin(),
        );

        if pressed.sends() {
            self.last_send_ms = now_ms;

            self.packet_sent(now_ms);

            self.panels.panes[PANE_PASSWORD].ui.panel.visible = false;

            self.lock = LOCK_AWAITING;
        }

        pressed
    }

    pub fn pin_change(&mut self, now_ms: u32) -> lock::Change {
        let pressed = lock::change(self.lock, &self.pad.pin(), &self.pad.saved_pin());

        let strings = self.panels.strings.clone();

        match &pressed {
            lock::Change::Submit(_) => {
                self.last_send_ms = now_ms;

                self.packet_sent(now_ms);

                self.panels.panes[PANE_PASSWORD].ui.panel.visible = false;

                let ui = &mut self.panels.panes[PANE_PASSWORD].ui;

                self.pad
                    .set_title_string(ui, &strings, keypad::CONFIRM_UISTRING);

                self.lock = LOCK_CHANGE_SENT;
            }
            lock::Change::Saved => {
                self.pad.save();

                let ui = &mut self.panels.panes[PANE_PASSWORD].ui;

                self.pad
                    .set_title_string(ui, &strings, keypad::CONFIRM_UISTRING);

                self.lock = LOCK_CHANGE_CONFIRM;
            }
            lock::Change::Confirm(_) => {
                self.last_send_ms = now_ms;

                self.packet_sent(now_ms);

                self.panels.panes[PANE_PASSWORD].ui.panel.visible = false;
            }
            lock::Change::Mismatch => {
                self.pad.clear();

                let ui = &mut self.panels.panes[PANE_PASSWORD].ui;

                self.pad
                    .set_title_string(ui, &strings, keypad::NEW_UISTRING);

                self.lock = LOCK_CHANGE_NEW;
            }
            lock::Change::TooShort | lock::Change::Ignored => {}
        }

        pressed
    }

    pub fn pin_cancel(&mut self) {
        self.panels.panes[PANE_PASSWORD].ui.panel.visible = false;
    }

    pub fn pin_reopen(&mut self, event: Mouse, now_ms: u32, rng: &mut Lcg) -> bool {
        let visible = self.panels.panes[PANE_PASSWORD].ui.panel.visible;

        if !lock::may_reopen(self.lock, visible, event, now_ms, self.last_send_ms) {
            return false;
        }

        self.reopen_pad(keypad::TITLE_UISTRING, rng);

        true
    }

    pub fn lock_reply(&mut self, opcode: u16, rng: &mut Lcg) -> lock::Reply {
        let reply = lock::reply(opcode, self.lock);

        match reply {
            lock::Reply::Unlocked => self.lock = LOCK_UNLOCKED,
            lock::Reply::ChangeAccepted => {
                self.pad.clear();

                self.lock = LOCK_CHANGE_NEW;

                self.reopen_pad(keypad::NEW_UISTRING, rng);
            }
            lock::Reply::Reset => {
                self.lock = LOCK_LOCKED;

                self.reopen_pad(keypad::TITLE_UISTRING, rng);
            }
            lock::Reply::Ignored => {}
        }

        reply
    }

    fn reopen_pad(&mut self, uistring: usize, rng: &mut Lcg) {
        self.pad.shuffle(rng);

        self.panels.panes[PANE_PASSWORD].ui.panel.visible = true;

        let strings = self.panels.strings.clone();

        let ui = &mut self.panels.panes[PANE_PASSWORD].ui;

        self.pad.set_title_string(ui, &strings, uistring);

        self.pad.label(ui);
    }

    pub fn packet_notice(&mut self, opcode: u16, now_ms: u32) -> bool {
        let record = match opcode {
            0x119 => 21,
            0x11A => 19,
            0x11B => 20,
            0x120 => 1510,
            _ => return false,
        };

        self.notice.post(
            &self.panels.strings.translate(&format!("ss_{record}")),
            2000,
            true,
            now_ms,
        )
    }

    pub fn delete_hold(&mut self, hold: account::DeleteHold, now_ms: u32) {
        if hold.current == 0 {
            self.notice.post(
                &self.panels.strings.translate("ss_1509"),
                2000,
                true,
                now_ms,
            );

            return;
        }

        let mut text = self.panels.strings.translate("ss_1508");

        for value in hold.remaining() {
            text = text.replacen("%d", &value.to_string(), 1);
        }

        let mut request = msgbox::Request::character_select(msgbox::CONTEXT_21F, 1508);

        request.body = msgbox::Body::Text(text.replace('/', "\n"));

        self.raise(&request);
    }

    pub fn refresh_account(
        &mut self,
        game: &GameDir,
        block: &Block,
        items: &ItemList,
        now_ms: u32,
    ) -> Result<()> {
        self.account = Some(*block);

        self.selected = None;

        self.info = None;

        self.world
            .actors
            .destroy(std::slice::from_ref(&(0..account::SLOTS)));

        let dolls: Vec<Doll> = dolls(Some(block), items)
            .into_iter()
            .filter(|d| d.record < account::SLOTS)
            .collect();

        self.world
            .actors
            .place_dolls(game, &dolls)
            .context("rebuilding the account row after a refresh")?;

        self.world
            .objects
            .load_held(game, &mut self.world.actors)
            .context("loading refreshed account weapons")?;

        self.world.play_sound(REFRESH_SOUND);

        self.set_mode(Mode::Select, now_ms);

        self.class_picked = None;

        self.create_pane = None;

        self.create_enabled = block.first_free().is_some();

        Ok(())
    }

    pub fn name_prompt(&mut self, now_ms: u32) -> bool {
        if !self.name_dialog.prompt_due(now_ms) || self.legacy_boxes.active().is_some() {
            return false;
        }

        let Some(slot) = self.selected.filter(|slot| *slot < 4) else {
            return false;
        };

        let Some(actor) = self.world.actors.actors.iter().find(|a| a.record == slot) else {
            return false;
        };

        let first = self
            .panels
            .strings
            .translate("ss_149")
            .replacen("%s", &actor.name, 1);

        let second = self.panels.strings.translate("ss_200");

        self.legacy_boxes.show(
            super::legacy_message_box::Owner::Question,
            &first,
            1,
            Some(&second),
        );

        self.panels.manager.pressed = None;

        self.panels.manager.capture = None;

        self.panels.manager.drag = None;

        true
    }

    pub(super) fn sync_name_focus(&mut self) {
        self.name_dialog
            .observe_legacy_focus(self.legacy_boxes.focus_generation());
    }

    pub fn name_result(&mut self, reply: account::NameResult, now_ms: u32) -> bool {
        self.sync_name_focus();

        self.name_dialog.pending = false;

        if reply.slot < 0 || (reply.result == 0 && reply.slot >= 4) {
            return false;
        }

        let record = match reply.result {
            0 => 201,
            1 => 202,
            2 => 203,
            3 => 205,
            4 => 1131,
            _ => 204,
        };

        self.message_panel.post(record, 3500, now_ms, true);

        if reply.result == 1 {
            self.name_dialog.open();

            self.legacy_boxes.yield_keyboard_to_edit();
        }

        if reply.result == 0 {
            let slot = reply.slot as usize;

            let name = if self.name_dialog.edit.text.is_empty() {
                self.account
                    .as_ref()
                    .map_or_else(String::new, |a| a.slots[slot].name())
            } else {
                std::mem::take(&mut self.name_dialog.edit.text)
            };

            let template = self.panels.strings.translate("ss_201");

            self.message_panel.state.text = Some(template.replacen("%s", &name, 1));

            self.name_dialog.edit.caret = 0;

            self.name_dialog.edit.anchor = 0;

            if let Some(selected) = self.selected {
                self.approach_doll(selected, Approach::Away, now_ms);
            } else {
                self.set_mode(Mode::Select, now_ms);
            }

            self.selected = None;

            self.info = None;

            self.panels.panes[PANE_SEL_INFO].ui.panel.visible = false;

            self.create_enabled = true;

            self.world
                .actors
                .destroy(std::slice::from_ref(&(slot..slot + 1)));

            if let Some(account) = &mut self.account {
                let unknown = account.slots[slot].unknown_4a8;

                account.slots[slot] = account::Slot {
                    unknown_4a8: unknown,
                    ..Default::default()
                };
            }
        }

        true
    }

    pub fn submit_name(&mut self, now_ms: u32) -> Option<Vec<u8>> {
        if !self.name_dialog.visible {
            return None;
        }

        self.name_dialog.close();

        let name = match super::character_name::validate_name(
            &mut self.name_dialog.edit.text,
            &self.reserved_names,
        ) {
            Ok(name) => name,
            Err(reject) => {
                self.message_panel.post(reject.strdef(), 2000, now_ms, true);

                return None;
            }
        };

        let slot = self.selected.filter(|slot| *slot < 4)?;

        let old = &self.account.as_ref()?.slots[slot];

        if old.is_empty() {
            return None;
        }

        let body = super::character_name::packet(slot, &old.name, &name);

        self.name_dialog.pending = true;

        self.name_dialog.requested_at_ms = now_ms;

        self.packet_sent(now_ms);

        Some(body.to_vec())
    }

    pub fn slot_effect(&mut self, slot: u32, now_ms: u32) -> bool {
        if slot >= 4 {
            return false;
        }

        let at = if slot < 2 {
            glam::Vec3::new(2052.0, 0.0, 2052.0 - 2.8 + slot as f32 * 1.4)
        } else {
            glam::Vec3::new(2052.0 - (slot - 1) as f32 * 1.4, 0.0, 2052.0)
        };

        let built = super::skill::judgement::build_at(at, 1, 0.1, now_ms);

        for child in built.orphans {
            self.world.effects.spawn(Box::new(child));
        }

        if let Some(mesh) = built.mesh {
            self.world.effects.spawn(Box::new(mesh));
        }

        self.world.effects.spawn(Box::new(built.token));

        true
    }

    pub fn approach_doll(&mut self, record: usize, direction: Approach, now_ms: u32) {
        let Some(actor) = self.world.actors.actors.iter().find(|a| a.record == record) else {
            return;
        };

        let doll = glam::Vec2::new(actor.position.x, actor.position.z);

        let pose = self.world.pose;

        let forward = render::camera::forward(pose.yaw, pose.pitch);

        let view = pose.view().matrix();

        let right = glam::Vec2::new(view.0[0], view.0[8]);

        let action = approach(self.mode, doll, forward, right, direction);

        self.world.play_camera(action, now_ms);
    }
}
