use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::actors::{self, Doll};
use crate::scene::attack;
use crate::scene::balloon;
use crate::scene::camera::{self, Camera};
use crate::scene::cast;
use crate::scene::cc;
use crate::scene::chat;
use crate::scene::consume;
use crate::scene::corpse;
use crate::scene::enter::Entered;
use crate::scene::ground::{self, Picker};
use crate::scene::hover;
use crate::scene::hud::{self, Hud, Plate};
use crate::scene::interact;
use crate::scene::item_move;
use crate::scene::landmarks::{self, Placed};
use crate::scene::login_static::{self, Layout, Rect, SceneTime};
use crate::scene::minimap::{self, Minimap, Scale};
use crate::scene::nameplate;
use crate::scene::notice;
use crate::scene::panels::{self, Action, Panels};
use crate::scene::pick::Ray;
use crate::scene::portal::{self, AirMoveFlight, AirMoveTick};
use crate::scene::screens;
use crate::scene::select_char;
use crate::scene::spend;
use crate::scene::weather;
use crate::scene::world::{self, World, WorldFrame, WorldSpec};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::item_effect::Effects;
use formats::data::mount;
use formats::mesh::appearance::{self, Look};
use formats::scene::scene_ui::{self, ControlType, Record, SceneUi};
use formats::text::Codepage;
use formats::texture::texture_set_list::{self, Item, TextureSetList};
use glam::Vec3;
use render::camera::Matrix4;
use render::device::Gpu;
use render::preset::RenderMode;
use render::sprite::{Quad, UvRule};

pub const SCENE_FILE: &[u8] = br"UI\FieldScene2.bin";

pub const MOUNT_DATA: &[u8] = b"MountData.bin";

pub const MOUNT_DATA_V: &[u8] = b"MountDataV.bin";

pub const MOUNTED_SPEED: f32 = 3.0;

pub fn mounted_pace(mount: &appearance::Mount) -> bool {
    match mount.model {
        Some(0x1F | 0x28 | 0x27) => true,
        Some(0x14) => mount.parts[0].mesh != 7,
        _ => false,
    }
}

pub const ZONE_SHIFT: i32 = 7;

pub const MINIMAP_NAME_ID: u32 = 0x121;

pub const MINIMAP_CONTROL_ID: i32 = -11;

pub const MINIMAP_ROTATION: f32 = f32::from_bits(0xBF49_0FDB);

pub const MINIMAP_COLOR: u32 = 0x80FF_FFFF;

pub const PLAYER_YAW: f32 = f32::from_bits(0x3EC9_0FDB);

pub const S_MODE: RenderMode = RenderMode::SControl2d;

pub const MARKER_LIFT: f32 = 0.3;

pub const MARKER_DROP: f32 = 0.2;

pub const MARKER_TEXTURE: usize = 0xE8;

pub const MARKER_SPIN_MS: u32 = 1_000;

pub const MARKER_SPIN_HALF: f32 = 500.0;

pub const MARKER_PULSE: f32 = 0.1;

pub const MARKER_PULSE_HARMONIC: f32 = 2.0;

pub const MARKER_MESH_IDS: [u32; 2] = [0x13C, 0x13D];

pub const MARKER_MESH_COLOURS: [u32; 2] = [0xFF11_1188, 0xFFFF_0000];

pub const MARKER_DISC_COLOUR: u32 = 0xFF99_99AA;

pub const MARKER_ANCHOR_COLOUR: u32 = 0xFFF2_6522;

pub const MARKER_GROW: f32 = 0.0005;

pub const MOVEMENT_DEFAULT: i32 = hud::MOVE_DEFAULT;

pub const ANCHOR_KEEP_RANGE: i32 = 0x11;

#[derive(Debug)]
pub struct AirMove {
    pub row: u32,
    pub flight: AirMoveFlight,
    restore: actors::AirMoveRestore,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundWave {
    pub centre: Vec3,
    pub birth_ms: u32,
}

impl GroundWave {
    pub fn uniforms(self, now: u32, origin: [f32; 2]) -> Option<[f32; 4]> {
        let age = now.wrapping_sub(self.birth_ms);

        if age >= 2000 {
            return None;
        }

        Some([
            self.centre.x - origin[0],
            self.centre.z - origin[1],
            (now as f64 / 300.0).rem_euclid(std::f64::consts::TAU) as f32,
            8.0 * age as f32 / 2000.0,
        ])
    }
}

pub struct FieldScene {
    pub(crate) analytics: crate::analytics::Analytics,
    pub interface: crate::game_settings::Interface,
    pub legacy_boxes: super::legacy_message_box::Dialogs,
    pub clipboard: ui::text_entry::Local,
    pub layout: Layout,
    pub time: SceneTime,
    pub world: World,
    pub camera: Camera,
    pub ground_wave: Option<GroundWave>,
    pub siege_effects: super::siege_effects::State,
    pub blind: Option<super::impact_scene::Blind>,
    pub camera_view: i32,
    pub automatic_movement: bool,
    pub low_ceiling: bool,
    pub entered: Entered,
    pub minimap: Minimap,
    pub ui: SceneUi,
    pub sets: TextureSetList,
    pub ui_textures: formats::texture::texture_list::TextureList,
    pub player: Option<usize>,
    pub air_move: Option<AirMove>,
    pub wire_speed: i32,
    pub minimap_visible: bool,
    pub map_moved: bool,
    map_button_pressed: [bool; 2],
    map_button_stamps: [u32; 2],
    pub coords_visible: bool,
    map_title_text: Option<String>,
    pub event_hud: super::event_hud::EventHud,
    pub question: super::question::State,
    pub long_message: super::long_message::State,
    pub long_message_captions: [String; 2],
    pub drop_gold_caption: String,
    pub reel: super::reel::Game,
    reel_position: Option<(f32, f32)>,
    reel_drag_cursor: Option<(i32, i32)>,
    reel_drag_capture: bool,
    pub quest_timer: super::quest_timer::State,
    pub combat: hud::Combat,
    pub lock: hud::InputLock,
    pub pending: super::pending::Pending,
    pub revive_pending: super::pending::RevivePending,
    pub(crate) death_return_birth_ms: u32,
    pub summon: super::summon::State,
    pub command_pending: super::pending::CommandPending,
    pub minipopup_icons: super::minipopup::IconTextures,
    pub quest_strings: Option<formats::text::string_table::StringTable>,
    pub collection: super::collection::Book,
    pub pk: bool,
    pub guild_chat: bool,
    pub guild_mark_off: bool,
    pub marks: super::guild_mark::Marks,
    pub guild_mark_ini: Option<std::path::PathBuf>,
    pub picker: Picker,
    pub marker: Option<ClickMarker>,
    pub hovered: Option<u16>,
    pub name_display: nameplate::Display,
    pub(crate) name_hover: nameplate::NameHover,
    pub target: Option<u16>,
    pub last_target_ms: u32,
    pub last_interact_ms: u32,
    pub nell_browser: crate::nell_browser::State,
    pub guild_application_entity: u32,
    pub shop_npc: Option<u16>,
    pub cargo_npc: Option<u16>,
    pub mission: Option<super::mission::State>,
    pub party: super::party::State,
    pub party_auto_join: bool,
    pub shop_sell: [formats::net::shop::SellRow; formats::net::shop::SELL_SLOTS],
    pub shop_price_mode: u32,
    pub shop_tax_percent: i32,
    pub shop_coupon_pending: bool,
    pub shop_coupon: bool,
    pub affect_stamps: [u32; formats::net::affect::AFFECTS],
    pub affect_time: super::affect_time::Clock,
    pub zone_time: super::zone_time::Shared,
    pub coin_prompt: Option<(i32, i32)>,
    pub cursor: (i32, i32),
    pub drag: Option<Drag>,
    pub right_held: bool,
    pub right_suspended: bool,
    pub chrome: std::collections::BTreeMap<u32, login_static::LoadedTexture>,
    pub cape_flag_textures: std::collections::BTreeMap<u32, login_static::LoadedTexture>,
    pub hud: Option<Hud>,
    pub panels: Option<Panels>,
    pub exit: Option<Exit>,
    pub server_select: Option<Exit>,
    pub countdown_quotient: u32,
    pub session: landmarks::Session,
    pub redirect: super::redirect::State,
    pub status: super::status::Status,
    pub aggregate_fetch: super::status::AggregateFetch,
    pub aggregate_statuses: Vec<Option<super::status::Status>>,
    pub servers: super::listener::Tables,
    pub text: super::stext::SceneText,
    pub captions: bool,
    pub statics: super::spawn::Statics,
    pub quick_bar: super::quick_bar::QuickBar,
    pub projected: Option<(Matrix4, Matrix4)>,
    pub roster: super::roster::Roster,
    pub hovered_drop: Option<u16>,
    pub pending_pickup: Option<u16>,
    pub drops: super::drops::Drops,
    pub gates: super::placed::Gates,
    pub init_items: Vec<formats::data::init_item::Entry>,
    pub outbox: Vec<Vec<u8>>,
    pub sent_dest: Option<(i32, i32)>,
    pub last_move_sent_ms: u32,
    pub attack: super::attack::State,
    pub skill_bar: super::skill_bar::State,
    pub cc: super::cc::State,
    pub damage: super::damage::Numbers,
    pub gate: super::gate::State,
    pub khepra: Option<u16>,
    pub city: super::city::State,
    pub siege: super::siege::State,
    pub screen_weather: super::screen_weather::Gusts,
    pub ambient: super::field_ambient::State,
    pub pending_music: Option<usize>,
    pub unconditional_sounds: Vec<super::audio::UnconditionalSound>,
    pub message_panel: super::kickout::MessagePanel,
    pub firecracker: super::firecracker::State,
    pub pk_warning_inside: bool,
    pub message_strings: ui::Strings,
    pub arena: super::arena::Board,
    pub lines: super::kept::Lines,
    pub records: super::kept::Records,
    pub saved_position: Option<(usize, (u16, u16))>,
    pub emote_keys: super::emote::Keys,
    pub trade: super::trade::Session,
}

mod language;

#[cfg(test)]
mod marker_tests;

#[cfg(test)]
mod overlay_tests;

pub(crate) use language::LanguageUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub ground: bool,
    pub suspended: bool,
    pub retry: bool,
    pub at: (i32, i32),
    pub tile: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickMarker {
    pub at: Vec3,
    pub tile: (i32, i32),
    pub walking: bool,
}

impl ClickMarker {
    pub fn spin(now_ms: u32) -> f32 {
        ((now_ms % MARKER_SPIN_MS) as f32 * std::f32::consts::PI) / MARKER_SPIN_HALF
    }

    pub fn pulse(now_ms: u32) -> f32 {
        (MARKER_PULSE_HARMONIC * ClickMarker::spin(now_ms)).sin() * MARKER_PULSE + 1.0
    }

    pub fn positions(&self) -> [Vec3; 3] {
        [
            self.at,
            self.at,
            Vec3::new(self.at.x, self.at.y - MARKER_DROP, self.at.z),
        ]
    }

    pub fn shown(action: usize) -> bool {
        action == appearance::ACTION_WALK || action == appearance::ACTION_RUN
    }

    pub fn draws(&self, now_ms: u32) -> MarkerDraws {
        let mut out = MarkerDraws::default();

        let [point0, point1, disc] = self.positions();

        let yaw = Self::spin(now_ms);

        for (i, at) in [point0, point1].into_iter().enumerate() {
            let mut effect = super::effect_mesh::EffectMesh::new(
                MARKER_MESH_IDS[i] as i32,
                MARKER_MESH_COLOURS[i],
                yaw,
                0,
                0,
            );

            effect.position = at;

            out.meshes.push(super::effect_tree::AuthoredMesh {
                mesh_id: MARKER_MESH_IDS[i],
                world: effect.world(),
                texture: None,
                material: super::effect_tree::MeshMaterial::Effect {
                    blend: effect.blend,
                    tint: effect.tint(),
                    uv_offset: effect.uv_offset,
                },
            });
        }

        out.quads.push(click_marker_quad(
            disc,
            MARKER_DISC_COLOUR,
            Self::pulse(now_ms),
            0,
        ));

        out
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MarkerDraws {
    pub meshes: Vec<super::effect_tree::AuthoredMesh>,
    pub quads: Vec<super::quads::GroundQuad>,
}

pub const ANCHOR_PIN_DRAWN: bool = true;

pub fn hover_drops(scene: &mut FieldScene) {
    let Some((view, projection)) = scene.projected else {
        return;
    };

    let viewport = (scene.layout.width, scene.layout.height);

    let ray = super::pick::Ray::unproject(&view, &projection, scene.cursor, viewport);

    let present = scene.player.is_some();

    scene.hovered_drop = super::drops::hover(&mut scene.drops, &ray, present);
}

pub fn hover_gates(scene: &mut FieldScene) {
    let Some((view, projection)) = scene.projected else {
        return;
    };

    let viewport = (scene.layout.width, scene.layout.height);

    let ray = super::pick::Ray::unproject(&view, &projection, scene.cursor, viewport);

    let body = scene
        .player
        .map(|index| scene.world.actors.actors[index].position);

    scene.hovered_drop = super::placed::hover(&mut scene.gates, &ray, body, scene.hovered_drop);
}

pub fn drop_draws(scene: &FieldScene) -> Vec<super::drops::DropDraw> {
    let world = &scene.world;

    let terrains = world.terrains();

    super::drops::draws(&scene.drops, |at| {
        super::lights::sample(&terrains, at.x, at.z)
    })
}

pub fn gate_draws(scene: &FieldScene) -> Vec<super::placed::GateDraw> {
    let world = &scene.world;

    let terrains = world.terrains();

    super::placed::draws(&scene.gates, |at| {
        super::lights::sample(&terrains, at.x, at.z)
    })
}

pub fn click_markers(scene: &FieldScene, now_ms: u32) -> MarkerDraws {
    let mut out = MarkerDraws::default();

    let walking = scene
        .player
        .map(|index| {
            ClickMarker::shown(scene.world.actors.actors[index].action)
                && !scene.automatic_movement
                && scene.attack.stuck.is_none()
                && scene.attack.approach_target.is_none()
                && scene.pending_pickup.is_none()
        })
        .unwrap_or(false);

    if let Some(marker) = scene.marker.filter(|_| walking) {
        out = marker.draws(now_ms);
    }

    if ANCHOR_PIN_DRAWN && scene.combat.movement == 1 && scene.player.is_some() {
        let (x, z) = scene.cc.anchor;

        let zone = scene.world.actors.zone;

        let floor = formats::world::path::HeightField::window(&scene.world.actors.window, zone)
            .ground(x as i32, z as i32);

        let y = floor + MARKER_LIFT;

        out.quads.push(click_marker_quad(
            Vec3::new(x, y, z),
            MARKER_ANCHOR_COLOUR,
            1.0,
            1,
        ));
    }

    out
}

fn click_marker_quad(at: Vec3, argb: u32, scale: f32, ordinal: usize) -> super::quads::GroundQuad {
    super::quads::GroundQuad {
        owner: 0,
        slot: super::quads::Slot::Placed,
        texture: MARKER_TEXTURE,
        pass: render::billboard::BillboardPass::Additive,
        anchor: at,
        base_scale: Vec3::splat(scale),
        grow: MARKER_GROW,
        spin_period: 0,
        ordinal,
        argb,
    }
}

pub const EXIT_MS: u32 = 5_000;

pub const EXIT_TICK_MS: u32 = 1_000;

pub const TOAST_QUIT: usize = 224;

pub const TOAST_LOGOUT: usize = 225;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Characters,
    Quit,
    ServerSelect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub kind: ExitKind,
    pub at_ms: u32,
    pub shown: Option<u32>,
}

impl Exit {
    pub fn seconds(&self, now_ms: u32) -> u32 {
        self.at_ms.wrapping_sub(now_ms).wrapping_add(EXIT_MS) / EXIT_TICK_MS + 1
    }

    pub fn fired(&self, now_ms: u32) -> bool {
        self.at_ms.wrapping_add(EXIT_MS) < now_ms
    }

    pub fn record(&self) -> usize {
        match self.kind {
            ExitKind::Characters => TOAST_LOGOUT,
            ExitKind::Quit | ExitKind::ServerSelect => TOAST_QUIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    ServerSelect,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    pub hud: bool,
    pub captions: bool,
}

impl Default for Chrome {
    fn default() -> Chrome {
        Chrome {
            hud: true,
            captions: false,
        }
    }
}

impl Chrome {
    pub fn bare() -> Chrome {
        Chrome {
            hud: false,
            captions: false,
        }
    }
}

pub const PLAYER_RECORD: usize = 0;

pub fn build_entry(
    game: &GameDir,
    backbuffer: (u32, u32),
    time: &SceneTime,
    entered: &Entered,
    chrome: &Chrome,
) -> Result<FieldScene> {
    let mut scene = build(game, backbuffer, time, entered, chrome)?;

    scene.camera.start_shake(10, time.now_ms);

    Ok(scene)
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    time: &SceneTime,
    entered: &Entered,
    chrome: &Chrome,
) -> Result<FieldScene> {
    let zone_name = entered.zone_name();

    let mut world = world::build_spec(game, &WorldSpec::field(&zone_name), &time.calendar)
        .with_context(|| format!("building the field world at {zone_name}"))?;

    let items = super::item_table::load(game)?;

    let init_items = super::init_place::load(game)?;

    let mount_data = mount::parse_data(&game.read(MOUNT_DATA).context("reading MountData.bin")?)
        .context("parsing MountData.bin")?;

    let visuals = mount::parse_visual(&game.read(MOUNT_DATA_V).context("reading MountDataV.bin")?)
        .context("parsing MountDataV.bin")?;

    let effects = Effects::new(&items, &mount_data);

    let dolls = player_doll(entered, &effects, &visuals, 0)
        .into_iter()
        .collect::<Vec<_>>();

    world
        .actors
        .place_dolls(game, &dolls)
        .context("placing the player")?;

    world
        .objects
        .load_held(game, &mut world.actors)
        .context("loading the player's weapons")?;

    world
        .objects
        .load_by_id(game, &MARKER_MESH_IDS)
        .context("loading the click marker meshes")?;

    let objects = &world.objects;

    world.actors.place_swing_radii(|id| objects.mesh_max_z(id));

    let player = world
        .actors
        .actors
        .iter()
        .position(|a| a.record == PLAYER_RECORD);

    world.actors.viewer = Some(entered.client_id);

    if let Some(index) = player {
        world.actors.actors[index].speed = entered.speed();

        world.actors.actors[index].guild_rank = u16::from(entered.mob_guild_rank);

        if let Some(mount) = dolls.first().and_then(|d| d.look.mount)
            && world.actors.actors[index].mounted
            && mounted_pace(&mount)
        {
            world.actors.actors[index].speed = MOUNTED_SPEED;
        }
    }

    let mut entered = *entered;

    entered.refresh_class();

    entered.kingdom = dolls.first().map_or(0, |doll| doll.look.kingdom);

    entered.quick_selected = 0;

    let entry_skill = super::skill_bar::entry_selection(&entered.quick);

    if let Some(slot) = entry_skill {
        entered.quick_selected = slot as u8;
    }

    let entered = &entered;

    let ui = scene_ui::parse(
        &game
            .read(SCENE_FILE)
            .context(r"reading UI\FieldScene2.bin")?,
    )
    .context(r"parsing UI\FieldScene2.bin")?;

    let sets = texture_set_list::parse(
        &game
            .read(login_static::TEXTURE_SET_LIST)
            .context(r"reading UI\UITextureSetList.txt")?,
    )
    .context("parsing UITextureSetList.txt")?;

    let minimap = Minimap::build(game, entered.zone(), Scale::Full)
        .with_context(|| format!("composing the minimap around {zone_name}"))?;

    let skills = match game.read(super::tooltip::SKILL_DATA) {
        Ok(raw) => formats::data::skill_data::parse(&raw).ok(),
        Err(_) => None,
    };

    let list = formats::texture::texture_list::parse(
        &game
            .read(login_static::TEXTURE_LIST)
            .context(r"reading UI\UITextureListN.bin")?,
    )
    .context("parsing UITextureListN.bin")?;

    let mut cape_flag_textures = std::collections::BTreeMap::new();

    if let Some(flag_set) = sets.get(super::cape_flag::TEXTURE_SET) {
        for slot in flag_set
            .items
            .iter()
            .filter_map(|item| u32::try_from(item.texture_index).ok())
            .filter(|slot| *slot <= 0x200)
        {
            if cape_flag_textures.contains_key(&slot) || list.get(slot as usize).is_none() {
                continue;
            }

            if let Some(loaded) = login_static::load_texture(game, &list, slot)
                .with_context(|| format!("loading the character-sheet flag texture slot {slot}"))?
            {
                cape_flag_textures.insert(slot, loaded);
            }
        }
    }

    let mut map_chrome = std::collections::BTreeMap::new();

    for name_id in [landmarks::COMPASS_NAME_ID, landmarks::TITLE_PANEL_NAME_ID] {
        let Some(slot) = panel(&ui, name_id).and_then(|r| texture_slot(&sets, r)) else {
            continue;
        };

        if let Some(loaded) = login_static::load_texture(game, &list, slot)
            .with_context(|| format!("loading the minimap chrome's texture slot {slot}"))?
        {
            map_chrome.insert(slot, loaded);
        }
    }

    for record in ui.records.iter().filter(|r| {
        r.control == ControlType::Panel && r.header.parent_id == super::long_message::PANEL_ID
    }) {
        if let Some(slot) = texture_slot(&sets, record)
            && !map_chrome.contains_key(&slot)
            && let Some(loaded) = login_static::load_texture(game, &list, slot)?
        {
            map_chrome.insert(slot, loaded);
        }
    }

    if let Some(slot) = panel(&ui, 0x16200).and_then(|r| texture_slot(&sets, r))
        && !map_chrome.contains_key(&slot)
        && let Some(loaded) = login_static::load_texture(game, &list, slot)?
    {
        map_chrome.insert(slot, loaded);
    }

    if let Some(item) = sets.item(super::nameplate::SIGN_SET, 0)
        && let Ok(slot) = u32::try_from(item.texture_index)
        && !map_chrome.contains_key(&slot)
        && let Some(loaded) = login_static::load_texture(game, &list, slot)
            .with_context(|| format!("loading the shop signboard's texture slot {slot}"))?
    {
        map_chrome.insert(slot, loaded);
    }

    for set in super::message_panel::SETS {
        if let Some(item) = sets.item(set, 0)
            && let Ok(slot) = u32::try_from(item.texture_index)
            && !map_chrome.contains_key(&slot)
            && let Some(loaded) = login_static::load_texture(game, &list, slot)?
        {
            map_chrome.insert(slot, loaded);
        }
    }

    if super::firecracker::rectangle(&ui).is_some() {
        for set in 375..=380 {
            if let Some(set) = sets.get(set) {
                for item in &set.items {
                    if let Ok(slot) = u32::try_from(item.texture_index)
                        && !map_chrome.contains_key(&slot)
                        && let Some(loaded) = login_static::load_texture(game, &list, slot)?
                    {
                        map_chrome.insert(slot, loaded);
                    }
                }
            }
        }
    }

    for set in super::damage::SETS {
        let Some(items) = super::damage::set_items(&sets, set) else {
            continue;
        };

        for slot in items
            .iter()
            .filter_map(|item| u32::try_from(item.texture_index).ok())
        {
            if map_chrome.contains_key(&slot) {
                continue;
            }

            if let Some(loaded) = login_static::load_texture(game, &list, slot)
                .with_context(|| format!("loading number set {set:#04X}'s texture slot {slot}"))?
            {
                map_chrome.insert(slot, loaded);
            }
        }
    }

    if let Some(record) = panel(&ui, 0x1911).filter(|r| r.header.parent_id == super::reel::PANEL)
        && let Some(slot) = texture_slot(&sets, record)
        && !map_chrome.contains_key(&slot)
        && let Some(loaded) = login_static::load_texture(game, &list, slot)?
    {
        map_chrome.insert(slot, loaded);
    }

    for record in ui
        .records
        .iter()
        .filter(|r| r.header.parent_id == super::reel::PANEL && r.control == ControlType::Button)
    {
        if let Some(set) = record
            .control_id()
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| sets.get(id))
        {
            for item in &set.items {
                if let Ok(slot) = u32::try_from(item.texture_index)
                    && !map_chrome.contains_key(&slot)
                    && let Some(loaded) = login_static::load_texture(game, &list, slot)?
                {
                    map_chrome.insert(slot, loaded);
                }
            }
        }
    }

    for set in super::reel::BASE_SETS[0]..=super::reel::COUNTER_FRAME_SET {
        let Some(row) = sets.get(set) else {
            continue;
        };

        for slot in row
            .items
            .iter()
            .filter_map(|item| u32::try_from(item.texture_index).ok())
        {
            if map_chrome.contains_key(&slot) {
                continue;
            }

            if let Some(loaded) = login_static::load_texture(game, &list, slot)
                .with_context(|| format!("loading reel set {set}'s texture slot {slot}"))?
            {
                map_chrome.insert(slot, loaded);
            }
        }
    }

    let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

    let text = super::stext::SceneText::new(&chains, backbuffer.0);

    let strings = crate::scene::login_panels::strings(game);

    let map_title_text = ui
        .records
        .iter()
        .find(|r| {
            r.control == ControlType::Text && r.header.name_id == landmarks::TITLE_TEXT_NAME_ID
        })
        .and_then(|r| r.fields.get(10))
        .and_then(|n| usize::try_from(*n).ok())
        .map(|n| {
            super::stext::display_text(
                strings
                    .ui
                    .as_ref()
                    .map(|table| table.get(n))
                    .unwrap_or_default(),
            )
        });

    let zone_time: super::zone_time::Shared = Default::default();

    let (mut hud, screens) = match chrome.hud {
        true => {
            let hud = hud::build(game, backbuffer, &strings, &chains, chrome.captions)
                .context("opening contents group 4 — the HUD")?;

            let mut screens = panels::build(game, backbuffer, &strings, &chains, chrome.captions)
                .context("opening contents group 4 — the toggled screens")?;

            screens.zone_time = zone_time.clone();

            screens.field_close_owner_present = player.is_some();

            crate::scene::screens::build_all(&mut screens, entered);

            panels::load_icon_sheets(game, &mut screens)
                .context("loading the item-icon sheets the inventory draws out of")?;

            (Some(hud), Some(screens))
        }
        false => (None, None),
    };

    if let (Some(hud), Some(slot)) = (hud.as_mut(), entry_skill) {
        hud.select_skill_only(slot);
    }

    let minipopup_icons = match chrome.hud {
        true => super::minipopup::load_textures(game)
            .context("loading the MiniPopup's direct icon sheets")?,
        false => super::minipopup::IconTextures::default(),
    };

    let session = session(game);

    let guild_mark_ini = crate::user_paths::guild_mark(game)?;

    let mark_flag = load_guild_mark(&guild_mark_ini);

    let long_message = super::long_message::State::new(
        ui.records.iter().any(|record| {
            record.control == ControlType::Panel
                && record.header.name_id == super::long_message::PANEL_ID
        }),
        strings
            .ui
            .as_ref()
            .map(|table| table.get(super::long_message::INITIAL_CAPTION).to_string())
            .unwrap_or_default(),
        time.now_ms,
    );

    let long_message_captions = [
        strings
            .strdef
            .as_ref()
            .map(|table| table.get(super::long_message::QUIZ_CAPTION).to_string())
            .unwrap_or_default(),
        strings
            .strdef
            .as_ref()
            .map(|table| table.get(super::long_message::GUILD_CAPTION).to_string())
            .unwrap_or_default(),
    ];

    let mut scene = FieldScene {
        analytics: Default::default(),
        interface: crate::game_settings::Interface::default()
            .with_costume_flags(entered.wearer.flags),
        clipboard: ui::text_entry::Local::default(),
        drop_gold_caption: strings
            .strdef
            .as_ref()
            .map(|s| s.get(65).to_string())
            .unwrap_or_default(),
        layout: Layout::new(backbuffer.0, backbuffer.1),
        time: *time,
        world,
        camera: Camera::field(),
        ground_wave: None,
        siege_effects: Default::default(),
        blind: None,
        camera_view: 0,
        automatic_movement: false,
        low_ceiling: false,
        entered: *entered,
        minimap,
        ui,
        sets,
        ui_textures: list,
        player,
        air_move: None,
        wire_speed: entered.speed() as i32,
        minimap_visible: false,
        map_moved: false,
        map_button_pressed: [false; 2],
        map_button_stamps: [0; 2],
        coords_visible: false,
        name_display: nameplate::Display::default(),
        map_title_text,
        event_hud: super::event_hud::EventHud::new(&Layout::new(backbuffer.0, backbuffer.1)),
        question: super::question::State::new(backbuffer.0, backbuffer.1),
        long_message,
        long_message_captions,
        reel: super::reel::Game::new(time.now_ms),
        reel_position: None,
        reel_drag_cursor: None,
        reel_drag_capture: false,
        quest_timer: super::quest_timer::State::default(),
        drag: None,
        right_held: false,
        right_suspended: false,
        combat: hud::Combat {
            movement: MOVEMENT_DEFAULT,
            ..hud::Combat::default()
        },
        lock: hud::InputLock::default(),
        pending: super::pending::Pending::default(),
        revive_pending: super::pending::RevivePending::default(),
        death_return_birth_ms: 0,
        summon: super::summon::State::default(),
        command_pending: super::pending::CommandPending::default(),
        minipopup_icons,
        pk: PK_MODE,
        guild_chat: GUILD_CHAT_START,
        guild_mark_off: mark_flag,
        marks: marks_for(&strings, mark_flag, session.server),
        guild_mark_ini: Some(guild_mark_ini),
        chrome: map_chrome,
        cape_flag_textures,
        hud,
        panels: screens,
        exit: None,
        server_select: None,
        countdown_quotient: 0,
        picker: Picker::default(),
        marker: None,
        hovered: None,
        name_hover: nameplate::NameHover::default(),
        target: None,
        last_target_ms: 0,
        last_interact_ms: 0,
        nell_browser: crate::nell_browser::State::default(),
        guild_application_entity: 0,
        shop_npc: None,
        cargo_npc: None,
        mission: None,
        party: super::party::State::default(),
        party_auto_join: false,
        shop_sell: [formats::net::shop::SellRow::default(); formats::net::shop::SELL_SLOTS],
        shop_price_mode: 0,
        shop_tax_percent: 0,
        shop_coupon_pending: false,
        shop_coupon: false,
        affect_stamps: [0; formats::net::affect::AFFECTS],
        affect_time: super::affect_time::Clock {
            server_seconds: entered.server_seconds,
            received_raw_ms: time.raw_ms,
            ..Default::default()
        },
        zone_time,
        coin_prompt: None,
        cursor: (backbuffer.0 as i32 / 2, backbuffer.1 as i32 / 2),
        session,
        redirect: super::redirect::State::default(),
        status: super::status::Status::default(),
        aggregate_fetch: super::status::AggregateFetch::default(),
        aggregate_statuses: Vec::new(),
        servers: crate::scene::login_panels::server_tables(game),
        text,
        captions: chrome.captions,
        quick_bar: super::quick_bar::load(game, &entered.name(), time.calendar.unix_seconds as u32),
        statics: super::spawn::Statics {
            items,
            mounts: mount_data,
            visuals,
            skills,
            icons: formats::data::item_icon::parse(
                &game.read(super::panels::ITEM_ICON).unwrap_or_default(),
            ),
        },
        projected: None,
        damage: super::damage::Numbers::default(),
        gate: super::gate::State::new(),
        khepra: None,
        city: super::city::State::new(),
        siege: super::siege::State::new(),
        screen_weather: super::screen_weather::Gusts::default(),
        ambient: super::field_ambient::State::default(),
        pending_music: None,
        unconditional_sounds: Vec::new(),
        legacy_boxes: super::legacy_message_box::Dialogs::build(game, backbuffer.0)?,
        message_panel: super::kickout::MessagePanel::new(),
        firecracker: super::firecracker::State::default(),
        pk_warning_inside: false,
        message_strings: strings.clone(),
        arena: super::arena::Board::default(),
        lines: super::kept::Lines::new(),
        records: super::kept::Records::new(),
        saved_position: None,
        emote_keys: super::emote::Keys::default(),
        trade: super::trade::Session::default(),
        roster: super::roster::Roster::default(),
        hovered_drop: None,
        pending_pickup: None,
        drops: super::drops::Drops::default(),
        gates: super::placed::Gates::default(),
        init_items,
        outbox: Vec::new(),
        sent_dest: None,
        last_move_sent_ms: 0,
        attack: super::attack::State::default(),
        skill_bar: super::skill_bar::State::default(),
        cc: super::cc::State::default(),
        quest_strings: crate::ui::read_language(game, "QuestStringTable.txt")
            .ok()
            .map(|(bytes, cp)| formats::text::string_table::quest::parse(&bytes, cp)),
        collection: super::collection::Book {
            table: super::collection::load(game),
            ..Default::default()
        },
    };

    if let Some(index) = scene.player {
        let tile = attack::cell_of(&scene.world.actors.actors[index]);

        scene.cc.auto_move = tile;

        scene.sent_dest = Some(tile);
    }

    scene.world.pose = scene.pose();

    reanchor(&mut scene);

    scene.world.weather = weather::Weather::new(u32::from(entered.weather));

    let tile = player_tile(&scene);

    let env_type = scene.world.env_global;

    let now_ms = scene.time.now_ms;

    scene
        .world
        .weather
        .apply(&mut scene.world.sky, env_type, tile, now_ms);

    let blend = scene.world.sky.update_blend(now_ms);

    scene.world.apply_day(blend);

    refresh_sheet_magic(&mut scene);

    super::guild::refresh_session(&mut scene);

    if scene.entered.status.level == 0
        && let Some(panels) = scene.panels.as_mut()
    {
        super::minipopup::set(panels, super::minipopup::Notice::Quest, true);

        let row = crate::ui::read_language(game, "QuestMessage.txt")
            .ok()
            .and_then(|(bytes, cp)| {
                formats::text::quest_message::parse(&bytes, cp)
                    .into_iter()
                    .find(|row| row.id == 1)
            });

        let (text, color) = row
            .map(|row| row.arrival(i32::from(scene.entered.equipment.body().index) % 10))
            .unwrap_or_else(|| (panels.strings.translate("ss_307"), 0xffaaaaff));

        super::chat::append(panels, &text, color, None);
    }

    refresh_channel_popup(&mut scene);

    Ok(scene)
}

pub fn player_tile(scene: &FieldScene) -> (i32, i32) {
    scene.player.map_or((0, 0), |index| {
        let at = scene.world.actors.actors[index].position;

        world::tile_of(at.x, at.z)
    })
}

pub fn item_position(scene: &FieldScene) -> (u16, u16) {
    scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map_or((0, 0), |actor| {
            (
                actor.position.x as i32 as u16,
                actor.position.z as i32 as u16,
            )
        })
}

fn refresh_item_inputs(scene: &mut FieldScene) {
    let position = item_position(scene);

    let actor = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index));

    if let Some(panels) = &mut scene.panels {
        panels.live.position = position;

        panels.live.affect_20 = actor.is_some_and(|actor| actor.flags.get(0xB39) == 1);

        panels.live.move_alpha = actor.map_or(0.0, |actor| actor.mover.move_alpha);

        panels.live.pet = actor.map_or(0, |actor| actor.pet_item as i16);

        panels.live.mode_letter =
            formats::net::enter::selected(&scene.entered.quick, scene.entered.quick_selected)
                .map_or(0, |b| if b > 0x68 { b.wrapping_add(0x5F) } else { b });

        panels.live.last_attack_ms = scene.attack.last_attack_ms;
    }
}

pub fn sheet_magic_display(scene: &FieldScene) -> super::magic_damage::Display {
    let Some(code) =
        super::magic_damage::selected_code(&scene.entered.quick, scene.entered.quick_selected)
    else {
        return super::magic_damage::Display::Invalid;
    };

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|skills| skills.get(code as usize))
    else {
        return super::magic_damage::Display::Invalid;
    };

    let effects = scene.statics.effects();

    let weapon = super::magic_damage::weapon_bonus(&scene.entered.equipment, &effects);

    let input = super::magic_damage::Input::of(&scene.entered, weapon);

    let mut weather = scene.world.weather.code as i32;

    if weather == 3 || super::world::is_special_tile(player_tile(scene)) {
        weather = 2;
    }

    super::magic_damage::display(Some(super::magic_damage::damage(
        code, skill, weather, input,
    )))
}

pub fn sheet_flag_display(
    scene: &FieldScene,
) -> Option<(super::cape_flag::Flag, Option<[u32; 2]>)> {
    let strings = &scene.panels.as_ref()?.strings;

    let loaded = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .and_then(|actor| actor.cape_flag);

    let body = scene.entered.equipment.body();

    let equipped = scene.entered.equipment.items[15];

    let flag = super::cape_flag::resolve(
        loaded,
        body.appearance,
        equipped,
        &scene.statics.items,
        &scene.sets,
        &scene.ui_textures,
        strings,
    )?;

    let size = flag.texture_slot.and_then(|slot| {
        scene
            .cape_flag_textures
            .get(&slot)
            .map(|loaded| [loaded.texture.width, loaded.texture.height])
    });

    Some((flag, size))
}

pub fn refresh_sheet_magic(scene: &mut FieldScene) {
    let display = sheet_magic_display(scene);

    let flag = sheet_flag_display(scene);

    if let Some(panels) = scene.panels.as_mut() {
        super::screens::sheet_magic(panels, display);

        super::screens::sheet_flag(panels, flag);
    }
}

pub fn reanchor(scene: &mut FieldScene) {
    let here = scene.player.map(|index| {
        let at = scene.world.actors.actors[index].position;

        (at.x, at.z)
    });

    scene.cc.anchor = match (scene.combat.movement == hud::MOVE_PLANS_A_PATH, here) {
        (true, Some(at)) => at,
        _ => super::cc::ANCHOR_CLEARED,
    };
}

pub const MARK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn marks_for(strings: &ui::Strings, off: bool, server: usize) -> super::guild_mark::Marks {
    let mut marks = super::guild_mark::Marks::default().with_strings(strings, server);

    marks.off = off;

    marks
}

fn load_guild_mark(path: &std::path::Path) -> bool {
    let text = crate::user_paths::read_to_string(path).unwrap_or_default();

    let mut ini = formats::config::ini::parse(&text);

    let (off, needs_write) = formats::config::ini::guild_mark::read(&ini);

    if needs_write {
        ini.set(
            formats::config::ini::guild_mark::SECTION,
            formats::config::ini::guild_mark::KEY,
            "0",
        );

        let _ = crate::user_paths::write(path, ini.to_string_crlf().as_bytes());
    }

    off
}

pub fn toggle_guild_mark(path: &std::path::Path, cached: bool) -> bool {
    let Ok(text) = crate::user_paths::read_to_string(path) else {
        return cached;
    };

    let mut ini = formats::config::ini::parse(&text);

    if ini
        .get_int(
            formats::config::ini::guild_mark::SECTION,
            formats::config::ini::guild_mark::KEY,
        )
        .is_none()
    {
        return cached;
    }

    let off = formats::config::ini::guild_mark::toggle(&mut ini);

    let _ = crate::user_paths::write(path, ini.to_string_crlf().as_bytes());

    off
}

fn session(game: &GameDir) -> landmarks::Session {
    let names = game
        .read(b"sn.bin")
        .map(|bytes| formats::config::sn::parse(&bytes, Codepage::Windows1252))
        .unwrap_or_else(|_| formats::config::sn::parse(&[], Codepage::Windows1252));

    landmarks::Session {
        server: 0,
        channel: 0,
        names: names.labels.to_vec(),
    }
}

pub fn adopt_session(scene: &mut FieldScene, picked: &super::listener::Picked) {
    scene.session.adopt(picked.server, picked.channel as i32);

    scene.marks.server = picked.server as u32;

    scene.status = picked.status.clone();

    refresh_channel_popup(scene);

    super::guild::refresh_inputs(scene);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectFollow {
    pub ticket: protocol::server::Redirect,
    pub channel: i32,
    pub address: Option<String>,
}

pub fn cache_redirect(
    scene: &mut FieldScene,
    ticket: protocol::server::Redirect,
) -> super::redirect::Cached {
    scene.redirect.cache(scene.entered.client_id, ticket)
}

pub fn redirect_after_disconnect(
    scene: &mut FieldScene,
    socket_closed: bool,
    now_ms: u32,
) -> Option<RedirectFollow> {
    let super::redirect::Closed::Follow(ticket) =
        scene.redirect.stream_failed(socket_closed, now_ms)
    else {
        return None;
    };

    super::guild::report_offline(scene);

    scene.message_panel.post(7, 0, now_ms);

    let destination =
        super::redirect::destination(&scene.servers.table, scene.session.server, &ticket);

    scene
        .session
        .adopt(scene.session.server, destination.channel);

    Some(RedirectFollow {
        ticket,
        channel: destination.channel,
        address: destination.address,
    })
}

pub fn refresh_channel_popup(scene: &mut FieldScene) {
    let FieldScene {
        panels,
        servers,
        status,
        aggregate_statuses,
        time,
        session,
        ..
    } = scene;

    let Some(panels) = panels.as_mut() else {
        return;
    };

    let rows = super::listener::aggregate_channels(
        &super::listener::AggregateInput {
            table: &servers.table,
            names: &servers.names,
            statuses: aggregate_statuses,
            strings: &panels.strings,
            w_day: time.calendar.server_day_of_month as i32,
        },
        session.server,
    )
    .unwrap_or_else(|| {
        super::listener::channels(
            &super::listener::RowInput {
                table: &servers.table,
                names: &servers.names,
                status,
                strings: &panels.strings,
                bounds: servers.bounds,
                w_day: time.calendar.server_day_of_month as i32,
                epoch_day: time.calendar.epoch_day,
            },
            session.server,
        )
    });

    let Some(screen) = panels.screen_mut(super::panels::CONTENT_MOVE_SERVER) else {
        return;
    };

    super::screens::server_popup_refresh(screen, &rows);
}

pub fn channel_popup_visibility(scene: &mut FieldScene, open: bool) {
    if !open {
        scene.aggregate_fetch.cancel();

        return;
    }

    let source_count = super::listener::aggregate_record(&scene.servers.table);

    if scene.session.server != source_count {
        scene.aggregate_fetch.cancel();

        refresh_channel_popup(scene);

        return;
    }

    let urls: Vec<String> = scene
        .servers
        .table
        .servers
        .iter()
        .take(source_count)
        .map(|server| server.status_url.clone())
        .collect();

    scene.aggregate_statuses = vec![None; source_count];

    let (_, immediate) = scene.aggregate_fetch.begin(&urls);

    if let Some(statuses) = immediate {
        scene.aggregate_statuses = statuses;
    }

    refresh_channel_popup(scene);
}

pub fn poll_channel_status(scene: &mut FieldScene) -> bool {
    let Some(statuses) = scene.aggregate_fetch.poll() else {
        return false;
    };

    let visible = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.is_open(super::panels::CONTENT_MOVE_SERVER));

    let aggregate = scene.session.server == super::listener::aggregate_record(&scene.servers.table);

    if !visible || !aggregate {
        return false;
    }

    scene.aggregate_statuses = statuses;

    refresh_channel_popup(scene);

    true
}

pub fn player_doll(
    entered: &Entered,
    effects: &Effects,
    visuals: &[mount::MountVisual],
    costume_item: u16,
) -> Option<Doll> {
    let equip = &entered.equipment;

    let look = Look::for_field(equip, effects, visuals, entered.wearer, costume_item)?;

    Some(Doll {
        look,
        pet_item: equip.items[13].index,
        position: entered.position(),
        yaw: actors::ORIGIN_YAW,
        scale: select_char::doll_scale_for(
            entered.status.stat[select_char::CON_STAT],
            equip.body().index as i16,
        ),
        hands: (
            i32::from(equip.items[7].index),
            i32::from(equip.items[6].index),
        ),
        record: PLAYER_RECORD,
        entity: entered.client_id,
        name: entered.name(),
        status: entered.status,
        name_level: entered.status.level.min(u16::from(u8::MAX)) as u8,
        level: entered.status.level,
        guild: entered.guild,
        guild_name: String::new(),
        face: i32::from(equip.body().index),
        affects: formats::net::affect::NO_AFFECTS,
        pick_action: None,
        shop_title: String::new(),
        green_name: false,
    })
}

impl FieldScene {
    pub fn resize(&mut self, chains: &crate::fonts::Chains, size: (u32, u32)) {
        let layout = Layout::new(size.0, size.1);

        self.text = super::stext::SceneText::new(chains, size.0);

        self.legacy_boxes.resize(chains, &self.layout, &layout);

        for label in [&mut self.event_hud.clock, &mut self.event_hud.counter] {
            label.x *= layout.sx / self.layout.sx;

            label.y *= layout.sy / self.layout.sy;

            label.w *= layout.sx / self.layout.sx;

            label.h *= layout.sy / self.layout.sy;
        }

        if let Some((x, y)) = &mut self.reel_position {
            *x *= layout.sx / self.layout.sx;

            *y *= layout.sy / self.layout.sy;
        }

        self.question.resize(size.0, size.1);

        self.layout = layout;

        if let Some(hud) = &mut self.hud {
            hud.reposition(size);

            hud.manager.screen = self.layout.virtual_screen();
        }

        if let Some(panels) = &mut self.panels {
            panels.reposition(size);
        }

        self.cursor = (size.0 as i32 / 2, size.1 as i32 / 2);

        self.drag = None;
    }

    pub fn target(&self) -> glam::Vec3 {
        self.player
            .and_then(|i| self.world.actors.actors.get(i))
            .map_or_else(|| self.entered.position(), |a| a.position)
    }

    pub fn pose(&mut self) -> world::Pose {
        let target = self.target();

        let ground = |x: f32, z: f32| self.world.relief_height(x, z).unwrap_or(-10_000.0);

        let player = self
            .player
            .and_then(|i| self.world.actors.actors.get(i))
            .map(|a| a.position);

        self.camera
            .view_with_player(target, self.rider(), player, &ground);

        world::Pose {
            eye: self.camera.eye,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
        }
    }

    pub fn rider(&self) -> camera::Rider {
        let Some(actor) = self.player.and_then(|i| self.world.actors.actors.get(i)) else {
            return camera::Rider::default();
        };

        camera::Rider {
            mounted: actor.mounted,
            mount_model: actor.mount_model,
            mount_kind: actor.mount_kind,
            con: self.entered.status.stat[select_char::CON_STAT],
            action: actor.action,
        }
    }

    pub fn scenic(&self) -> bool {
        let target = self.target();

        let (zx, zy) = (target.x as i32 >> ZONE_SHIFT, target.z as i32 >> ZONE_SHIFT);

        match (u8::try_from(zx), u8::try_from(zy)) {
            (Ok(zx), Ok(zy)) => world::is_special_zone(zx, zy),
            _ => false,
        }
    }

    pub fn player_yaw(&self) -> f32 {
        self.player
            .and_then(|i| self.world.actors.actors.get(i))
            .map_or(actors::ORIGIN_YAW, |a| a.yaw)
    }

    pub fn walking(&self) -> bool {
        self.player
            .and_then(|i| self.world.actors.actors.get(i))
            .is_some_and(|a| a.mover.moving)
    }

    pub fn clear_color(&self) -> wgpu::Color {
        self.world.clear_color()
    }
}

pub fn air_move_active(scene: &FieldScene) -> bool {
    scene.air_move.is_some()
}

pub fn begin_air_move(scene: &mut FieldScene, row: u32) -> bool {
    if air_move_active(scene) || scene.panels.is_none() {
        return false;
    }

    let Some(player) = scene.player else {
        return false;
    };

    let Some(flight) = AirMoveFlight::start(row as usize, scene.time.now_ms) else {
        return false;
    };

    let Some(restore) = scene.world.actors.begin_air_move(player, scene.time.now_ms) else {
        return false;
    };

    super::movement::discard_spatial_outbox(&mut scene.outbox);

    scene.air_move = Some(AirMove {
        row,
        flight,
        restore,
    });

    clear_air_move_hidden(scene);

    let raised = super::burst::departure(scene);

    tracing::info!("{}", super::burst::describe(&raised));

    if let Some(panels) = scene.panels.as_mut() {
        panels.portal.departed = Some(row);

        panels.wire.push(portal::depart(row));
    }

    true
}

pub fn finish_air_move(scene: &mut FieldScene) -> bool {
    let Some(active) = scene.air_move.take() else {
        return false;
    };

    finish_air_move_taken(scene, active)
}

fn finish_air_move_taken(scene: &mut FieldScene, active: AirMove) -> bool {
    if !scene
        .world
        .actors
        .finish_air_move(active.restore, scene.time.now_ms)
    {
        if let Some(panels) = scene.panels.as_mut() {
            panels.portal.departed = None;
        }

        return false;
    }

    let raised = super::burst::arrival(scene);

    tracing::info!("{}", super::burst::describe(&raised));

    if let Some(panels) = scene.panels.as_mut() {
        panels.portal.departed = None;

        panels.wire.push(portal::arrive(active.row));
    }

    true
}

fn clear_air_move_hidden(scene: &mut FieldScene) {
    if let Some(actor) = scene
        .player
        .and_then(|index| scene.world.actors.actors.get_mut(index))
    {
        actor.status.flags &= !1;
    }
}

fn tick_air_move(scene: &mut FieldScene) {
    let Some(mut active) = scene.air_move.take() else {
        return;
    };

    clear_air_move_hidden(scene);

    let Some(mut pose) = scene.world.actors.air_move_pose() else {
        if let Some(panels) = scene.panels.as_mut() {
            panels.portal.departed = None;
        }

        return;
    };

    let step = active.flight.tick(
        scene.time.now_ms,
        &mut pose,
        &mut scene.world.actors.effect_rng,
    );

    if !scene.world.actors.apply_air_move_pose(pose) {
        if let Some(panels) = scene.panels.as_mut() {
            panels.portal.departed = None;
        }

        return;
    }

    match step {
        AirMoveTick::Active => scene.air_move = Some(active),
        AirMoveTick::Teardown => {
            finish_air_move_taken(scene, active);
        }
    }
}

fn advance_bodies(scene: &mut FieldScene) {
    if scene.world.actors.ticked_at(scene.time.now_ms) {
        return;
    }

    let viewer_was_walking = scene.player.is_some_and(|index| {
        matches!(
            scene.world.actors.actors[index].action,
            appearance::ACTION_WALK | appearance::ACTION_RUN
        )
    });

    let tick_reach = scene.world.pose_reach();

    if scene
        .world
        .actors
        .tick_with_reach(scene.time.now_ms, tick_reach)
    {
        scene.attack.approach_target = None;

        if viewer_was_walking {
            let _ = attack::settled_tick(scene, scene.time.now_ms);
        }
    }

    if scene.world.actors.take_viewer_death_landed() {
        let unsafe_cell = scene
            .player
            .and_then(|index| scene.world.actors.actors.get(index))
            .map(attack::cell_of)
            .is_some_and(|cell| !attack::safe_here(scene.world.actors.attributes(), cell));

        if unsafe_cell {
            raise_revive_box(scene.panels.as_mut(), &super::pending::death_box());
        }
    }

    tick_air_move(scene);

    for entity in super::vanish::reap_idle(scene) {
        tracing::info!(entity, "NWYD: remote actor completed its departure");
    }

    for reaped in corpse::reap(scene) {
        tracing::info!("{}", corpse::describe(&reaped));
    }
}

pub fn world_frame(scene: &mut FieldScene, backbuffer: (u32, u32)) -> WorldFrame {
    super::impact_scene::tick(scene, scene.time.now_ms);

    scene.world.actors.tree_kingdom = scene.siege.zone_kingdom;

    drag_step(scene);

    right_step(scene);

    advance_bodies(scene);

    if scene.projected.is_some() {
        scene.hovered = None;
    }

    hover::update(scene);

    hover_drops(scene);

    hover_gates(scene);

    super::impact_delayed::tick(scene, scene.time.now_ms);

    super::siege_effects::update(scene, scene.time.now_ms);

    scene.camera.tick_shake(scene.time.now_ms);

    let pose = scene.pose();

    scene.world.camera_distance = scene.camera.dist;

    scene.world.weather_target = scene.player.map(|i| scene.world.actors.actors[i].position);

    let mut frame = scene.world.frame_at(pose, scene.time.now_ms, backbuffer);

    if let Some(wave) = scene.ground_wave {
        let zone = scene.world.actors.zone;

        if let Some(uniforms) = wave.uniforms(
            scene.time.now_ms,
            [zone.0 as f32 * 128.0, zone.1 as f32 * 128.0],
        ) {
            frame.uniforms.ground_wave = uniforms;
        } else {
            scene.ground_wave = None;
        }
    }

    scene.projected = Some((frame.view.matrix(), frame.projection.matrix()));

    frame
}

pub fn camera_cycle(scene: &mut FieldScene) {
    if !scene.lock.allows(scene.time.now_ms) || scene.camera.shake.is_some() {
        return;
    }

    scene.camera_view = camera::next_view(scene.camera_view, scene.low_ceiling);

    scene.camera.set_view(scene.camera_view, scene.scenic());
}

pub fn camera_input(scene: &mut FieldScene, mouse: camera::MouseInput) {
    let rider = scene.rider();

    scene.camera.update(mouse, rider);
}

pub fn click(scene: &mut FieldScene, x: i32, y: i32, shift: bool) -> Option<ClickMarker> {
    world_input_admission(scene).ok()?;

    let hit = pick_ground(scene, x, y)?;

    let zone = scene.world.actors.zone;

    let floor = formats::world::path::HeightField::window(&scene.world.actors.window, zone)
        .ground(hit.x as i32, hit.z as i32);

    let tile = (hit.x as i32, hit.z as i32);

    let answered = hit.y > ground::MISS_FLOOR;

    let swung = attack::dispatch(scene, shift, scene.time.now_ms);

    if let Some(line) = attack::describe(&swung) {
        tracing::info!("{line}");
    }

    let acted = answered && !shift && !swung.consumed() && !consumed_by_entity(scene);

    let walking = acted && ground_order(scene, tile);

    let marker = ClickMarker {
        at: Vec3::new(hit.x, floor + MARKER_LIFT, hit.z),
        tile,
        walking,
    };

    scene.marker = Some(marker);

    scene.drag = Some(Drag {
        at: (x, y),
        tile,
        ground: acted,
        suspended: false,
        retry: !walking,
    });

    Some(marker)
}

fn ground_order(scene: &mut FieldScene, tile: (i32, i32)) -> bool {
    let reachable = !scene.combat.plans_a_path()
        || super::movement::preview(scene, tile).is_some_and(|p| !p.dirs.is_empty());

    let y = height_of_move_marker(scene, tile);

    let result = cc::walk(
        scene,
        (tile.0 as f32, y, tile.1 as f32),
        scene.time.now_ms,
        cc::WALK_CLICK,
    );

    let cc::Walked::Ordered { walking, .. } = result else {
        return false;
    };

    if scene.combat.plans_a_path() {
        let far = scene.player.is_some_and(|index| {
            let at = scene.world.actors.actors[index].position;

            let from = (at.x as i32, at.z as i32);

            formats::world::path::tile_distance(from, tile) >= ANCHOR_KEEP_RANGE
        });

        let unreachable = !reachable;

        if far || unreachable {
            reanchor(scene);
        }
    }

    walking
}

pub fn move_to(scene: &mut FieldScene, tile: (i32, i32)) -> Result<(i32, i32), &'static str> {
    if !(0..4096).contains(&tile.0) || !(0..4096).contains(&tile.1) {
        return Err("coordinates_out_of_range");
    }

    let index = move_admission(scene)?;

    let actor = &scene.world.actors.actors[index];

    let zone = scene.world.actors.zone;

    let local = (
        tile.0 - i32::from(zone.0) * 128,
        tile.1 - i32::from(zone.1) * 128,
    );

    if !(0..256).contains(&local.0) || !(0..256).contains(&local.1) {
        return Err("destination_outside_loaded_terrain");
    }

    if attack::cell_of(actor) == tile {
        return Err("already_at_destination");
    }

    if actor.mover.last_dest == tile {
        return Err("already_heading_there");
    }

    let Some(preview) = super::movement::preview(scene, tile) else {
        return Err("no_path");
    };

    if preview.dirs.is_empty() {
        return Err("no_path");
    }

    let (points, count) = formats::world::path::expand(attack::cell_of(actor), &preview.dirs);

    if points[..count].iter().any(|p| {
        !scene
            .world
            .holds(((p[0] as i32 / 128) as u8, (p[1] as i32 / 128) as u8))
    }) {
        return Err("terrain_unavailable");
    }

    if scene.sent_dest == Some(preview.reached) {
        return Err("no_progress");
    }

    ground_order(scene, tile);

    if scene.cc.auto_move != tile && scene.cc.auto_move != preview.reached {
        return Err("movement_cooldown");
    }

    let floor = height_of_move_marker(scene, tile);

    scene.marker = Some(ClickMarker {
        at: Vec3::new(
            tile.0 as f32 + 0.5,
            floor + MARKER_LIFT,
            tile.1 as f32 + 0.5,
        ),
        tile,
        walking: true,
    });

    scene.drag = None;

    Ok(preview.reached)
}

fn height_of_move_marker(scene: &FieldScene, tile: (i32, i32)) -> f32 {
    formats::world::path::HeightField::window(&scene.world.actors.window, scene.world.actors.zone)
        .ground(tile.0, tile.1)
}

pub(super) fn move_admission(scene: &FieldScene) -> Result<usize, &'static str> {
    let index = world_input_admission(scene)?;

    let actor = &scene.world.actors.actors[index];

    if attack::chase_block(actor) != 0 {
        return Err("movement_blocked");
    }

    if attack::action_busy(actor, &scene.world.actors.models, scene.time.now_ms) {
        return Err("action_busy");
    }

    Ok(index)
}

fn world_input_admission(scene: &FieldScene) -> Result<usize, &'static str> {
    if scene.world.force_load.is_some() {
        return Err("destination_loading");
    }

    let index = scene
        .player
        .filter(|&i| i < scene.world.actors.actors.len())
        .ok_or("player_unavailable")?;

    let actor = &scene.world.actors.actors[index];

    if scene.entered.status.hp < 1 || actor.is_dead {
        return Err("player_dead");
    }

    if air_move_active(scene) || actor.mover.server_driven {
        return Err("server_controlled_movement");
    }

    if scene.exit.is_some() || !scene.lock.allows(scene.time.now_ms) {
        return Err("input_locked");
    }

    if scene
        .panels
        .as_ref()
        .is_some_and(|p| p.boxes.topmost().is_some())
    {
        return Err("modal_dialog");
    }

    if scene
        .panels
        .as_ref()
        .is_some_and(|p| p.is_open(cc::WALK_PANEL))
    {
        return Err("warehouse_open");
    }

    if scene
        .hud
        .as_ref()
        .is_some_and(|h| h.manager.carried.is_some())
    {
        return Err("carrying_item");
    }

    if scene
        .panels
        .as_ref()
        .and_then(|p| p.store.as_ref())
        .is_some_and(|s| s.owner == scene.entered.client_id)
    {
        return Err("own_store_open");
    }

    Ok(index)
}

pub fn consumed_by_entity(scene: &FieldScene) -> bool {
    let store_open = scene
        .panels
        .as_ref()
        .is_some_and(|screens| screens.is_open(panels::CONTENT_STORE));

    let target = scene
        .hovered
        .and_then(|entity| scene.world.actors.entity(entity))
        .map(|index| &scene.world.actors.actors[index]);

    interact::consumes_a_click(store_open, target, interact::Where::of(scene))
}

fn pick_ground(scene: &mut FieldScene, x: i32, y: i32) -> Option<Vec3> {
    let (view, projection) = scene.projected?;

    let viewport = (scene.layout.width, scene.layout.height);

    let mut ray = Ray::unproject(&view, &projection, (x, y), viewport);

    ray.direction = ray.direction.normalize_or_zero();

    let target = scene.target();

    let radius = ground::radius(scene.camera.pitch, scene.camera.dist);

    let current = scene.world.current;

    let actors = &scene.world.actors;

    let block = |zone: (u8, u8)| {
        let (zone, heights) = actors.zone_heights.iter().find(|(at, _)| *at == zone)?;

        Some(ground::Block {
            zone: *zone,
            heights,
        })
    };

    let here = block(current)?;

    let neighbours = world::Direction::ALL
        .map(|dir| world::neighbour_of(current, dir).and_then(|(zone, _)| block(zone)));

    let terrains = scene.world.terrains();

    let coarse = ground::Coarse {
        terrains: &terrains,
        view: glam::Mat4::from_cols_array(&view.0),
        projection: glam::Mat4::from_cols_array(&projection.0),
        viewport,
        automatic_movement: scene.automatic_movement,
    };

    Some(scene.picker.pick_with_coarse(
        here,
        &neighbours,
        &ray,
        target,
        radius,
        scene.camera.pitch,
        Some(&coarse),
    ))
}

pub fn cursor_ground(scene: &mut FieldScene) -> Vec3 {
    let (x, y) = scene.cursor;

    pick_ground(scene, x, y).unwrap_or(ground::MISS)
}

pub fn drag_step(scene: &mut FieldScene) {
    drag_aim(scene);
}

pub fn right_step(scene: &mut FieldScene) {
    let now_ms = scene.time.now_ms;

    if !scene.right_held
        || scene.right_suspended
        || world_input_admission(scene).is_err()
        || now_ms.wrapping_sub(scene.last_interact_ms) < interact::INTERACT_INTERVAL_MS
        || scene
            .player
            .is_some_and(|i| scene.world.actors.actors[i].action == appearance::ACTION_DEAD)
        || shift_player_whisper(scene, now_ms)
        || ctrl_held(scene)
    {
        return;
    }

    let at = cursor_ground(scene);

    let approach = !shift_held(scene);

    let outcome = cast::cast(scene, (at.x, at.y, at.z), now_ms, approach, None);

    if scene.combat.mode == cc::CC_MODE_PHYSICAL && !outcome.answer() {
        scene.cc.queued = Some(cc::Queued {
            at: (at.x, at.y, at.z),
            approach,
            target: scene.hovered,
        });
    }

    if !matches!(outcome, cast::Cast::Refused(_) | cast::Cast::Talked(_))
        && let Some(line) = outcome.describe()
    {
        tracing::info!("{line}");
    }
}

pub fn drag_moved(scene: &mut FieldScene) {
    drag_aim(scene);
}

fn drag_aim(scene: &mut FieldScene) {
    let Some(drag) = scene.drag else {
        return;
    };

    if drag.suspended || world_input_admission(scene).is_err() {
        return;
    }

    let shift = shift_held(scene);

    if !drag.ground || shift {
        let swung = attack::dispatch(scene, shift, scene.time.now_ms);

        if swung.consumed() || consumed_by_entity(scene) {
            if let Some(held) = scene.drag.as_mut() {
                held.retry = true;
            }

            return;
        }
    }

    if shift || move_admission(scene).is_err() {
        if let Some(held) = scene.drag.as_mut() {
            held.retry = true;
        }

        return;
    }

    let Some(hit) = pick_ground(scene, drag.at.0, drag.at.1) else {
        return;
    };

    if hit.y <= ground::MISS_FLOOR {
        return;
    }

    let tile = (hit.x as i32, hit.z as i32);

    let floor = height_of_move_marker(scene, tile);

    let at = Vec3::new(hit.x, floor + MARKER_LIFT, hit.z);

    let walking = scene.marker.is_some_and(|marker| marker.walking);

    scene.marker = Some(ClickMarker { at, tile, walking });

    if tile == drag.tile && !drag.retry && scene.cc.auto_move == tile {
        return;
    }

    scene.drag = Some(Drag {
        tile,
        ground: true,
        retry: false,
        ..drag
    });

    let walking = ground_order(scene, tile);

    if let Some(held) = scene.drag.as_mut() {
        held.retry = scene.cc.auto_move != tile;
    }

    if let Some(marker) = scene.marker.as_mut() {
        marker.walking = walking;
    }
}

pub fn cancel_pointer_hold(scene: &mut FieldScene) {
    scene.drag = None;

    scene.right_held = false;

    scene.right_suspended = false;

    if let Some(panels) = scene.panels.as_mut() {
        panels.spend_repeat.cancel();
    }
}

pub fn order(scene: &mut FieldScene, tile: (i32, i32)) -> bool {
    super::movement::order(scene, tile)
}

pub fn numpad(scene: &mut FieldScene, vk: u8) -> super::emote::Press {
    if !scene.lock.allows(scene.time.now_ms) {
        return super::emote::Press::Refused(super::emote::Refusal::Locked);
    }

    let caret = scene_caret(scene);

    let press = super::emote::press(scene, vk, caret, CONTROL_SCHEME_DEFAULT);

    if let Some(line) = super::emote::describe_press(vk, &press) {
        tracing::info!("{line}");
    }

    press
}

pub fn recall(scene: &mut FieldScene) -> super::recall::Outcome {
    let now_ms = scene.time.now_ms;

    let move_alpha = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map(|actor| actor.mover.move_alpha);

    if let Err(refusal) = super::recall::admit(
        &scene.lock,
        now_ms,
        scene.emote_keys.floor_ms,
        air_move_active(scene),
        move_alpha,
    ) {
        return super::recall::Outcome::Refused(refusal);
    }

    let slot = {
        let effects = scene.statics.effects();

        super::recall::first_slot(&scene.entered.carry, &effects)
    };

    let Some(slot) = slot else {
        return super::recall::Outcome::NoScroll;
    };

    let Some(actor) = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
    else {
        return super::recall::Outcome::MissingBody { slot };
    };

    let at = (
        super::recall::truncate_coordinate_to_u16(actor.position.x),
        super::recall::truncate_coordinate_to_u16(actor.position.z),
    );

    let staged = item_move::Request::Use { slot, at };

    scene.cc.last_action_ms = now_ms;

    scene.lock.stamp(super::pending::STAMP_SLOT, now_ms);

    scene.pending.armed = true;

    scene
        .outbox
        .push(portal::move_request(scene.entered.client_id));

    scene.pending.frame = Some(staged);

    let spent = super::recall::spend(&mut scene.entered.carry, slot)
        .expect("first_slot always returns a live native inventory coordinate");

    if let Some(panels) = scene.panels.as_mut() {
        let Panels {
            screens,
            items,
            icons,
            strings,
            ..
        } = panels;

        if let Some(screen) = screens
            .iter_mut()
            .find(|screen| screen.content == panels::CONTENT_INVENTORY)
        {
            screens::inventory_refresh(screen, &scene.entered, items, icons, strings);
        }

        panels::refresh_inventory_hover(panels, &scene.entered, slot);

        panels::refine_item_changed(panels, &scene.entered);
    }

    scene
        .unconditional_sounds
        .push(super::audio::UnconditionalSound::new(
            super::item_use::SOUND_TELEPORT,
        ));

    if let Some(panels) = scene.panels.as_mut() {
        screens::refresh_all(panels, &scene.entered);
    }

    scene.emote_keys.floor_ms = now_ms;

    super::recall::Outcome::Used(spent)
}

pub fn refresh_screens(scene: &mut FieldScene) {
    if (scene.entered.status.hp as i32) > (scene.entered.status.max_hp as i32) {
        scene.entered.status.hp = scene.entered.status.max_hp;
    }

    scene.entered.refresh_class();

    if let Some(panels) = scene.panels.as_mut() {
        screens::refresh_all(panels, &scene.entered);
    }
}

pub fn open_reel(scene: &mut FieldScene, item: i16) -> bool {
    if scene.reel.visible || !matches!(item, super::reel::TALK_A | super::reel::TALK_B) {
        return false;
    }

    if let Some(panels) = scene.panels.as_mut() {
        panels.close_by_toggle(panels::CONTENT_COIN_INPUT);

        panels.close_by_toggle(panels::CONTENT_SKILL_MASTER);

        panels.close_by_toggle(panels::CONTENT_SKILL);

        panels.close_by_toggle(panels::CONTENT_CHARACTER);

        if panels.is_open(panels::CONTENT_CARGO) {
            panels.close_by_toggle(panels::CONTENT_CARGO);

            panels.close_by_toggle(panels::CONTENT_INVENTORY);
        }
    }

    if super::store::close(scene).wire() {
        scene.outbox.push(super::store::close_frame());
    }

    if let Some(panels) = scene.panels.as_mut() {
        panels.close_by_toggle(panels::CONTENT_INVENTORY);

        super::shop::close(panels);
    }

    scene.reel.open_talk(item)
}

pub fn close_reel(scene: &mut FieldScene) -> bool {
    if !scene.reel.close() {
        return false;
    }

    scene.reel_drag_capture = false;

    if let Some(panels) = scene.panels.as_mut() {
        panels.close_by_toggle(panels::CONTENT_COIN_INPUT);
    }

    true
}

fn reel_sound(scene: &mut FieldScene, slot: i64) {
    scene
        .unconditional_sounds
        .push(super::audio::UnconditionalSound::new(slot));
}

fn reel_start(scene: &mut FieldScene, now_ms: u32) -> super::reel::Start {
    let has_room = super::item_use::free_carry_slot(&scene.entered, 0, true) >= 0;

    let started = scene.reel.start(
        scene.entered.client_id,
        scene.entered.gold,
        has_room,
        now_ms,
    );

    match &started {
        super::reel::Start::Sent { frame, debit } => {
            scene.outbox.push(frame.clone());

            scene.entered.gold = scene.entered.gold.wrapping_sub(*debit);

            reel_sound(scene, super::reel::SOUND_SPIN);

            refresh_screens(scene);
        }
        super::reel::Start::NoRoom => {
            if let Some(panels) = scene.panels.as_mut() {
                let text = panels
                    .strings
                    .translate(&format!("ss_{}", super::shop::BAG_FULL_STRDEF));

                super::chat::system(panels, &text, super::shop::BAG_FULL_COLOR);
            }

            reel_sound(scene, super::reel::SOUND_NO_ROOM);
        }
        super::reel::Start::Busy | super::reel::Start::NoGold => {}
    }

    started
}

fn reel_tick(scene: &mut FieldScene, now_ms: u32) {
    let Some(pending) = scene.reel.begin_tick(now_ms, scene.attack.auto) else {
        return;
    };

    if pending.auto_start {
        reel_command(scene, super::reel::START, now_ms);
    }

    let ticked = scene.reel.finish_tick(pending);

    if ticked.controller.settled {
        scene.entered.gold = scene
            .entered
            .gold
            .wrapping_add(ticked.controller.gold_delta as u32);

        refresh_screens(scene);

        if let Some(banner) = ticked.controller.banner {
            let suffix = scene
                .message_strings
                .strdef
                .as_ref()
                .map_or("", |strings| strings.get(284));

            let text = banner.text(suffix);

            tracing::info!("NWYD: reel result — {text}");

            if scene
                .message_panel
                .post_text(text.clone(), super::reel::BANNER_MS, now_ms)
            {
                reel_sound(scene, super::notice::SOUND_SLOT);
            }

            scene.reel.post_banner(text, now_ms);

            reel_sound(scene, super::reel::SOUND_RESULT);
        }
    }

    for _ in 0..ticked.controller.stop_sounds {
        reel_sound(scene, super::reel::SOUND_STOP);
    }
}

fn scene_caret(_scene: &FieldScene) -> Option<i32> {
    None
}

pub const CONTROL_SCHEME_DEFAULT: i16 =
    formats::config::config_bin::DEFAULTS[formats::config::config_bin::slot::CONTROL_SCHEME];

pub const MOVE_SEND_INTERVAL_MS: u32 = 1000;

pub fn redirect(scene: &mut FieldScene, tile: (i32, i32)) -> bool {
    ground_order(scene, tile)
}

pub fn upload(scene: &FieldScene, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.upload(gpu, renderer);

    renderer.upload_world(gpu, &scene.world);

    super::sun::upload(&scene.world.effects, gpu, renderer);

    super::screen_weather::upload(&scene.world.effects, gpu, renderer);

    if let Some(hud) = &scene.hud {
        hud::upload(hud, gpu, renderer);
    }

    if let Some(screens) = &scene.panels {
        panels::upload(screens, gpu, renderer);
    }

    super::minipopup::upload(&scene.minipopup_icons, gpu, renderer);

    for (slot, loaded) in &scene.chrome {
        renderer.upload(
            gpu,
            TextureKey::Scene(*slot),
            &loaded.texture,
            loaded.alpha,
            &format!("{} (slot {slot})", loaded.stored_path),
        );
    }

    for (slot, loaded) in &scene.cape_flag_textures {
        renderer.upload_once(
            gpu,
            TextureKey::ExternalUi(*slot),
            &loaded.texture,
            loaded.alpha,
            &format!("{} (character-sheet flag slot {slot})", loaded.stored_path),
        );
    }

    renderer.upload(
        gpu,
        TextureKey::Scene(minimap::DYNAMIC_SLOT),
        &scene.minimap.composite,
        render::texture::AlphaPolicy::Stored,
        "minimap composite (dynamic texture 4)",
    );
}

pub fn sync(scene: &mut FieldScene, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.sync(gpu, renderer);

    renderer.sync_scene_text(gpu, &scene.text);

    renderer.sync_guild_marks(gpu, &scene.marks);

    if let Some(hud) = &mut scene.hud {
        hud::sync(hud, gpu, renderer);
    }

    if let Some(screens) = &mut scene.panels {
        panels::sync(screens, gpu, renderer);
    }
}

fn sync_affect_tooltip_clock(scene: &mut FieldScene) {
    if let Some(panels) = scene.panels.as_mut() {
        panels.tooltip_calendar = scene.time.calendar;

        panels.now_ms = scene.time.now_ms;

        panels.sealed_records = scene.records.clone();
    }

    if let Some(hud) = scene.hud.as_mut() {
        hud.affect_time = scene.affect_time;

        hud.affect_raw_ms = scene.time.raw_ms;

        hud.affect_game_ms = scene.time.now_ms;

        hud.affect_stamps = scene.affect_stamps;

        hud.affect_live_times = scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .map_or([0; 32], |actor| actor.details.map(|detail| detail.time_8s));
    }
}

pub fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    append_frame_draws(scene, &mut out);

    out
}

pub(crate) fn append_frame_draws(scene: &mut FieldScene, out: &mut Vec<PlannedQuad>) {
    sync_affect_tooltip_clock(scene);

    super::firecracker::drain_open(scene);

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    if let Some(panels) = scene.panels.as_mut() {
        panels.tooltip.configure_preview(
            scene.interface.item_3d_preview,
            &scene.world.objects.registries,
            (scene.layout.width, scene.layout.height),
            scene.time.now_ms,
        );

        screens::refresh_opened(panels, &scene.entered);

        super::merge_item::refresh(panels, &scene.entered);
    }

    if scene
        .panels
        .as_mut()
        .is_some_and(|p| std::mem::take(&mut p.sheet_live_pending) && p.field_close_owner_present)
    {
        refresh_sheet_magic(scene);
    }

    out.extend(super::sun::frame_draws(
        &mut scene.world,
        (scene.layout.width, scene.layout.height),
    ));

    out.extend(scene.screen_weather.quads(
        &scene.layout,
        scene.world.actors.effects_detail.effects,
        &scene.world.effects,
    ));

    if let Some(blind) = scene.blind {
        let colour = blind.colour(scene.time.now_ms);

        if colour != 0 {
            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::White,
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: 0.0,
                    dst_y: 0.0,
                    dst_w: scene.layout.width as f32,
                    dst_h: scene.layout.height as f32,
                    src_x: 0.0,
                    src_y: 0.0,
                    src_w: 1.0,
                    src_h: 1.0,
                    color: colour,
                    rotation: 0.0,
                },
                uv: UvRule::TexelCentre,
            });
        }
    }

    out.extend(event_hud_draws(scene));

    out.extend(map_draws(scene));

    out.extend(
        scene
            .damage
            .frame_draws(&scene.sets, &scene.layout, &mut scene.text),
    );

    out.extend(nameplate::frame_draws(scene));

    out.extend(super::drop_presentation::frame_draws(scene));

    out.extend(super::message_panel::frame_draws(scene));

    out.extend(reel_draws(scene));

    out.extend(super::firecracker::frame_draws(scene));

    out.extend(question_draws(scene));

    out.extend(long_message_draws(scene));

    out.extend(super::arena_draw::frame_draws(scene));

    let (plate, experience, combat) = (plate(scene), experience(scene), scene.combat);

    let target = hud::TargetPlate::select(&scene.world.actors.actors, scene.player);

    let target_affects = scene.world.actors.target_affects;

    let hold = has_cp(scene);

    let pk = scene.pk;

    let armed = scene.attack.auto;

    let auto_skills = usize::try_from(scene.skill_bar.auto_count).unwrap_or(0);

    let details = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map_or(formats::net::affect::NO_DETAILS, |actor| actor.details);

    let cooldown_affected = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .is_some_and(cast::cooldown_affected);

    let cooldown_refined = cast::cooldown_refined(scene);

    let (stamps, now_ms) = (scene.affect_stamps, scene.time.now_ms);

    let evolution = cc::evolution_tier(scene);

    let party_has_player = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_some();

    let arena_visible = scene.arena.visible;

    let FieldScene {
        hud,
        panels: screens,
        entered,
        statics,
        cc: macros,
        quick_bar,
        party,
        world,
        ..
    } = scene;

    if party_has_player
        && let Some(screens) = screens.as_mut()
        && let Some(screen) = screens.screen_mut(panels::CONTENT_PARTY)
    {
        super::screens::party_live_refresh(screen, party, &world.actors, arena_visible);
    }

    if let (Some(hud), Some(screens)) = (hud.as_mut(), screens.as_mut()) {
        chat::drain_toasts(screens, &mut hud.toasts, hud::TOAST_COLOR);

        chat::frame(screens);

        super::minipopup::frame(screens, now_ms);
    }

    if let Some(hud) = hud {
        if hud.bar.ui.panel.visible {
            if party_has_player {
                hud.prepare_target_refresh();
            }

            hud.refresh_target(target.as_ref(), &target_affects);
        }

        if party_has_player && hud.plate.ui.panel.visible {
            hud.prepare_plate_refresh();
        }

        hud.refresh_plate(&plate);

        hud.refresh_experience(&experience, hold);

        hud.refresh_buffs(&details, &stamps, now_ms);

        hud.refresh_flags(pk, armed, auto_skills);

        hud.refresh_combat(&combat);

        if let Some(screens) = screens.as_mut() {
            panels::refresh_combat_hover(hud, screens);
        }

        hud.refresh_skills(&entered.quick, &statics.items);

        hud.refresh_quick(
            &quick_bar.ids,
            &entered.carry,
            &statics.items,
            &statics.icons,
        );

        hud.refresh_cooldowns(
            &entered.quick,
            statics.skills.as_ref(),
            &macros.skill_stamps,
            cooldown_refined,
            cooldown_affected,
            evolution,
            entered.status.level,
            now_ms,
        );

        if let Some(screens) = screens {
            panels::refresh_icon_hover(hud, screens, entered);

            let offset = out.len();

            out.extend(panels::field_frame_draws(hud, screens));

            screens.tooltip.offset_preview(offset);
        } else {
            out.extend(hud::frame_draws(hud));
        }
    }

    out.extend(scene.legacy_boxes.frame_draws(
        &scene.layout,
        &scene.message_strings,
        scene.captions,
    ));
}

fn reel_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    if !scene.reel.visible {
        return Vec::new();
    }

    let Some(parent) = reel_parent_rect(scene) else {
        return Vec::new();
    };

    let mut out = child_quad_at(scene, parent, super::reel::PANEL, 0.0)
        .into_iter()
        .collect::<Vec<_>>();

    if let Some(record) =
        panel(&scene.ui, 0x1911).filter(|r| r.header.parent_id == super::reel::PANEL)
        && let Some([x, y, w, h]) = record.rect()
        && let Some(item) = set_item(&scene.sets, record)
        && let Ok(slot) = u32::try_from(item.texture_index)
    {
        let mut rect = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

        rect.x += parent.x;

        rect.y += parent.y;

        if on_screen(&scene.layout, rect) {
            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::Scene(slot),
                quad: textured(rect, item, record.fields[5] as u32, 0.0),
                uv: UvRule::TexelCentre,
            });
        }
    }

    for command in super::reel::BUTTONS.into_iter().rev() {
        let Some([x, y, w, h]) = control_rect(&scene.ui, u32::from(command)) else {
            continue;
        };

        let local = scene
            .layout
            .control(x as f32 + 1.0, y as f32 - 1.0, w as f32, h as f32);

        let rect = Rect {
            x: parent.x + local.x,
            y: parent.y + local.y,
            w: local.w,
            h: local.h,
        };

        if on_screen(&scene.layout, rect)
            && let Some(record) = scene.ui.records.iter().find(|r| {
                r.header.name_id == u32::from(command)
                    && r.header.parent_id == super::reel::PANEL
                    && r.control == ControlType::Button
            })
            && let Some(set) = record.control_id().and_then(|id| usize::try_from(id).ok())
            && let Some(frame) = scene.reel.button_frame(command)
        {
            out.extend(reel_set_quad(scene, set, frame, rect));
        }
    }

    if scene.captions
        && let Some(command) = scene.reel.hovered
    {
        let id = u32::from(command);

        if let Some(record) = scene.ui.records.iter().find(|r| r.header.name_id == id)
            && let Some([x, y, w, h]) = record.rect()
            && let Some(label) = record
                .fields
                .get(7)
                .copied()
                .filter(|v| (0..=500).contains(v))
        {
            let value = super::stext::display_text(
                &scene.message_strings.translate(&format!("uu_{label}")),
            );

            let key = super::reel::TOOLTIP_TEXT_BASE + id;

            if scene.text.resolve(key, &value, 0xffffffff)
                && let Some(line) = scene.text.get(key)
            {
                let width = (line.rect[2] - line.rect[0]) as f32;

                let at = (
                    (parent.x + (x + w - 1) as f32 * scene.layout.sx)
                        .min(scene.layout.width as f32 - width)
                        .max(0.0),
                    parent.y
                        + (y - 1) as f32 * scene.layout.sy
                        + (h as f32 - super::stext::LINE_BOX) * scene.layout.sy * 0.5,
                );

                out.push(PlannedQuad {
                    mode: render::preset::RenderMode::Ui2dPoint,
                    texture: TextureKey::SceneText(key),
                    quad: super::stext::shadow_quad(line, at),
                    uv: UvRule::TexelCentre,
                });

                out.push(PlannedQuad {
                    mode: S_MODE,
                    texture: TextureKey::SceneText(key),
                    quad: super::stext::quad(line, at),
                    uv: UvRule::TexelCentre,
                });
            }
        }
    }

    if scene.captions {
        let labels: Vec<_> = scene
            .ui
            .records
            .iter()
            .filter(|r| {
                r.header.parent_id == super::reel::PANEL
                    && r.control == ControlType::Text
                    && r.header.name_id != super::reel::MONEY
                    && r.header.name_id != super::reel::BET
            })
            .filter_map(|r| {
                r.fields
                    .get(10)
                    .copied()
                    .filter(|id| (0..=500).contains(id))
                    .map(|label| (r.header.name_id, label))
            })
            .collect();

        for (id, label) in labels.into_iter().rev() {
            let raw = scene
                .message_strings
                .ui
                .as_ref()
                .map(|table| table.get(label as usize))
                .unwrap_or_default();

            let value = super::stext::display_text(raw);

            if !value.is_empty() {
                out.extend(reel_text_quad(scene, id, &value, parent));
            }
        }
    }

    let selected = scene.reel.selected;

    if !scene.reel.controller_visible[selected] {
        return out;
    }

    let controller = &scene.reel.controllers[selected];

    let origin = (
        parent.x + scene.layout.sx * super::reel::CONTROLLER_AT.0,
        parent.y + scene.layout.sy * super::reel::CONTROLLER_AT.1,
    );

    out.push(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::White,
        uv: UvRule::TexelCentre,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: origin.0,
            dst_y: origin.1,
            dst_w: scene.layout.sx * 186.0,
            dst_h: scene.layout.sy * 180.0,
            src_x: 0.0,
            src_y: 0.0,
            src_w: 1.0,
            src_h: 1.0,
            color: 0x01010101,
            rotation: 0.0,
        },
    });

    for reel in 0..3 {
        for (cell, item) in super::reel::REEL_OFFSETS[reel].iter().copied().enumerate() {
            let Some(y) = controller.cell_y(reel, cell) else {
                continue;
            };

            let rect = Rect {
                x: origin.0 + scene.layout.sx * (reel as f32 * 63.0),
                y: origin.1 + scene.layout.sy * y as f32,
                w: scene.layout.sx * super::reel::CELL as f32,
                h: scene.layout.sy * super::reel::CELL as f32,
            };

            out.extend(reel_set_quad(
                scene,
                super::reel::BASE_SETS[selected] + item,
                0,
                rect,
            ));
        }
    }

    for index in 0..9 {
        let rect = Rect {
            x: origin.0 + scene.layout.sx * ((index % 3) as f32 * 63.0),
            y: origin.1 + scene.layout.sy * ((index / 3) as f32 * 62.0),
            w: scene.layout.sx * 62.0,
            h: scene.layout.sy * 62.0,
        };

        for (visible, set) in [
            (controller.primary[index], super::reel::MARKER_SETS[0]),
            (controller.alternate[index], super::reel::MARKER_SETS[1]),
        ] {
            if visible {
                out.extend(reel_set_quad(scene, set, 0, rect));
            }
        }
    }

    out.extend(reel_set_quad(
        scene,
        super::reel::COUNTER_FRAME_SET,
        0,
        Rect {
            x: origin.0 + scene.layout.sx * 30.0,
            y: origin.1 - scene.layout.sy * 50.0,
            w: scene.layout.sx * 160.0,
            h: scene.layout.sy * 28.0,
        },
    ));

    for (index, digit) in controller.digits().into_iter().enumerate() {
        let rect = Rect {
            x: origin.0 + scene.layout.sx * (41.0 + index as f32 * 15.0),
            y: origin.1 - scene.layout.sy * 49.0,
            w: scene.layout.sx * 14.0,
            h: scene.layout.sy * 25.0,
        };

        out.extend(reel_set_quad(scene, super::reel::DIGIT_SET, digit, rect));
    }

    let money = (scene.entered.gold as i32).wrapping_sub(scene.reel.wager);

    out.extend(reel_text_quad(
        scene,
        super::reel::BET,
        &format!("{:6}", scene.reel.wager),
        parent,
    ));

    out.extend(reel_text_quad(
        scene,
        super::reel::MONEY,
        &super::reel::money_text(money),
        parent,
    ));

    out
}

fn reel_set_quad(scene: &FieldScene, set: usize, item: usize, rect: Rect) -> Option<PlannedQuad> {
    let item = scene.sets.item(set, item)?;

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::Scene(u32::try_from(item.texture_index).ok()?),
        quad: textured(rect, item, 0xFFFF_FFFF, 0.0),
        uv: UvRule::TexelCentre,
    })
}

fn reel_text_quad(
    scene: &mut FieldScene,
    key: u32,
    value: &str,
    parent: Rect,
) -> Option<PlannedQuad> {
    let [x, y, w, h] = control_rect(&scene.ui, key)?;

    let local = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

    let at = (
        parent.x + local.x + super::stext::LEFT_INSET * scene.layout.sx,
        parent.y + local.y + (local.h - super::stext::LINE_BOX * scene.layout.sy) * 0.5,
    );

    if !scene.text.resolve(key, value, 0xFFFF_FFFF) {
        return None;
    }

    let line = scene.text.get(key)?;

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::SceneText(key),
        quad: super::stext::quad(line, at),
        uv: UvRule::TexelCentre,
    })
}

fn question_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let seed = scene.question.draw_seed();

    if !seed.visible {
        return Vec::new();
    }

    let solid = |rect: super::question::Rect, colour| PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::White,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: rect.left,
            dst_y: rect.top,
            dst_w: rect.width(),
            dst_h: rect.height(),
            src_x: 0.0,
            src_y: 0.0,
            src_w: 1.0,
            src_h: 1.0,
            color: colour,
            rotation: 0.0,
        },
        uv: UvRule::TexelCentre,
    };

    let mut out = vec![solid(seed.panel, seed.panel_color)];

    let mut text = |seed: &super::question::TextSeed| {
        let key = u32::from(seed.control.unwrap_or(super::question::PANEL_ID));

        let at = (
            seed.rect.left + super::stext::LEFT_INSET * scene.layout.sx,
            seed.rect.top + (seed.rect.height() - super::stext::LINE_BOX * scene.layout.sy) * 0.5,
        );

        if !scene
            .text
            .resolve(key, &super::stext::display_text(&seed.text), seed.color)
        {
            return None;
        }

        let line = scene.text.get(key)?;

        Some([
            PlannedQuad {
                mode: RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: super::stext::shadow_quad(line, at),
                uv: UvRule::TexelCentre,
            },
            PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::SceneText(key),
                quad: super::stext::quad(line, at),
                uv: UvRule::TexelCentre,
            },
        ])
    };

    for answer in seed.answers.iter().rev() {
        out.push(solid(answer.rect, super::question::BUTTON_COLOR));

        out.extend(text(answer).into_iter().flatten());
    }

    out.extend(text(&seed.question).into_iter().flatten());

    out
}

fn long_message_origin(scene: &FieldScene) -> (f32, f32) {
    (
        scene.layout.width as f32 * 0.5 - super::long_message::PANEL_HALF_WIDTH,
        scene.layout.height as f32 * 0.5
            - scene.layout.sy * super::long_message::PANEL_VERTICAL_OFFSET,
    )
}

fn long_message_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let mut rows = Vec::new();

    if scene.long_message.panel.visible {
        let parent = long_message_origin(scene);

        rows.extend(long_message_backdrops(scene, parent));

        let mut controls = Vec::with_capacity(5);

        controls.push((
            super::long_message::CAPTION_ID,
            scene.long_message.panel.caption.clone(),
        ));

        controls.extend(
            super::long_message::LINE_IDS
                .into_iter()
                .zip(scene.long_message.panel.lines.clone()),
        );

        for (key, value) in controls {
            let Some([x, y, w, h]) = control_rect(&scene.ui, key) else {
                continue;
            };

            let control = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

            rows.extend(long_message_text_quad(
                scene,
                key,
                &value,
                0xFFFF_FFFF,
                parent,
                control,
            ));
        }
    }

    let visible = scene
        .long_message
        .visible_overlay()
        .cloned()
        .collect::<Vec<_>>();

    let parent = (
        scene.layout.sx * super::long_message::OVERLAY_AT.0,
        scene.layout.sy * super::long_message::OVERLAY_AT.1,
    );

    for (index, row) in visible.iter().enumerate() {
        let control = scene.layout.control(
            0.0,
            index as f32 * super::long_message::OVERLAY_ROW_HEIGHT,
            300.0,
            16.0,
        );

        if let Some(foreground) = long_message_text_quad(
            scene,
            0x7B10_0000 + index as u32,
            &row.text,
            row.colour,
            parent,
            control,
        ) {
            let line = scene.text.get(0x7B10_0000 + index as u32).unwrap();

            rows.push(PlannedQuad {
                mode: RenderMode::Ui2dPoint,
                texture: foreground.texture,
                uv: foreground.uv,
                quad: super::stext::shadow_quad(
                    line,
                    (foreground.quad.dst_x, foreground.quad.dst_y),
                ),
            });

            rows.push(foreground);
        }
    }

    rows
}

fn long_message_backdrops(scene: &FieldScene, parent: (f32, f32)) -> Vec<PlannedQuad> {
    scene
        .ui
        .records
        .iter()
        .filter(|r| {
            r.control == ControlType::Panel && r.header.parent_id == super::long_message::PANEL_ID
        })
        .filter_map(|record| {
            let [x, y, w, h] = record.rect()?;

            let local = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

            let rect = Rect {
                x: parent.0 + local.x,
                y: parent.1 + local.y,
                w: local.w,
                h: local.h,
            };

            if !on_screen(&scene.layout, rect) {
                return None;
            }

            let item = set_item(&scene.sets, record)?;

            Some(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::Scene(u32::try_from(item.texture_index).ok()?),
                quad: textured(rect, item, *record.fields.get(5)? as u32, 0.0),
                uv: UvRule::TexelCentre,
            })
        })
        .collect()
}

fn long_message_text_quad(
    scene: &mut FieldScene,
    key: u32,
    value: &str,
    colour: u32,
    parent: (f32, f32),
    control: Rect,
) -> Option<PlannedQuad> {
    let (x, y) = super::stext::origin(&scene.layout, parent, control);

    if !on_screen(
        &scene.layout,
        Rect {
            x,
            y,
            w: control.w,
            h: control.h,
        },
    ) || !scene.text.resolve(key, value, colour)
    {
        return None;
    }

    let line = scene.text.get(key)?;

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::SceneText(key),
        quad: super::stext::quad(line, (x, y)),
        uv: UvRule::TexelCentre,
    })
}

fn reel_parent_rect(scene: &FieldScene) -> Option<Rect> {
    let [x, y, w, h] = control_rect(&scene.ui, super::reel::PANEL)?;

    let mut rect = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

    if let Some((x, y)) = scene.reel_position {
        rect.x = x;

        rect.y = y;
    }

    Some(rect)
}

fn passive_panel_mouse(scene: &FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> bool {
    use ui::input::Mouse;

    if !matches!(
        event,
        Mouse::Move | Mouse::LeftDown | Mouse::LeftUp | Mouse::RightDown
    ) {
        return false;
    }

    let hit = |id, parent: (f32, f32)| {
        panel(&scene.ui, id)
            .and_then(|r| r.rect())
            .is_some_and(|r| {
                let rect = scene
                    .layout
                    .control(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32);

                let (cx, cy) = (parent.0.trunc() + rect.x, parent.1.trunc() + rect.y);

                x as f32 >= cx
                    && y as f32 >= cy
                    && (x as f32) < cx + rect.w
                    && (y as f32) < cy + rect.h
            })
    };

    if super::message_panel::consumes_mouse(&scene.message_panel, &scene.layout, event, x, y) {
        return true;
    }

    if nameplate::consumes_gauge_mouse(scene, event, x, y) {
        return true;
    }

    if nameplate::consumes_badge_mouse(scene, event, x, y) {
        return true;
    }

    if super::drop_presentation::consumes_badge_mouse(scene, event, x, y) {
        return true;
    }

    if [0x1675, 0x10084].into_iter().any(|id| hit(id, (0.0, 0.0))) {
        return true;
    }

    if scene.arena.visible && hit(0x16200, (0.0, 0.0)) {
        return true;
    }

    if scene.minimap_visible
        && let Some(map) = live_minimap_rect(scene)
        && let Some(bar) = title_rect(scene)
    {
        let (cx, cy) = (map.x.trunc() + bar.x, map.y.trunc() + bar.y);

        if x as f32 >= cx && y as f32 >= cy && (x as f32) < cx + bar.w && (y as f32) < cy + bar.h {
            return true;
        }
    }

    scene.long_message.panel.visible
        && [0x17d9, 0x17da, 0x17db]
            .into_iter()
            .any(|id| hit(id, long_message_origin(scene)))
}

fn reel_parent_mouse(scene: &mut FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> bool {
    use ui::input::Mouse;

    if !scene.reel.visible {
        return false;
    }

    let Some(parent) = reel_parent_rect(scene) else {
        return false;
    };

    let inside = x as f32 >= parent.x
        && y as f32 >= parent.y
        && (x as f32) < parent.x + parent.w
        && (y as f32) < parent.y + parent.h;

    match event {
        Mouse::Move => {
            if let Some((px, py)) = scene.reel_drag_cursor {
                scene.reel_position = Some((
                    (parent.x + (x - px) as f32)
                        .max(0.0)
                        .min(scene.layout.width as f32 - parent.w),
                    (parent.y + (y - py) as f32)
                        .max(0.0)
                        .min(scene.layout.height as f32 - parent.h),
                ));

                scene.reel_drag_cursor = Some((x, y));
            }
        }
        Mouse::LeftDown => {
            if !scene.reel_drag_capture && inside && (y as f32) < parent.y + 24.0 {
                scene.reel_drag_capture = true;

                scene.reel_drag_cursor = Some((x, y));
            }
        }
        Mouse::LeftUp => {
            scene.reel_drag_capture = false;

            scene.reel_drag_cursor = None;
        }
        Mouse::RightDown => {}
        _ => return false,
    }

    let child_inside = panel(&scene.ui, 0x1911)
        .filter(|r| r.header.parent_id == super::reel::PANEL)
        .and_then(|r| r.rect())
        .zip(reel_parent_rect(scene))
        .is_some_and(|(r, p)| {
            let child = scene
                .layout
                .control(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32);

            let (cx, cy) = (p.x.trunc() + child.x, p.y.trunc() + child.y);

            x as f32 >= cx
                && y as f32 >= cy
                && (x as f32) < cx + child.w
                && (y as f32) < cy + child.h
        });

    inside || child_inside
}

fn reel_hit(scene: &FieldScene, point: (f32, f32)) -> (Option<u16>, bool) {
    let Some(parent) = reel_parent_rect(scene) else {
        return (None, false);
    };

    let contains = |rect: Rect| {
        rect.x <= point.0
            && rect.y <= point.1
            && point.0 < rect.x + rect.w
            && point.1 < rect.y + rect.h
    };

    let hit = super::reel::BUTTONS.iter().copied().find(|command| {
        let Some([x, y, w, h]) = control_rect(&scene.ui, u32::from(*command)) else {
            return false;
        };

        let local = scene
            .layout
            .control(x as f32 + 1.0, y as f32 - 1.0, w as f32, h as f32);

        contains(Rect {
            x: parent.x.trunc() + local.x,
            y: parent.y.trunc() + local.y,
            w: local.w,
            h: local.h,
        })
    });

    (hit, contains(parent))
}

fn reel_command(scene: &mut FieldScene, command: u16, now_ms: u32) {
    if !scene.lock.allows(now_ms) {
        return;
    }

    match command {
        super::reel::START => {
            let outcome = reel_start(scene, now_ms);

            tracing::info!("NWYD: reel start button — {outcome:?}");
        }
        super::reel::QUIT => {
            if close_reel(scene) {
                tracing::info!("NWYD: reel panel closed");
            }
        }
        _ if scene.reel.adjust(command, scene.entered.gold) => {
            tracing::info!("NWYD: reel wager changed to {}", scene.reel.wager);
        }
        _ => {}
    }
}

pub fn mouse(scene: &mut FieldScene, event: ui::input::Mouse, x: i32, y: i32) -> Vec<hud::Command> {
    if matches!(
        event,
        ui::input::Mouse::RightDown | ui::input::Mouse::RightUp
    ) {
        scene.right_held = false;
    }

    scene.right_suspended = true;

    super::guild::refresh_inputs(scene);

    sync_affect_tooltip_clock(scene);

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    if let Some(panels) = scene.panels.as_mut() {
        match event {
            ui::input::Mouse::LeftDown => panels.spend_repeat.left_down = true,
            ui::input::Mouse::LeftUp => panels.spend_repeat.left_down = false,
            _ => {}
        }

        panels.now_ms = scene.time.now_ms;
    }

    if air_move_active(scene) {
        return Vec::new();
    }

    let now_ms = scene.time.now_ms;

    let lock = scene.lock;

    refresh_item_inputs(scene);

    let mut combat = scene.combat;

    let cancelled_carry = event == ui::input::Mouse::RightDown
        && scene
            .hud
            .as_ref()
            .is_some_and(|hud| hud.manager.carried.is_some());

    let pgt_context = panel_command_context(scene);

    if event == ui::input::Mouse::LeftDown {
        scene.gate.answer_pressed = false;
    }

    let (commands, actions, consumed, notifications) = {
        let FieldScene {
            hud,
            panels: screens,
            entered,
            quick_bar,
            gate,
            layout,
            hovered,
            ..
        } = &mut *scene;

        match (hud.as_mut(), screens.as_mut()) {
            (Some(hud), Some(screens)) => {
                let scale = layout.ui_scale();

                let (px, py) = ((x as f32 / scale) as i32, (y as f32 / scale) as i32);

                if event == ui::input::Mouse::LeftDown {
                    gate.answer_pressed = screens.box_hit(px, py).is_some_and(|(slot, _)| {
                        screens.boxes.open().any(|(candidate, open)| {
                            candidate == slot
                                && open.kind == super::interact::TALK_BOX_KIND
                                && open.context == super::gate::CONTEXT
                        })
                    });
                }

                let (events, consumed) = panels::route_in_field(
                    hud,
                    screens,
                    entered,
                    quick_bar,
                    event,
                    (px, py),
                    panels::FieldInput {
                        now_ms,
                        player_present: pgt_context.player_present,
                        player_dead: pgt_context.player_dead,
                    },
                );

                let commands = hud::handle_in_field(
                    hud,
                    &events,
                    now_ms,
                    &lock,
                    &mut combat,
                    pgt_context.player_present,
                );

                let (actions, clear_hover) = panels::handle_with_manager(
                    hud,
                    screens,
                    entered,
                    &events,
                    lock.allows(now_ms),
                    &pgt_context,
                    panels::FieldInput {
                        now_ms,
                        player_present: pgt_context.player_present,
                        player_dead: pgt_context.player_dead,
                    },
                );

                if clear_hover {
                    *hovered = None;
                }

                if event == ui::input::Mouse::Move {
                    let shift = hud.manager.keys.shift;

                    panels::hover(hud, screens, entered, shift);
                }

                (commands, actions, consumed, events)
            }
            _ => (Vec::new(), Vec::new(), false, Vec::new()),
        }
    };

    finish_widget_commands(scene, &commands, actions, combat, now_ms);

    if let Some(panels) = scene.panels.as_mut() {
        panels::finish_popup_events(panels, &notifications);
    }

    if air_move_active(scene) {
        return commands;
    }

    if scene.legacy_boxes.active().is_some() {
        if !(event == ui::input::Mouse::LeftDown && consumed)
            && scene.lock.allows(now_ms)
            && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        {
            scene.legacy_boxes.mouse_with_capture(
                event,
                (x, y),
                &scene.layout,
                now_ms,
                scene.reel.visible && scene.reel_drag_capture,
            );
        }

        return commands;
    }

    if scene.firecracker.visible {
        if !(event == ui::input::Mouse::LeftDown && consumed)
            && scene.lock.allows(now_ms)
            && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        {
            super::firecracker::mouse(scene, event, x, y);
        }

        return commands;
    }

    if !(event == ui::input::Mouse::LeftDown && consumed)
        && scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
    {
        nameplate::mouse_names(scene, x, y);
    }

    let question_event = match event {
        ui::input::Mouse::LeftDown if !consumed => Some(super::question::Mouse::Down),
        ui::input::Mouse::LeftUp => Some(super::question::Mouse::Up),
        _ => None,
    };

    if scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        && let Some(event) = question_event
    {
        let input = scene.question.input(event, (x as f32, y as f32), now_ms);

        if let Some(choice) = input.choice {
            scene
                .unconditional_sounds
                .push(super::audio::UnconditionalSound::new(53));

            let response =
                formats::net::question::ChoiceResponse::new(scene.entered.client_id, choice.index)
                    .expect("question::State only returns one of its four buttons");

            scene.outbox.push(response.to_frame().to_vec());

            tracing::info!(
                "NWYD: 0x2C7 — question button {:#05x} answered choice {}",
                choice.control,
                choice.index,
            );
        }

        if input.consumed {
            return commands;
        }
    }

    let mut reel_parent_consumed = false;

    if scene.reel.visible
        && !(event == ui::input::Mouse::LeftDown && consumed)
        && scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
    {
        reel_parent_consumed = reel_parent_mouse(scene, event, x, y);

        scene.reel.hovered = reel_hit(scene, (x as f32, y as f32)).0;
    }

    let reel_event = match event {
        ui::input::Mouse::LeftDown if !consumed => Some(super::reel::Mouse::Down),
        ui::input::Mouse::LeftUp => Some(super::reel::Mouse::Up),
        _ => None,
    };

    if scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        && let Some(event) = reel_event
    {
        let (hit, inside_parent) = reel_hit(scene, (x as f32, y as f32));

        let input = scene.reel.input(event, hit, inside_parent, now_ms);

        if let Some(command) = input.command {
            scene
                .unconditional_sounds
                .push(super::audio::UnconditionalSound::new(53));

            reel_command(scene, command, now_ms);
        }

        if input.consumed {
            return commands;
        }
    }

    if !(event == ui::input::Mouse::LeftDown && consumed)
        && scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        && minimap_buttons_mouse(scene, event, x, y, now_ms)
    {
        return commands;
    }

    if reel_parent_consumed {
        return commands;
    }

    if !(event == ui::input::Mouse::LeftDown && consumed)
        && scene.lock.allows(now_ms)
        && now_ms.wrapping_sub(scene.last_interact_ms) >= interact::INTERACT_INTERVAL_MS
        && passive_panel_mouse(scene, event, x, y)
    {
        return commands;
    }

    scene.cursor = (x, y);

    scene.right_suspended = consumed || cancelled_carry;

    match event {
        ui::input::Mouse::LeftDown if !consumed => {
            if !death_mouse(scene) {
                let shift = shift_held(scene);

                click(scene, x, y, shift);
            }
        }
        ui::input::Mouse::RightDown if !consumed && !cancelled_carry => {
            if resurrect_mouse(scene, x, y, now_ms) {
                return commands;
            }

            scene.right_held = true;

            right_step(scene);
        }
        ui::input::Mouse::LeftUp => {
            scene.drag = None;

            let store_open = scene
                .panels
                .as_ref()
                .is_some_and(|screens| screens.is_open(panels::CONTENT_STORE));

            super::gate::clear_latch(&mut scene.gate, store_open);

            if !consumed {
                let released = interact::release(scene, now_ms);

                if let Some(line) = interact::describe_release(&released) {
                    tracing::info!("{line}");
                }
            }
        }
        ui::input::Mouse::RightUp if !consumed && ctrl_held(scene) => {
            let latched = interact::latch_target(scene, now_ms);

            if let Some(line) = interact::describe(&latched) {
                tracing::info!("{line}");
            }
        }
        ui::input::Mouse::Move => {
            if let Some(drag) = &mut scene.drag {
                drag.suspended = consumed;

                if !consumed {
                    drag.at = (x, y);

                    drag_moved(scene);
                }
            }
        }
        _ => {}
    }

    commands
}

fn death_mouse(scene: &mut FieldScene) -> bool {
    let Some(index) = scene.player else {
        return false;
    };

    let actor = &scene.world.actors.actors[index];

    if !(scene.entered.status.hp < 1 || actor.is_dead)
        || scene.entered.equipment.items[13].index == super::vanish::IMMUNE_PET
    {
        return false;
    }

    let cell = attack::cell_of(actor);

    let (left, top, right, bottom) = super::city::MUSIC_RECT;

    let arena = left <= cell.0 && cell.0 < right && top <= cell.1 && cell.1 < bottom;

    if attack::safe_here(scene.world.actors.attributes(), cell) && !arena {
        return false;
    }

    raise_revive_box(scene.panels.as_mut(), &super::pending::death_box());

    true
}

fn resurrect_mouse(scene: &mut FieldScene, x: i32, y: i32, now_ms: u32) -> bool {
    let Some(index) = scene.player else {
        return false;
    };

    let actor = &scene.world.actors.actors[index];

    if scene.revive_pending.resurrect.armed
        || scene.entered.equipment.items[13].index == super::vanish::IMMUNE_PET
        || !(scene.entered.status.hp < 1 || actor.is_dead)
        || ctrl_held(scene)
        || !scene.lock.allows(now_ms)
        || x <= 0
        || y <= 0
        || x >= scene.layout.width as i32
        || y >= scene.layout.height as i32
    {
        return false;
    }

    let Some(slot) = super::pending::resurrect_slot(&scene.entered.quick) else {
        return true;
    };

    scene.entered.quick_selected = slot;

    raise_revive_box(scene.panels.as_mut(), &super::pending::resurrect_box());

    true
}

fn raise_revive_box(panels: Option<&mut Panels>, request: &super::msgbox::Request) {
    let Some(panels) = panels else {
        return;
    };

    if panels
        .boxes
        .find(super::pending::DEATH_BOX_CONTEXT)
        .is_none()
        && panels
            .boxes
            .find(super::pending::RESURRECT_BOX_CONTEXT)
            .is_none()
    {
        panels.raise_box(request, None);
    }
}

fn ctrl_held(scene: &FieldScene) -> bool {
    scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.ctrl)
}

fn shift_held(scene: &FieldScene) -> bool {
    scene.hud.as_ref().is_some_and(|hud| hud.manager.keys.shift)
}

fn shift_player_whisper(scene: &mut FieldScene, now_ms: u32) -> bool {
    if !shift_held(scene) {
        return false;
    }

    let Some(entity) = scene
        .hovered
        .filter(|entity| *entity < nameplate::PLAYER_ID_LIMIT)
    else {
        return false;
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return false;
    };

    let actor = &scene.world.actors.actors[index];

    let x = actor.position.x.trunc() as i32;

    let z = actor.position.z.trunc() as i32;

    if (x & !0x7F) == 0x80 && (z & !0x7F) == 0x200 && !actor.tracked {
        return true;
    }

    let name = actor.name.clone();

    if let (Some(hud), Some(panels)) = (&mut scene.hud, &mut scene.panels) {
        panels.chat.whisper = name;

        let strings = panels.strings.clone();

        chat::set_mode(panels, chat::WHISPER, true, &strings);

        chat::request_edit_focus(panels, hud);
    }

    scene.attack.last_attack_ms = now_ms;

    true
}

fn menu_action(scene: &FieldScene, content: Option<PanelId>) -> Option<Action> {
    match content {
        Some(panels::CONTENT_INVENTORY) => Some(Action::HudInventory),
        Some(panels::CONTENT_CHARACTER) => {
            let panels = scene.panels.as_ref()?;

            panels.screen(super::panels::CONTENT_CHARACTER)?;

            if !panels.is_open(super::panels::CONTENT_CHARACTER)
                && panels.is_open(super::panels::CONTENT_STORE)
            {
                None
            } else {
                Some(Action::Toggle(super::panels::CONTENT_CHARACTER))
            }
        }
        None => Some(Action::OpenCommunity),
        Some(panels::CONTENT_STORE) => {
            let actor = scene
                .player
                .and_then(|i| scene.world.actors.actors.get(i))?;

            if !attack::safe_here(
                scene.world.actors.attributes(),
                (actor.position.x as i32, actor.position.z as i32),
            ) {
                return None;
            }

            scene.panels.as_ref()?.screen(panels::CONTENT_STORE)?;

            let open = scene
                .panels
                .as_ref()
                .is_some_and(|p| p.is_open(panels::CONTENT_STORE));

            Some(match open {
                true => Action::CloseStore,
                false => Action::StorePrompt,
            })
        }
        Some(content) if panels::unbuilt(content) => Some(Action::Unbuilt(content)),
        Some(content) => Some(Action::Toggle(content)),
    }
}

pub fn character(scene: &mut FieldScene, ch: char) -> Vec<Action> {
    if air_move_active(scene) {
        return Vec::new();
    }

    if !scene.lock.allows(scene.time.now_ms) {
        return Vec::new();
    }

    if chat_key(scene, ch) {
        return Vec::new();
    }

    if scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.chat.focused)
    {
        return Vec::new();
    }

    if let Some(hud) = scene.hud.as_mut()
        && ch == '\u{1b}'
        && hud.manager.carried.is_some()
    {
        hud.manager.cancel_carry();

        return Vec::new();
    }

    if ch != '\u{1b}' && scene.legacy_boxes.active().is_some() {
        scene.legacy_boxes.character(ch);

        return Vec::new();
    }

    let bound = {
        let FieldScene {
            hud,
            panels: screens,
            entered,
            ..
        } = &mut *scene;

        match (hud.as_mut(), screens.as_mut()) {
            (Some(hud), Some(screens)) => panels::bind_hovered_skill(hud, screens, entered, ch),
            _ => false,
        }
    };

    if bound {
        return Vec::new();
    }

    if scene.skill_bar.adjust_auto_count(ch, scene.attack.auto) {
        if scene.attack.auto {
            let count = usize::try_from(scene.skill_bar.auto_count).unwrap_or(0);

            if let Some(hud) = scene.hud.as_mut() {
                hud.refresh_flags(scene.pk, true, count);
            }
        }

        return Vec::new();
    }

    if matches!(ch, '=' | '+') {
        if let Some(panels) = scene.panels.as_mut() {
            chat::size_hotkey(panels);
        }

        return Vec::new();
    }

    if matches!(ch, 'n' | 'N') && !scene.name_display.blocked {
        scene.name_display.toggle();

        return Vec::new();
    }

    if let Some(slot) = super::quick_bar::hotkey(ch)
        && use_quick(scene, slot)
    {
        return Vec::new();
    }

    let alive = scene.entered.status.hp as i32 > 0;

    if super::automatic_move::character(scene, ch) {
        return Vec::new();
    }

    if !alive && (super::skill_bar::key_slot(ch).is_some() || matches!(ch, 'z' | 'Z')) {
        return Vec::new();
    }

    if ch == '\u{1b}' && scene.firecracker.visible {
        let earlier = scene
            .panels
            .as_ref()
            .is_some_and(|p| panels::ESCAPE_ORDER[..4].iter().any(|&id| p.is_open(id)));

        if !earlier {
            scene.firecracker.visible = false;

            return Vec::new();
        }
    }

    if ch == '\u{1b}' && scene.reel.visible {
        let earlier_panel_is_open = scene.panels.as_ref().is_some_and(|panels| {
            !matches!(
                panels::escape(panels),
                Action::Toggle(panels::CONTENT_SYSTEM)
            )
        });

        if !earlier_panel_is_open {
            close_reel(scene);

            return Vec::new();
        }
    }

    if ch == '\u{1b}' && scene.legacy_boxes.states[0].visible {
        let earlier = scene
            .panels
            .as_ref()
            .is_some_and(|p| !matches!(panels::escape(p), Action::Toggle(panels::CONTENT_SYSTEM)));

        if !earlier {
            scene
                .legacy_boxes
                .hide(super::legacy_message_box::Owner::Question);

            return Vec::new();
        }
    }

    let action = match ch {
        'z' | 'Z' => Some(Action::SkillPage),
        '\u{1b}' => scene.panels.as_ref().map(|_| Action::Escape),
        ch => scene
            .panels
            .as_ref()
            .and_then(|screens| panels::hotkey(screens, ch)),
    };

    let Some(action) = action else {
        let page = scene.hud.as_ref().map(|hud| hud.page);

        let selected = super::skill_bar::select(&mut scene.emote_keys.swallow_next, ch, page);

        apply_skill_selection(scene, selected);

        return Vec::new();
    };

    act(scene, &[action])
}

fn apply_skill_selection(scene: &mut FieldScene, selected: super::skill_bar::Select) {
    let super::skill_bar::Select::Selected(slot) = selected else {
        return;
    };

    scene.entered.quick_selected = slot as u8;

    if let Some(hud) = scene.hud.as_mut() {
        hud.select_skill_only(slot);
    }
}

fn use_quick(scene: &mut FieldScene, slot: usize) -> bool {
    if scene.hud.is_none() || scene.player.is_none() {
        return false;
    }

    let position = item_position(scene);

    let FieldScene {
        quick_bar,
        entered,
        statics,
        panels,
        time,
        ..
    } = scene;

    let Some(panels) = panels.as_mut() else {
        return false;
    };

    let effects = statics.effects();

    if super::quick_bar::cooldown_handled(
        quick_bar,
        entered,
        &effects,
        &panels.cooldowns,
        slot,
        time.now_ms,
    ) {
        return true;
    }

    let actions = super::quick_bar::use_slot(
        quick_bar,
        entered,
        &statics.items,
        &effects,
        &mut panels.cooldowns,
        slot,
        time.now_ms,
        position,
    );

    if actions.is_empty() {
        return false;
    }

    panels::carry_out(panels, entered, &actions, time.now_ms);

    true
}

pub fn unclaimed_key(scene: &mut FieldScene, key: ui::text_entry::Key) {
    if !scene.lock.allows(scene.time.now_ms) {
        return;
    }

    match key {
        ui::text_entry::Key::Insert => {
            let mut packet = vec![0; 160];

            packet[4..6].copy_from_slice(&0x334u16.to_le_bytes());

            packet[6..8].copy_from_slice(&scene.entered.client_id.to_le_bytes());

            packet[12..16].copy_from_slice(b"time");

            scene.outbox.push(packet);
        }
        ui::text_entry::Key::PageUp | ui::text_entry::Key::PageDown => {
            if let Some(panels) = scene.panels.as_mut() {
                chat::page_log(panels, key == ui::text_entry::Key::PageDown);
            }
        }
        _ => {}
    }
}

pub fn inventory_key(scene: &mut FieldScene, tab: usize) {
    if tab >= 4 || !scene.lock.allows(scene.time.now_ms) {
        return;
    }

    let open = scene
        .panels
        .as_ref()
        .is_some_and(|p| p.is_open(panels::CONTENT_INVENTORY));

    if !open {
        super::inventory::open(scene, 0, false);
    }

    if let Some(panels::Panels {
        screens: all,
        items,
        icons,
        strings,
        ..
    }) = scene.panels.as_mut()
        && let Some(screen) = all
            .iter_mut()
            .find(|s| s.content == panels::CONTENT_INVENTORY && s.open)
    {
        screens::inventory_page(screen, tab as i32);

        if scene.player.is_some() {
            screens::inventory_bag_refresh(screen, &scene.entered, items, icons, strings);
        }
    }
}

pub fn delete_hud_slot(scene: &mut FieldScene) {
    if !scene.lock.allows(scene.time.now_ms) || scene.player.is_none() {
        return;
    }

    let Some(hud) = scene.hud.as_mut() else {
        return;
    };

    if scene.quick_bar.selected.is_none() {
        let page = usize::try_from(hud.page).unwrap_or(0).min(1);

        let checked = (page * 10..page * 10 + 10).find(|slot| {
            hud.bar
                .ui
                .panel
                .element(
                    &format!("Skill{slot}"),
                    formats::ui::pane::ElementType::IconButton,
                )
                .and_then(|element| element.widget.button())
                .is_some_and(|state| state.checked)
        });

        let Some(slot) = checked else {
            return;
        };

        if let Some(element) = hud.bar.ui.panel.element_mut(
            &format!("Skill{slot}"),
            formats::ui::pane::ElementType::IconButton,
        ) {
            element.set_icon_contents(None);
        }

        scene.entered.quick[slot] = 0xFF;

        if let Some(panels) = scene.panels.as_mut() {
            panels.wire.push(protocol::client::quick_bar(
                &scene.entered.quick,
                scene.entered.class,
            ));
        }

        return;
    }

    let checked = (0..hud::QUICK_CELLS).find(|slot| {
        hud.bar
            .ui
            .panel
            .element(
                &format!("Quick{slot}"),
                formats::ui::pane::ElementType::IconButton,
            )
            .and_then(|element| element.widget.button())
            .is_some_and(|state| state.checked)
    });

    let Some(slot) = checked else {
        return;
    };

    if let Some(element) = hud.bar.ui.panel.element_mut(
        &format!("Quick{slot}"),
        formats::ui::pane::ElementType::IconButton,
    ) {
        element.set_icon_contents(None);
    }

    scene.quick_bar.clear(slot);
}

fn focused_widget_edit(scene: &mut FieldScene) -> Option<&mut ui::EditState> {
    if air_move_active(scene) {
        return None;
    }

    let panels = scene.panels.as_mut()?;

    if panels.msgbox.placed.ui.panel.visible || panels.is_open(super::panels::CONTENT_COIN_INPUT) {
        return None;
    }

    let hud = scene.hud.as_mut()?;

    let target = hud.manager.focused?;

    if chat::holds_caret(panels, &hud.manager) {
        return None;
    }

    if !matches!(
        target.owner,
        ui::draw::Owner::Element(_)
            | ui::draw::Owner::ListRow { child: Some(_), .. }
            | ui::draw::Owner::Descendant { item: None, .. }
    ) {
        return None;
    }

    let placed = match target.panel {
        0 => &mut hud.bar,
        1 => &mut hud.plate,
        2 => &mut hud.target,
        index => {
            let screen = panels
                .screens
                .get_mut(index.checked_sub(super::panels::HUD_PANELS)?)?;

            if !screen.open {
                return None;
            }

            &mut screen.placed
        }
    };

    hud.manager.reconcile_panel(target.panel, &placed.ui.panel);

    let target = hud.manager.focused?;

    let edit = ui::input::owner_element_mut(&mut placed.ui.panel, target.owner)?
        .0
        .widget
        .edit_mut()?;

    if matches!(target.owner, ui::draw::Owner::Element(_)) && edit.note_pad {
        return None;
    }

    Some(edit)
}

pub fn type_char(scene: &mut FieldScene, ch: char) -> bool {
    let mut clipboard = std::mem::take(&mut scene.clipboard);

    let consumed = type_char_with_clipboard(scene, ch, &mut clipboard);

    scene.clipboard = clipboard;

    consumed
}

fn type_char_with_clipboard(
    scene: &mut FieldScene,
    ch: char,
    clipboard: &mut dyn ui::text_entry::Clipboard,
) -> bool {
    if let (Some(hud), Some(panels)) = (&mut scene.hud, &mut scene.panels) {
        super::panels::sync_coin_focus(hud, panels);
    }

    if scene
        .panels
        .as_mut()
        .is_some_and(|panels| panels.box_type_char(ch))
    {
        return true;
    }

    if scene
        .panels
        .as_mut()
        .is_some_and(|panels| panels.coin_input_type_char(ch))
    {
        return true;
    }

    if let Some(edit) = focused_widget_edit(scene) {
        ui::text_entry::character(edit, ch, clipboard);

        return true;
    }

    let FieldScene {
        hud: Some(hud),
        panels: Some(panels),
        ..
    } = scene
    else {
        return false;
    };

    if let Some((screen, index)) = panels.focused_notepad(&hud.manager) {
        let edit = screen.placed.ui.panel.elements[index]
            .widget
            .edit_mut()
            .unwrap();

        ui::text_entry::character(edit, ch, clipboard);

        super::panels::refresh_guild_notice_limit(screen);

        return true;
    }

    if !chat::holds_caret(panels, &hud.manager) {
        return false;
    }

    let strings = panels.strings.clone();

    let notify = match ch as u32 {
        8 => chat::edit_mut(panels).is_some_and(|edit| edit.caret != edit.anchor || edit.caret > 0),
        0x16 | 0x18 => true,
        1..=0x1d => false,
        _ => true,
    };

    if let Some(edit) = chat::edit_mut(panels) {
        ui::text_entry::character(edit, ch, clipboard);
    }

    if notify {
        chat::text_changed(panels, &strings);
    }

    true
}

pub fn edit_key(
    scene: &mut FieldScene,
    key: ui::text_entry::Key,
    mods: ui::text_entry::Mods,
) -> bool {
    let mut clipboard = std::mem::take(&mut scene.clipboard);

    let consumed = edit_key_with_clipboard(scene, key, mods, &mut clipboard);

    scene.clipboard = clipboard;

    consumed
}

fn edit_key_with_clipboard(
    scene: &mut FieldScene,
    key: ui::text_entry::Key,
    mods: ui::text_entry::Mods,
    clipboard: &mut dyn ui::text_entry::Clipboard,
) -> bool {
    if let (Some(hud), Some(panels)) = (&mut scene.hud, &mut scene.panels) {
        super::panels::sync_coin_focus(hud, panels);
    }

    if !matches!(
        key,
        ui::text_entry::Key::Return | ui::text_entry::Key::Escape
    ) && let Some(edit) = focused_widget_edit(scene)
    {
        ui::text_entry::key(edit, key, mods, clipboard);

        return true;
    }

    if key != ui::text_entry::Key::Escape
        && let (Some(hud), Some(panels)) = (&scene.hud, &mut scene.panels)
        && let Some((screen, index)) = panels.focused_notepad(&hud.manager)
    {
        screen
            .placed
            .ui
            .notepad_key_with_clipboard(index, key, mods, clipboard);

        super::panels::refresh_guild_notice_limit(screen);

        return true;
    }

    if matches!(
        key,
        ui::text_entry::Key::Return | ui::text_entry::Key::Escape
    ) {
        return false;
    }

    if scene
        .panels
        .as_mut()
        .is_some_and(|panels| panels.box_edit_key(key, mods))
    {
        return true;
    }

    if scene
        .panels
        .as_mut()
        .is_some_and(|panels| panels.coin_input_edit_key(key, mods))
    {
        return true;
    }

    let FieldScene {
        hud: Some(hud),
        panels: Some(panels),
        ..
    } = scene
    else {
        return false;
    };

    if !chat::holds_caret(panels, &hud.manager) {
        return false;
    }

    let strings = panels.strings.clone();

    if let ui::text_entry::Key::PageUp | ui::text_entry::Key::PageDown = key {
        chat::page_log(panels, key == ui::text_entry::Key::PageDown);

        return true;
    }

    if let ui::text_entry::Key::Up | ui::text_entry::Key::Down = key {
        let text = panels.chat.recall(key == ui::text_entry::Key::Up);

        chat::set_text(panels, &text);

        return true;
    }

    if let Some(edit) = chat::edit_mut(panels) {
        ui::text_entry::key(edit, key, mods, clipboard);
    }

    if key == ui::text_entry::Key::Delete {
        chat::text_changed(panels, &strings);
    }

    true
}

pub fn chat_key(scene: &mut FieldScene, ch: char) -> bool {
    refresh_item_inputs(scene);

    super::guild::refresh_inputs(scene);

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    let player_present = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_some();

    let player_dead = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_some_and(|a| a.is_dead);

    if scene.hud.is_none() || scene.panels.is_none() {
        return false;
    }

    let now_ms = scene.time.now_ms;

    let client_id = scene.entered.client_id;

    let speaker = scene.entered.name();

    let level = scene.entered.status.level as i16;

    let move_alpha = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map_or(f32::NAN, |actor| actor.mover.move_alpha);

    let mut raised = None;

    let mut local: Option<&'static chat::Local> = None;

    let mut relocation: Option<Option<String>> = None;

    let mut pending_command = None;

    let FieldScene {
        hud: Some(hud),
        panels: Some(panels),
        entered,
        ..
    } = &mut *scene
    else {
        return false;
    };

    if let Some((slot, yes)) = panels.box_key(ch) {
        panels.answer_box_in_field(
            entered,
            slot,
            yes,
            panels::FieldInput {
                now_ms,
                player_present,
                player_dead,
            },
        );

        super::store::finish_panel_close(scene);

        super::trade::finish_panel_close(scene);

        drain_store_owner_opened(scene);

        drain_guild_application_accepted(scene);

        drain_guild_fame_accepted(scene);

        super::pgt::chief_accepts(scene);

        drain_summon_accepted(scene, now_ms);

        drain_revive_accepted(scene, now_ms);

        drain_item_skill_accepted(scene, now_ms);

        if let Some(frame) = super::trade::panel_accepts(scene) {
            scene.outbox.push(frame);

            tracing::info!("NWYD: 899 — the trade was accepted and the buffer is on the wire");
        }

        drain_staged(scene, now_ms);

        if let Some(accepted) = super::gate::panel_accepts(scene) {
            tracing::info!(
                "{}",
                match accepted {
                    super::gate::Accepted::Sent =>
                        "NWYD: 0x290 — using the teleport gate underfoot",
                    super::gate::Accepted::OffGate =>
                        "NWYD: the teleport gate answered Yes off its own cell — strdef[38]",
                },
            );
        }

        return true;
    }

    if let Some(yes) = panels.coin_input_key(ch) {
        answer_coin_input(scene, yes);

        return true;
    }

    let focused = panels.chat.focused;

    let consumed = match ch {
        '\r' | '\n' => {
            match (focused, chat::holds_caret(panels, &hud.manager)) {
                (false, _) => chat::focus(panels, hud, true, false),
                (true, true) => {
                    let reserved = panels.reserved.clone();

                    match chat::enter_with_move_alpha(
                        panels, client_id, &speaker, level, move_alpha, &reserved, now_ms,
                    ) {
                        chat::Enter::Local(row) => {
                            local = Some(row);
                        }
                        chat::Enter::Relocate(target) => relocation = Some(target),
                        chat::Enter::PendingCommand(frame) => {
                            pending_command = Some(frame);
                        }
                        _ => {}
                    }

                    chat::focus(panels, hud, false, false);

                    raised = chat::take_balloon(panels);
                }
                (true, false) => chat::focus(panels, hud, true, false),
            }

            true
        }
        '\u{1b}' if focused => {
            chat::focus(panels, hud, false, true);

            true
        }
        _ => false,
    };

    if let Some((speaker, text, life_ms)) = raised {
        balloon::raise(scene, speaker, &text, life_ms);
    }

    if let Some(row) = local {
        run_local(scene, row);
    }

    if let Some(target) = relocation {
        super::summon::command(
            &mut scene.summon,
            &mut scene.lock,
            target.as_deref(),
            now_ms,
        );
    }

    if let Some(frame) = pending_command {
        scene.command_pending.arm(&mut scene.lock, frame, now_ms);
    }

    consumed
}

fn run_local(scene: &mut FieldScene, row: &chat::Local) {
    match row.what {
        chat::LocalWhat::Effects { toggle_grass } => {
            scene.world.actors.toggle_effects(toggle_grass);
        }
        chat::LocalWhat::Help => {
            act(scene, &[Action::Toggle(super::panels::CONTENT_HELP)]);
        }
        chat::LocalWhat::GuildMark => {
            scene.guild_mark_off = match scene.guild_mark_ini.as_deref() {
                Some(path) => toggle_guild_mark(path, scene.guild_mark_off),
                None => !scene.guild_mark_off,
            };

            scene.marks.off = scene.guild_mark_off;

            let text = match scene.guild_mark_off {
                false => chat::GUILD_MARK_ON,
                true => chat::GUILD_MARK_OFF,
            };

            scene.long_message.local_reply(text);
        }
        chat::LocalWhat::ShowExp => {
            scene.long_message.rewards_hidden = !scene.long_message.rewards_hidden;

            scene
                .long_message
                .local_reply(if scene.long_message.rewards_hidden {
                    "Show Exp : OFF"
                } else {
                    "Show Exp : ON"
                });
        }
        other => {
            tracing::info!(
                "NWYD: not modelled — the local command {} ({other:?})",
                row.word
            )
        }
    }
}

fn answer_coin_input(scene: &mut FieldScene, yes: bool) {
    use super::panels::CONTENT_COIN_INPUT;

    let Some(p) = scene
        .panels
        .as_mut()
        .filter(|p| p.is_open(CONTENT_COIN_INPUT))
    else {
        return;
    };

    let action = p.coin_action;

    if !yes {
        if action == 12 {
            super::stack_split::answer(p, &scene.entered, false, false, scene.time.now_ms);
        }

        p.store_price = None;

        p.set_open(CONTENT_COIN_INPUT, false);

        return;
    }

    if scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_none_or(|a| a.is_dead)
    {
        return;
    }

    if action == 4 && p.store_price.is_some() {
        super::store::answer_price(p, &scene.entered, true, scene.time.now_ms);

        return;
    }

    let Some(text) = p
        .coin_input_text()
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
    else {
        return;
    };

    if text == "all" {
        let gold = match action {
            0 | 1 | 7 => scene.entered.gold,
            2 => scene.entered.warehouse.gold,
            _ => return,
        };

        p.coin_input_set_text((gold as i32).to_string());

        return;
    }

    let digits = text.bytes().all(|b| b.is_ascii_digit());

    let amount = text.parse::<i64>().unwrap_or(i64::MAX);

    if action == 12 && !digits {
        let message = p.strings.translate("ss_409");

        super::notice::post(p, &message, 1000, true, scene.time.now_ms);

        return;
    }

    if !matches!(action, 12 | 11 | 8 | 3 | 6)
        && (!digits || amount < 0 || (action == 0 && amount > i64::from(scene.entered.gold as i32)))
    {
        let message = p.strings.translate("ss_34");

        super::notice::post(p, &message, 1000, true, scene.time.now_ms);

        return;
    }

    match action {
        6 if p.seal_name_slot.is_some() => {
            super::seal_name::answer(scene, true);

            return;
        }
        12 if p.split_stack.is_some() => {
            if let Some(frame) =
                super::stack_split::answer(p, &scene.entered, true, false, scene.time.now_ms)
            {
                scene.outbox.push(frame);
            }

            return;
        }
        4 => {
            if p.store_price.is_some() {
                super::store::answer_price(p, &scene.entered, true, scene.time.now_ms);

                return;
            }

            if amount > 1999999999 {
                let message = p
                    .strings
                    .translate("ss_143")
                    .replacen("%d", "2000000000", 1);

                super::notice::post(p, &message, 1000, true, scene.time.now_ms);

                return;
            }
        }
        0 | 2 | 7 => {
            let opcode: u16 = match action {
                0 => 0x388,
                2 => 0x387,
                _ => 0x3d1,
            };

            let mut frame = vec![0; 16];

            frame[4..6].copy_from_slice(&opcode.to_le_bytes());

            frame[6..8].copy_from_slice(&scene.entered.client_id.to_le_bytes());

            frame[12..16].copy_from_slice(&(amount as u32).to_le_bytes());

            scene.outbox.push(frame);
        }
        1 if p.is_open(super::trade::CONTENT) => {
            let frame = super::trade::offer_gold(scene, amount as u32);

            scene.outbox.push(frame);
        }
        _ => {}
    }

    if let Some(p) = scene.panels.as_mut() {
        p.set_open(CONTENT_COIN_INPUT, false);
    }
}

fn open_coin_money(scene: &mut FieldScene, content: u32) {
    use super::panels::{CONTENT_CARGO, CONTENT_INVENTORY};

    if scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_none_or(|a| a.is_dead)
    {
        return;
    }

    let Some(p) = scene.panels.as_mut() else {
        return;
    };

    let (action, strdef, listener) = match content {
        CONTENT_INVENTORY if p.is_open(CONTENT_INVENTORY) => {
            if p.is_open(CONTENT_CARGO) {
                (0, 136, (0x18, 0x20))
            } else if p.is_open(super::trade::CONTENT) && (scene.trade.mine.gold as i32) <= 0 {
                (1, 137, (0x1d, 0x22))
            } else {
                return;
            }
        }
        CONTENT_CARGO if p.is_open(CONTENT_INVENTORY) => (2, 137, (0x18, 0x21)),
        _ => return,
    };

    if p.begin_coin_money(action, strdef) {
        scene.coin_prompt = Some(listener);
    }
}

fn panel_command_context(scene: &FieldScene) -> super::pgt::Context {
    let (pgt_player_model, pgt_player_dead) = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map_or((0, false), |actor| (actor.model_id as u8, actor.is_dead));

    super::pgt::Context {
        player_present: scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .is_some(),
        player_model: pgt_player_model,
        player_dead: pgt_player_dead,
        target_present: scene.target.is_some(),
        target: scene.target.and_then(|id| interact::actor_party(scene, id)),
        party_target: scene
            .panels
            .as_ref()
            .and_then(|p| p.pgt_target.as_ref())
            .and_then(|p| interact::actor_party(scene, p.entity)),
        first_row: scene.party.row_visual(0).map(|visual| visual.state),
        ally: scene.siege.ally,
    }
}

fn finish_widget_commands(
    scene: &mut FieldScene,
    commands: &[hud::Command],
    actions: Vec<Action>,
    combat: hud::Combat,
    now_ms: u32,
) {
    if scene
        .panels
        .as_mut()
        .is_some_and(|p| std::mem::take(&mut p.item_use_clear_hover))
    {
        scene.hovered = None;
    }

    if let (Some(hud), Some(panels)) = (scene.hud.as_mut(), scene.panels.as_mut()) {
        super::panels::sync_coin_focus(hud, panels);
    }

    let cycled = combat.movement != scene.combat.movement;

    scene.combat = combat;

    for command in commands {
        match *command {
            hud::Command::SelectSkill(slot) => {
                scene.entered.quick_selected = slot as u8;

                scene.quick_bar.selected = None;
            }
            hud::Command::SelectQuick(slot) => {
                scene.entered.quick_selected = 0;

                scene.quick_bar.selected = Some(slot);
            }
            hud::Command::UnboundSkill => scene.quick_bar.selected = None,
            hud::Command::UnboundQuick => scene.entered.quick_selected = 0,
            _ => {}
        }
    }

    if cycled {
        reanchor(scene);
    }

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    super::trade::panel_items(scene);

    drain_store_owner_opened(scene);

    drain_guild_application_accepted(scene);

    drain_guild_fame_accepted(scene);

    super::pgt::chief_accepts(scene);

    drain_summon_accepted(scene, now_ms);

    drain_revive_accepted(scene, now_ms);

    drain_item_skill_accepted(scene, now_ms);

    super::shop::panel_clicks(scene);

    for outcome in cast::panel_casts(scene, now_ms) {
        if let Some(line) = outcome.describe() {
            tracing::info!("{line}");
        }
    }

    for outcome in super::mission::panel_clicks(scene) {
        tracing::info!("{}", outcome.describe());
    }

    if let Some(filter) = super::collection::panel_refresh(scene) {
        tracing::info!("NWYD: collection book rebuilt on filter {filter:?}");
    }

    if let Some(index) = super::collection::panel_signs_up(scene) {
        tracing::info!("NWYD: 0x3FE — collection record {index} signed up");
    }

    for outcome in super::mix::panel_clicks(scene) {
        tracing::info!("{}", outcome.describe());
    }

    if let Some(gate) = super::pgt::panel_trades(scene) {
        tracing::info!(
            "{}",
            match gate {
                super::pgt::TradeGate::TooFar => "NWYD: PanelPGTMenu — trade target is too far",
                super::pgt::TradeGate::Dead => "NWYD: PanelPGTMenu — dead players cannot trade",
                super::pgt::TradeGate::Excluded => {
                    "NWYD: PanelPGTMenu — trade is blocked in the fixed exclusion rectangle"
                }
                super::pgt::TradeGate::AlreadyTrading => {
                    "NWYD: PanelPGTMenu — already trading (strdef[35])"
                }
                super::pgt::TradeGate::Send => "NWYD: 0x383 — the direct trade request is sent",
            }
        );
    }

    if let Some(panels) = scene.panels.as_mut() {
        if std::mem::take(&mut panels.pgt_clear_target) {
            scene.target = None;
        }

        let records = std::mem::take(&mut panels.pgt_toasts);

        for record in records {
            let text = panels.strings.translate(&format!("ss_{record}"));

            super::notice::post(panels, &text, 2000, true, now_ms);
        }
    }

    let messages = scene
        .panels
        .as_mut()
        .map(|p| std::mem::take(&mut p.pgt_messages))
        .unwrap_or_default();

    for record in messages {
        super::message_panel::post(scene, record, 2000, now_ms);
    }

    if let Some(frame) = super::trade::panel_accepts(scene) {
        scene.outbox.push(frame);

        tracing::info!("NWYD: 899 — the trade was accepted and the buffer is on the wire");
    }

    drain_staged(scene, now_ms);

    if let Some(accepted) = super::gate::panel_accepts(scene) {
        tracing::info!(
            "{}",
            match accepted {
                super::gate::Accepted::Sent => "NWYD: 0x290 — using the teleport gate underfoot",
                super::gate::Accepted::OffGate =>
                    "NWYD: the teleport gate answered Yes off its own cell — strdef[38]",
            },
        );
    }

    let mut raised: Vec<Action> = actions;

    for command in commands {
        if let hud::Command::Screen { content, .. } = *command {
            raised.extend(menu_action(scene, content));
        }
    }

    act(scene, &raised);
}

pub fn widget_checkable(scene: &mut FieldScene, events: &[ui::input::Event], now_ms: u32) {
    if air_move_active(scene) {
        return;
    }

    deliver_widget_activation_events(scene, events, now_ms);
}

pub fn widget_wheel(scene: &mut FieldScene, events: &[ui::input::Event], now_ms: u32) {
    if air_move_active(scene) {
        return;
    }

    deliver_widget_activation_events(scene, events, now_ms);
}

pub fn set_checkbox_checked(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    checked: bool,
    notify: bool,
    now_ms: u32,
) -> bool {
    let Some(panels) = scene.panels.as_mut() else {
        return false;
    };

    let Some(index) = panels.screens.iter().position(|s| s.content == content) else {
        return false;
    };

    let panel = &mut panels.screens[index].placed.ui.panel;

    let Some(element) = ui::input::retained_element(panel, owner) else {
        return false;
    };

    if !matches!(
        element.element_type,
        formats::ui::pane::ElementType::CheckBox | formats::ui::pane::ElementType::RadioButton
    ) || element.widget.button().is_none()
    {
        return false;
    }

    let event = ui::input::set_checkbox_checked(
        super::panels::input_slot(index),
        panel,
        owner,
        checked,
        notify,
    );

    if let Some(event) = event {
        deliver_widget_activation_events(scene, &[event], now_ms);
    }

    true
}

pub fn set_radio_checked(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    checked: bool,
    clear_group: bool,
    notify: bool,
    now_ms: u32,
) -> bool {
    let (index, panel) = match content {
        hud::CONTENT_HUD | hud::CONTENT_MY_INFO | hud::CONTENT_YOU_INFO => {
            let Some(hud) = scene.hud.as_mut() else {
                return false;
            };

            match content {
                hud::CONTENT_HUD => (0, &mut hud.bar.ui.panel),
                hud::CONTENT_MY_INFO => (1, &mut hud.plate.ui.panel),
                _ => (2, &mut hud.target.ui.panel),
            }
        }
        _ => {
            let Some(panels) = scene.panels.as_mut() else {
                return false;
            };

            let Some(index) = panels.screens.iter().position(|s| s.content == content) else {
                return false;
            };

            (
                super::panels::input_slot(index),
                &mut panels.screens[index].placed.ui.panel,
            )
        }
    };

    let Some(events) =
        ui::input::set_radio_checked(index, panel, owner, checked, clear_group, notify)
    else {
        return false;
    };

    deliver_widget_activation_events(scene, &events, now_ms);

    true
}

pub fn activate_checkable(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    now_ms: u32,
) -> bool {
    activate_widget(scene, content, owner, now_ms, WidgetActivation::Checkable)
}

pub fn activate_button(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    now_ms: u32,
) -> bool {
    activate_widget(scene, content, owner, now_ms, WidgetActivation::Button)
}

pub fn activate_combo_box(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    now_ms: u32,
) -> bool {
    activate_widget(scene, content, owner, now_ms, WidgetActivation::ComboBox)
}

enum WidgetActivation {
    Button,
    Checkable,
    ComboBox,
}

fn activate_widget(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    now_ms: u32,
    activation: WidgetActivation,
) -> bool {
    let Some(hud) = scene.hud.as_mut() else {
        return false;
    };

    let Some(panels) = scene.panels.as_mut() else {
        return false;
    };

    let index = match content {
        hud::CONTENT_HUD => 0,
        hud::CONTENT_MY_INFO => 1,
        hud::CONTENT_YOU_INFO => 2,
        _ => {
            let Some(i) = panels.screens.iter().position(|s| s.content == content) else {
                return false;
            };

            super::panels::input_slot(i)
        }
    };

    let target = ui::input::Target {
        panel: index,
        owner,
    };

    let retained = [
        (0, &mut hud.bar.ui.panel),
        (1, &mut hud.plate.ui.panel),
        (2, &mut hud.target.ui.panel),
    ]
    .into_iter()
    .chain(
        panels
            .screens
            .iter_mut()
            .enumerate()
            .map(|(i, s)| (super::panels::input_slot(i), &mut s.placed.ui.panel)),
    );

    let events = match activation {
        WidgetActivation::Checkable => hud.manager.activate_checkable(target, retained),
        WidgetActivation::Button => hud.manager.activate_button(target, retained),
        WidgetActivation::ComboBox => hud.manager.activate_combo_box(target, retained),
    };

    let Some(events) = events else {
        return false;
    };

    deliver_widget_activation_events(scene, &events, now_ms);

    true
}

fn deliver_widget_activation_events(
    scene: &mut FieldScene,
    events: &[ui::input::Event],
    now_ms: u32,
) {
    use ui::input::EventId;

    let events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e.id,
                EventId::Click
                    | EventId::CheckChanged
                    | EventId::RadioChanged
                    | EventId::ComboChanged
                    | EventId::ListScrolled
                    | EventId::CustomListScrolled
            )
        })
        .cloned()
        .collect();

    if events.is_empty() {
        return;
    }

    super::guild::refresh_inputs(scene);

    let context = panel_command_context(scene);

    refresh_item_inputs(scene);

    let allowed = scene.lock.allows(now_ms);

    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    let mut combat = scene.combat;

    let commands = scene
        .hud
        .as_mut()
        .map(|hud| {
            hud::handle_in_field(
                hud,
                &events,
                now_ms,
                &scene.lock,
                &mut combat,
                context.player_present,
            )
        })
        .unwrap_or_default();

    let at = spend::Raise {
        dead: context.player_dead,
        absent: scene.player.is_none(),
    };

    let actions = if let Some(hud) = scene.hud.as_mut() {
        let (actions, clear_hover) = super::panels::handle_with_manager(
            hud,
            panels,
            &mut scene.entered,
            &events,
            allowed,
            &context,
            super::panels::FieldInput {
                now_ms,
                player_present: context.player_present,
                player_dead: context.player_dead,
            },
        );

        if clear_hover {
            scene.hovered = None;
        }

        actions
    } else {
        super::panels::handle_with_pgt_scene(
            panels,
            &mut scene.entered,
            &events,
            at,
            allowed,
            &context,
        )
    };

    finish_widget_commands(scene, &commands, actions, combat, now_ms);

    if let Some(panels) = scene.panels.as_mut() {
        super::panels::finish_popup_events(panels, &events);
    }
}

pub fn widget_selection(scene: &mut FieldScene, events: &[ui::input::Event], now_ms: u32) {
    let allowed = scene.lock.allows(now_ms);

    let Some(panels) = scene.panels.as_mut() else {
        return;
    };

    let events: Vec<_> = events
        .iter()
        .filter(|event| event.id == ui::input::EventId::ListSelect)
        .cloned()
        .collect();

    let actions = super::panels::handle(
        panels,
        &mut scene.entered,
        &events,
        spend::Raise::default(),
        allowed,
    );

    act(scene, &actions);
}

pub fn remove_tree_item(
    scene: &mut FieldScene,
    content: crate::frame::PanelId,
    owner: ui::draw::Owner,
    row: usize,
    now_ms: u32,
) -> bool {
    let Some(hud) = scene.hud.as_mut() else {
        return false;
    };

    let (index, panel) = match content {
        hud::CONTENT_HUD => (0, &mut hud.bar.ui.panel),
        hud::CONTENT_MY_INFO => (1, &mut hud.plate.ui.panel),
        hud::CONTENT_YOU_INFO => (2, &mut hud.target.ui.panel),
        _ => {
            let Some(panels) = scene.panels.as_mut() else {
                return false;
            };

            let Some(index) = panels.screens.iter().position(|s| s.content == content) else {
                return false;
            };

            (
                super::panels::input_slot(index),
                &mut panels.screens[index].placed.ui.panel,
            )
        }
    };

    let Some(event) = hud.manager.remove_tree_item(index, panel, owner, row) else {
        return false;
    };

    widget_selection(scene, &[event], now_ms);

    true
}

fn act(scene: &mut FieldScene, actions: &[Action]) -> Vec<Action> {
    super::trade::finish_panel_close(scene);

    let mut panel_actions = Vec::with_capacity(actions.len());

    for &action in actions {
        match action {
            Action::HudInventory => hud_inventory(scene),
            Action::AirMove(row) => {
                begin_air_move(scene, row);
            }
            other => panel_actions.push(other),
        }
    }

    let skill_was_open = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.is_open(super::panels::CONTENT_SKILL));

    let deferred = match scene.panels.as_mut() {
        Some(screens) => panels::apply(screens, &panel_actions),
        None => panel_actions,
    };

    if !skill_was_open
        && let Some(panels) = scene.panels.as_mut()
        && panels.is_open(super::panels::CONTENT_SKILL)
    {
        super::screens::skill_opened(panels, &scene.entered);
    }

    let mut left = Vec::new();

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    for action in deferred {
        match action {
            Action::HideFirecracker => scene.firecracker.visible = false,
            Action::PkToggle => scene.pk = !scene.pk,
            Action::PartyAutoJoin(checked) => {
                if scene
                    .player
                    .and_then(|i| scene.world.actors.actors.get(i))
                    .is_some()
                {
                    scene.party_auto_join = checked;
                }
            }
            Action::PartyExit => super::party::panel_exit(scene),
            Action::PartyMember(row) => super::party::panel_member(scene, row),
            Action::PartyCast(row) => {
                cast::party_cast(scene, row, scene.time.now_ms);
            }
            Action::GuildChat => {
                scene.guild_chat = !scene.guild_chat;

                scene.outbox.push(super::chat::guild_chat_frame(
                    scene.entered.client_id,
                    scene.guild_chat,
                ));

                tracing::info!(
                    "NWYD: 0x333 — guild chat {}",
                    if scene.guild_chat { "on" } else { "off" }
                );
            }
            Action::CloseStore => {
                let closed = super::store::close(scene);

                if closed.wire() {
                    scene.outbox.push(super::store::close_frame());
                }

                tracing::info!("{}", super::store::describe_close(&closed));
            }
            Action::OpenStore => {
                let announced = match scene.panels.as_mut() {
                    Some(panels) => {
                        super::store::announce(panels, &scene.entered, scene.time.now_ms)
                    }
                    None => super::store::Announced::Empty,
                };

                match announced {
                    super::store::Announced::Empty => {
                        tracing::info!("NWYD: the personal shop has no selected trade items")
                    }
                    super::store::Announced::Sent(frame) => {
                        scene.outbox.push(frame);

                        tracing::info!("NWYD: 0x397 — the stocked personal shop is on the wire");
                    }
                }
            }
            Action::CoinInput(yes) => {
                answer_coin_input(scene, yes);
            }
            Action::SplitIndividual => {
                if let Some(panels) = scene.panels.as_mut() {
                    super::stack_split::toggle_individual(panels);
                }
            }
            Action::StoreExhibition => {
                if let Some(panels) = scene.panels.as_mut() {
                    super::store::toggle_exhibition(panels);
                }
            }
            Action::CoinMoney(content) => open_coin_money(scene, content),
            Action::TradeReady => match super::trade::press_ready(scene, scene.time.now_ms) {
                super::trade::Ready::Cooling => {
                    tracing::info!("NWYD: the trade's ready button is inside its 2,000 ms window")
                }
                super::trade::Ready::Sent { ready, frame } => {
                    scene.outbox.push(frame);

                    tracing::info!("NWYD: 899 — trade ready {}", u8::from(ready));
                }
            },
            Action::CloseTrade => {
                let closing = super::trade::close(scene);

                match closing.frame {
                    Some(frame) => {
                        scene.outbox.push(frame);

                        tracing::info!(
                            "NWYD: 900 — the trade window is down and the server is told"
                        );
                    }
                    None => {
                        tracing::info!("NWYD: the trade window is down; no partner, so no 900")
                    }
                }
            }
            Action::AutoAttack => {
                let armed = scene.attack.toggle_auto();

                tracing::info!("NWYD: auto-attack {}", if armed { "on" } else { "off" });
            }
            Action::Potion(which) if scene.entered.status.hp as i32 > 0 => {
                drink(scene, which, scene.time.now_ms);
            }
            Action::Potion(_) => {}
            Action::FeedMount => {
                feed_mount(scene, scene.time.now_ms);
            }
            Action::SkillPage => {
                let current = scene.hud.as_ref().map(|hud| hud.page);

                let flipped = super::skill_bar::flip_and_select_one(
                    &mut scene.emote_keys.swallow_next,
                    current,
                );

                if let Some(expected) = flipped.page
                    && let Some(hud) = scene.hud.as_mut()
                {
                    let page = hud.flip_page(&scene.entered.quick, &scene.statics.items);

                    debug_assert_eq!(page, expected);

                    tracing::info!("NWYD: quick bar page {}", page + 1);
                }

                apply_skill_selection(scene, flipped.select);
            }
            Action::MapCycle => map_cycle(scene),
            Action::Logout => start_exit(scene, ExitKind::Characters),
            Action::Quit => start_exit(scene, ExitKind::Quit),
            Action::ServerSelect => start_server_select(scene),
            Action::MoveChannel(channel) => {
                scene
                    .pending
                    .arm_server(&mut scene.lock, channel, scene.time.now_ms);
            }
            Action::MoveServerVisibility(open) => channel_popup_visibility(scene, open),
            other => left.push(other),
        }
    }

    if let (Some(hud), Some(panels)) = (scene.hud.as_mut(), scene.panels.as_mut()) {
        super::panels::sync_coin_focus(hud, panels);
    }

    left
}

fn hud_inventory(scene: &mut FieldScene) {
    scene.hovered = None;

    if scene.reel.visible {
        return;
    }

    let Some(panels) = scene.panels.as_ref() else {
        return;
    };

    if panels.screen(super::panels::CONTENT_INVENTORY).is_none() {
        return;
    }

    if panels.is_open(super::panels::CONTENT_INVENTORY) {
        act(scene, &[Action::Toggle(super::panels::CONTENT_INVENTORY)]);

        return;
    }

    act(scene, &[Action::CloseStore]);

    if let Some(panels) = scene.panels.as_mut() {
        panels.pair_inventory(0, false);

        if !panels.is_open(super::panels::CONTENT_INVENTORY) {
            panels.toggle(super::panels::CONTENT_INVENTORY);
        }

        panels.pair_inventory(0, false);
    }
}

pub fn drink(scene: &mut FieldScene, which: consume::Which, now_ms: u32) -> consume::Drunk {
    refresh_item_inputs(scene);

    let FieldScene {
        panels: Some(panels),
        entered,
        ..
    } = scene
    else {
        return consume::Drunk::default();
    };

    let drunk = {
        let Panels {
            items,
            icons,
            strings,
            live,
            cooldowns,
            outbox,
            screens,
            ..
        } = panels;

        let drunk = consume::potion(entered, items, live, cooldowns, now_ms, which);

        for &(_, request) in &drunk.sent {
            outbox.push(request);
        }

        if !drunk.sent.is_empty()
            && let Some(screen) = screens
                .iter_mut()
                .find(|s| s.content == panels::CONTENT_INVENTORY)
        {
            screens::inventory_refresh(screen, entered, items, icons, strings);
        }

        drunk
    };

    if let Some(line) = drunk.describe(which) {
        tracing::info!("{line}");
    }

    for &(slot, _) in &drunk.sent {
        panels::refresh_inventory_hover(panels, entered, slot);

        panels::refine_item_changed(panels, entered);
    }

    for _ in &drunk.sent {
        scene.world.play_sound(consume::POTION_SOUND);
    }

    drunk
}

pub fn feed_mount(scene: &mut FieldScene, now_ms: u32) -> consume::Fed {
    let position = item_position(scene);

    let client = scene.world.actors.viewer.unwrap_or(0);

    let FieldScene {
        panels: Some(panels),
        entered,
        ..
    } = scene
    else {
        return consume::Fed::default();
    };

    let Panels {
        items,
        icons,
        strings,
        cooldowns,
        wire,
        screens,
        ..
    } = panels;

    let fed = consume::feed(entered, items, cooldowns, now_ms, position);

    if let Some(send) = fed.sent {
        wire.push(send.frame(client));
    }

    if let Some(line) = fed.describe() {
        tracing::info!("{line}");
    }

    if fed.sent.is_some()
        && let Some(screen) = screens
            .iter_mut()
            .find(|s| s.content == panels::CONTENT_INVENTORY)
    {
        screens::inventory_refresh(screen, entered, items, icons, strings);
    }

    if let Some(sent) = fed.sent {
        panels::refresh_inventory_hover(panels, entered, sent.slot);

        panels::refine_item_changed(panels, entered);
    }

    fed
}

pub fn map_cycle(scene: &mut FieldScene) {
    let wide = scene.minimap.zoom >= 1.0;

    match (scene.minimap_visible, wide) {
        (false, _) => {
            scene.minimap.zoom = minimap::ZOOM_DEFAULT;

            scene.minimap_visible = true;
        }
        (true, false) => scene.minimap.zoom = minimap::ZOOM_WIDE,
        (true, true) => scene.minimap_visible = false,
    }

    scene.map_moved = true;

    scene.coords_visible = scene.minimap_visible;
}

pub fn start_exit(scene: &mut FieldScene, kind: ExitKind) {
    if kind == ExitKind::ServerSelect {
        start_server_select(scene);

        return;
    }

    if kind == ExitKind::Quit && (scene.exit.is_some() || scene.lock.stamps[0] != 0) {
        return;
    }

    let at_ms = scene.time.now_ms;

    scene.lock.stamp(
        match kind {
            ExitKind::Quit => 0,
            ExitKind::Characters => 1,
            ExitKind::ServerSelect => unreachable!("handled above"),
        },
        at_ms,
    );

    scene.exit = Some(Exit {
        kind,
        at_ms,
        shown: None,
    });

    let mut frame = protocol::client::quit();

    frame[protocol::header::CLIENT_ID_OFF..protocol::header::CLIENT_ID_OFF + 2]
        .copy_from_slice(&scene.entered.client_id.to_le_bytes());

    scene.outbox.push(frame);
}

pub const WINDOW_CLOSE_REPEAT_MS: u32 = 3_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowClose {
    Started,
    Waiting,
    Exit,
}

pub fn request_window_close(scene: &mut FieldScene, now_ms: u32) -> WindowClose {
    let since = scene.lock.stamps[0];

    if since != 0 {
        return if now_ms < since.wrapping_add(WINDOW_CLOSE_REPEAT_MS) {
            WindowClose::Waiting
        } else {
            WindowClose::Exit
        };
    }

    scene.lock.stamp(0, now_ms);

    scene.exit = Some(Exit {
        kind: ExitKind::Quit,
        at_ms: now_ms,
        shown: None,
    });

    WindowClose::Started
}

pub fn start_server_select(scene: &mut FieldScene) {
    let at_ms = scene.time.now_ms;

    scene.lock.stamp(2, at_ms);

    scene.server_select = Some(Exit {
        kind: ExitKind::ServerSelect,
        at_ms,
        shown: None,
    });

    let mut frame = protocol::client::quit();

    let client = scene.entered.client_id.to_le_bytes();

    frame[protocol::header::CLIENT_ID_OFF..protocol::header::CLIENT_ID_OFF + 2]
        .copy_from_slice(&client);

    scene.outbox.push(frame);
}

pub fn stream_zones(scene: &mut FieldScene, game: Option<&GameDir>) -> bool {
    stream_zones_mode(scene, game, false, &mut |_, _| true)
}

pub fn stream_zones_background(scene: &mut FieldScene, game: Option<&GameDir>) -> bool {
    stream_zones_staged(scene, game, &mut |_, _| true)
}

pub(crate) fn stream_zones_staged(
    scene: &mut FieldScene,
    game: Option<&GameDir>,
    stage: &mut impl FnMut(u64, &world::loading::Prepared) -> bool,
) -> bool {
    stream_zones_mode(scene, game, true, stage)
}

fn stream_zones_mode(
    scene: &mut FieldScene,
    game: Option<&GameDir>,
    background: bool,
    stage: &mut impl FnMut(u64, &world::loading::Prepared) -> bool,
) -> bool {
    let Some(game) = game else {
        return false;
    };

    let visual_loaded = if background {
        match scene.world.poll_visual_zone(game, stage) {
            Ok(loaded) => loaded,
            Err(error) => {
                tracing::warn!(% error, "visual terrain preparation failed; will retry");

                false
            }
        }
    } else {
        false
    };

    if background && let Some(zone) = scene.world.force_load {
        if !scene.world.holds(zone) {
            let path = format!("Env/Field{:02}{:02}.trn", zone.0, zone.1);

            if game.contains(path.as_bytes()) {
                if let Err(error) = scene.world.load_zone_background(game, zone, stage) {
                    tracing::warn!(
                        % error, ? zone,
                        "teleport destination preparation failed; will retry"
                    );
                }

                if !scene.world.holds(zone) {
                    return visual_loaded;
                }
            }
        }

        if scene.world.holds(zone) {
            scene.minimap.recentre_background(game, zone);

            if scene.minimap.zone != zone {
                return visual_loaded;
            }
        }
    }

    let mut loaded = force_load(scene, game) | visual_loaded;

    if background {
        loaded |= scene.minimap.recentre_background(game, scene.world.current);
    }

    let mut committed = loaded;

    let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return loaded;
    };

    let position = player.position;

    for _ in 0..world::Direction::ALL.len() {
        let origin = glam::Vec2::new(
            f32::from(scene.world.current.0) * formats::world::ZONE_CELLS as f32,
            f32::from(scene.world.current.1) * formats::world::ZONE_CELLS as f32,
        );

        let offset = glam::Vec2::new(position.x, position.z) - origin;

        let margin =
            world::stream_margin(scene.world.current, glam::Vec2::new(position.x, position.z));

        let mut moved = false;

        for outcome in world::proximity(offset, margin).into_iter().flatten() {
            match outcome {
                world::Proximity::Load(dir) => {
                    match if background {
                        if committed {
                            if visual_loaded {
                                scene.world.restore_cached_stream_collision(dir);
                            }

                            Ok(false)
                        } else {
                            scene.world.stream_background_staged(game, dir, stage)
                        }
                    } else {
                        scene.world.stream(game, dir)
                    } {
                        Ok(added) => {
                            loaded |= added;

                            committed |= added;
                        }
                        Err(error) => {
                            tracing::info!("NWYD: could not stream the {dir:?} zone: {error:#}")
                        }
                    }
                }
                world::Proximity::Cross(dir) => {
                    if !background || !committed {
                        match if background {
                            scene.world.stream_background_staged(game, dir, stage)
                        } else {
                            scene.world.stream(game, dir)
                        } {
                            Ok(added) => {
                                loaded |= added;

                                committed |= added;
                            }
                            Err(error) => {
                                tracing::info!("NWYD: could not stream the {dir:?} zone: {error:#}")
                            }
                        }
                    }

                    if !scene.world.rebase(dir) {
                        continue;
                    }

                    moved = true;

                    let centre = scene.world.current;

                    match if background {
                        Ok(scene.minimap.recentre_background(game, centre))
                    } else {
                        scene.minimap.recentre(game, centre)
                    } {
                        Ok(changed) => loaded |= changed,
                        Err(error) => {
                            tracing::info!("NWYD: could not recentre the minimap: {error:#}")
                        }
                    }
                }
            }
        }

        if !moved {
            break;
        }
    }

    for (row, arrival) in super::init_place::pass(scene, game).unwrap_or_default() {
        tracing::info!("{}", super::init_place::describe(row, &arrival));
    }

    if background && !committed {
        match scene.world.prefetch_visual_zones(
            game,
            glam::Vec2::new(position.x, position.z),
            scene.camera.dist * camera::ORBIT_STRETCH + scene.camera.target_x_offset.abs(),
            (scene.layout.width, scene.layout.height),
            stage,
        ) {
            Ok(added) => loaded |= added,
            Err(error) => {
                tracing::warn!(% error, "visual terrain preparation failed; will retry");
            }
        }
    }

    loaded
}

fn tick_quest_timer(scene: &mut FieldScene, now_ms: u32) {
    if let Some(hud) = scene.hud.as_mut() {
        let environment =
            super::quest_timer::frame_environment(scene.world.env_global, scene.world.force_load);

        let update = super::quest_timer::tick(&mut scene.quest_timer, now_ms, environment);

        hud.refresh_quest_timer(&update);
    }
}

fn force_load(scene: &mut FieldScene, game: &GameDir) -> bool {
    let Some(zone) = scene.world.force_load.take() else {
        return false;
    };

    let restore_yaw = scene.world.force_load_restore_yaw.take();

    let here = player_tile(scene);

    let now_ms = scene.time.now_ms;

    let moved = match scene.world.reanchor(game, zone, here, now_ms) {
        Ok(moved) => moved,
        Err(error) => {
            tracing::info!("NWYD: could not load the zone the warp landed in — {error:#}");

            return false;
        }
    };

    if !moved {
        if !scene.world.holds(zone) {
            tracing::info!(
                "NWYD: warped into block ({}, {}), which the install does not ship — \
                 the world is still Field{:02}{:02}",
                zone.0,
                zone.1,
                scene.world.current.0,
                scene.world.current.1,
            );
        }

        return false;
    }

    scene.camera.yaw = restore_yaw.unwrap_or_else(|| {
        if super::weather::warp_resets_yaw(scene.world.env_global) {
            world::YAW_RESET
        } else {
            scene.camera.yaw
        }
    });

    match scene.minimap.recentre(game, scene.world.current) {
        Ok(_) => {}
        Err(error) => tracing::info!("NWYD: could not recentre the minimap: {error:#}"),
    }

    if let Some(index) = scene.player {
        let field = formats::world::path::HeightField::window(
            &scene.world.actors.window,
            scene.world.actors.zone,
        );

        let actor = &mut scene.world.actors.actors[index];

        let cell = (actor.position.x as i32, actor.position.z as i32);

        actor.position.y = field.ground(cell.0, cell.1);
    }

    tracing::info!(
        "NWYD: force-loaded Field{:02}{:02} for the warp",
        zone.0,
        zone.1,
    );

    true
}

pub fn hear(
    scene: &mut FieldScene,
    line: &protocol::server::ChatLine,
    hide_item_announcement: bool,
) -> Option<chat::Heard> {
    if line.opcode == protocol::server::CHAT_CONSOLE && line.speaker == 0 {
        let mut clock = scene.zone_time.get();

        if clock.receive(&line.text, scene.time.now_ms) {
            scene.zone_time.set(clock);

            return Some(chat::Heard::Dropped("server Nightmare clock consumed"));
        }
    }

    if line.opcode == protocol::server::CHAT_CONSOLE
        && line.speaker == 0
        && scene.affect_time.receive(&line.text)
    {
        return Some(chat::Heard::Dropped("server affect calendar consumed"));
    }

    if line.opcode == protocol::server::CHAT_SHOUT && scene.arena.visible {
        return Some(chat::Heard::Dropped("the arena hides global shouts"));
    }

    if line.opcode == protocol::server::CHAT_CHANNEL
        && scene.arena.visible
        && !line.text.starts_with('=')
    {
        return Some(chat::Heard::Dropped(
            "the arena permits only party channel lines",
        ));
    }

    if line.opcode == protocol::server::CHAT_CONSOLE
        && line.speaker == 0
        && let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i))
    {
        for (at, pattern) in super::firework::console_grid(&line.text, player.position) {
            let shell = super::firework::Firework::new(
                at,
                pattern,
                scene.time.now_ms,
                &mut scene.world.actors.effect_rng,
            );

            scene.world.effects.spawn(Box::new(shell));

            scene.world.play_sound(super::firework::LAUNCH_SOUND);
        }
    }

    if hide_item_announcement {
        return Some(chat::Heard::Dropped("item announcements are hidden"));
    }

    if line.opcode == protocol::server::CHAT_CONSOLE && line.speaker == 0 {
        let player = scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .map(|a| a.position);

        if let Some((text, life)) =
            super::console_notice::banner(&line.text, scene.session.channel, player, false)
            && let Some(panels) = scene.panels.as_mut()
        {
            notice::post(panels, &text, life, true, scene.time.now_ms);
        }
    }

    let speakers = chat::Speakers {
        mobs: scene
            .world
            .actors
            .actors
            .iter()
            .filter(|actor| actor.entity != 0)
            .map(|actor| (actor.entity, actor.name.clone()))
            .collect(),
        party: scene
            .party
            .rows
            .iter()
            .map(|row| (row.id, row.name.clone()))
            .collect(),
    };

    let screens = scene.panels.as_mut()?;

    let heard = chat::receive(screens, line, &speakers, &scene.time.calendar);

    if let Some((speaker, text, life_ms)) = chat::take_balloon(screens) {
        balloon::raise(scene, speaker, &text, life_ms);
    }

    Some(heard)
}

pub fn announce(
    scene: &mut FieldScene,
    message: &protocol::server::SystemMessage,
    hide_item_announcements: bool,
) -> Option<chat::Announced> {
    let now_ms = scene.time.now_ms;

    let world_changed = if chat::ends_khepra(message)
        && let Some(entity) = scene.khepra.take()
        && let Some(index) = scene.world.actors.entity(entity)
    {
        scene.world.actors.actors[index].status.hp = 0;

        super::kill::die(scene, index, now_ms);

        true
    } else {
        false
    };

    let Some(screens) = scene.panels.as_mut() else {
        return world_changed.then(|| chat::Announced {
            record: message.strdef(),
            world_changed,
            ..Default::default()
        });
    };

    let mut announced = chat::announce(
        screens,
        message,
        now_ms,
        &scene.time.calendar,
        hide_item_announcements,
    );

    announced.world_changed = world_changed;

    Some(announced)
}

pub fn take_outbox(scene: &mut FieldScene) -> Vec<Vec<u8>> {
    if let Some(panels) = &mut scene.panels {
        panels.pump_quest(&scene.entered);
    }

    take_pending_outbox(scene)
}

pub fn take_pending_outbox(scene: &mut FieldScene) -> Vec<Vec<u8>> {
    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    let mut out = std::mem::take(&mut scene.outbox);

    let client = scene.world.actors.viewer.unwrap_or(0);

    if let Some(screens) = scene.panels.as_mut() {
        out.append(&mut screens.chat.outbox);

        out.append(&mut screens.wire);

        let items = screens.outbox.drain(..);

        out.extend(items.map(|request| request.to_bytes(client, scene.cargo_npc.unwrap_or(0))));
    }

    out
}

fn drain_store_owner_opened(scene: &mut FieldScene) {
    let Some(title) = scene
        .panels
        .as_mut()
        .and_then(panels::Panels::take_store_owner_opened)
    else {
        return;
    };

    let closing = super::trade::close(scene);

    if let Some(frame) = closing.frame {
        scene.outbox.push(frame);
    }

    if let Some(panels) = scene.panels.as_mut() {
        let _ = super::store::open_owner(panels, &scene.entered, &title);
    }
}

fn drain_guild_fame_accepted(scene: &mut FieldScene) {
    if let Some(stamp) = scene
        .panels
        .as_mut()
        .and_then(|p| p.guild_fame_accepted_ms.take())
    {
        scene.last_interact_ms = stamp;
    }
}

fn drain_guild_application_accepted(scene: &mut FieldScene) {
    let accepted = scene
        .panels
        .as_mut()
        .is_some_and(panels::Panels::take_guild_application_accepted);

    if accepted {
        scene.outbox.push(interact::guild_application_reply(
            scene.guild_application_entity,
        ));
    }
}

fn drain_summon_accepted(scene: &mut FieldScene, now_ms: u32) {
    let accepted = scene
        .panels
        .as_mut()
        .is_some_and(panels::Panels::take_summon_accepted);

    if accepted {
        super::summon::answer(&mut scene.summon, &mut scene.lock, true, now_ms);
    }
}

fn drain_item_skill_accepted(scene: &mut FieldScene, now_ms: u32) {
    if let Some(at) = scene
        .panels
        .as_mut()
        .and_then(panels::Panels::take_item_skill_accepted)
    {
        super::crafting::accept_item_skill(scene, at, now_ms);
    }
}

fn drain_revive_accepted(scene: &mut FieldScene, now_ms: u32) {
    let (death, resurrect) = scene.panels.as_mut().map_or((false, false), |panels| {
        (
            panels.take_death_return_accepted(),
            panels.take_resurrect_accepted(),
        )
    });

    if death {
        scene.revive_pending.arm_death(&mut scene.lock, now_ms);

        scene
            .outbox
            .push(super::pending::death_accept_frame(scene.entered.client_id));
    }

    if resurrect {
        let dead = scene.entered.status.hp < 1
            || scene
                .player
                .is_some_and(|index| scene.world.actors.actors[index].is_dead);

        if dead {
            scene.revive_pending.arm_resurrect(&mut scene.lock, now_ms);
        }
    }
}

fn drain_staged(scene: &mut FieldScene, now_ms: u32) {
    super::firecracker::drain_open(scene);

    super::item_use::ticket_accepts(scene);

    super::seal_name::drain_open(scene);

    let staged: Vec<_> = scene
        .panels
        .as_mut()
        .map(|p| std::mem::take(&mut p.staged))
        .unwrap_or_default();

    for request in staged {
        scene.pending.arm(&mut scene.lock, request, now_ms);

        scene.cc.last_action_ms = now_ms;
    }
}

pub fn tick(scene: &mut FieldScene) -> Option<Transition> {
    advance_bodies(scene);

    refresh_map_title(scene);

    super::store::finish_panel_close(scene);

    super::trade::finish_panel_close(scene);

    let now_ms = scene.time.now_ms;

    if let Some(panels) = scene.panels.as_mut() {
        panels.tick_boxes(now_ms);
    }

    if scene.message_panel.tick(now_ms) {
        tracing::info!("NWYD: the shared message panel expired");
    }

    scene.screen_weather.update(now_ms);

    super::cannon::tick(scene, now_ms);

    super::weather::force_from_actors(
        &mut scene.world.actors.actors,
        &mut scene.world.weather,
        &mut scene.world.env_global,
        now_ms,
    );

    reel_tick(scene, now_ms);

    if let Some(mut exit) = scene.exit.filter(|exit| exit.kind == ExitKind::Quit) {
        if exit.fired(now_ms) {
            scene.exit = None;

            scene.analytics.ended();

            return Some(Transition::Quit);
        }

        let seconds = exit.seconds(now_ms);

        if scene.countdown_quotient != seconds - 1 {
            scene.countdown_quotient = seconds - 1;

            scene.automatic_movement = false;

            exit.shown = Some(seconds);

            let record = exit.record();

            super::message_panel::countdown(scene, record, seconds, now_ms);

            if let Some(hud) = &mut scene.hud {
                hud.toast_record(record, &[seconds as i32]);
            }
        }

        scene.exit = Some(exit);
    }

    if let Some(mut exit) = scene.server_select {
        if exit.fired(now_ms) {
            scene.server_select = None;

            scene.analytics.ended();

            scene.lock.stamps[2] = 0;

            return Some(Transition::ServerSelect);
        }

        let seconds = exit.seconds(now_ms);

        if scene.countdown_quotient != seconds - 1 {
            scene.countdown_quotient = seconds - 1;

            scene.automatic_movement = false;

            exit.shown = Some(seconds);

            super::message_panel::countdown(scene, exit.record(), seconds, now_ms);

            if let Some(hud) = &mut scene.hud {
                hud.toast_record(exit.record(), &[seconds as i32]);
            }
        }

        scene.server_select = Some(exit);
    }

    if let Some(mut exit) = scene.exit.filter(|exit| exit.kind == ExitKind::Characters) {
        if exit.fired(now_ms) {
            scene.exit = None;

            scene
                .outbox
                .push(protocol::client::leave_world(scene.entered.client_id));

            return None;
        }

        let seconds = exit.seconds(now_ms);

        if scene.countdown_quotient != seconds - 1 {
            scene.countdown_quotient = seconds - 1;

            scene.automatic_movement = false;

            exit.shown = Some(seconds);

            super::message_panel::countdown(scene, exit.record(), seconds, now_ms);

            if let Some(hud) = &mut scene.hud {
                hud.toast_record(exit.record(), &[seconds as i32]);
            }
        }

        scene.exit = Some(exit);
    }

    scene.revive_pending.shown = Some(scene.countdown_quotient);

    let death_step = scene.revive_pending.tick_death(&mut scene.lock, now_ms);

    scene.countdown_quotient = scene
        .revive_pending
        .shown
        .unwrap_or(scene.countdown_quotient);

    match death_step {
        super::pending::DeathStep::Nothing => {}
        super::pending::DeathStep::Expired => {
            tracing::info!("NWYD: the death-return stamp expired unarmed")
        }
        super::pending::DeathStep::Tick(seconds) => {
            scene.automatic_movement = false;

            if let Some(at) = scene
                .player
                .and_then(|index| scene.world.actors.actors.get(index))
                .map(|actor| actor.position)
            {
                let at = glam::Vec3::new(at.x, at.y + super::pending::COLUMN_LIFT, at.z);

                scene
                    .world
                    .effects
                    .spawn(Box::new(super::town_portal::TownPortal::new(
                        at,
                        super::pending::COLUMN_KIND,
                        now_ms,
                    )));
            }

            tracing::info!("NWYD: death return in {seconds}");
        }
        super::pending::DeathStep::Fire => {
            scene
                .outbox
                .push(super::pending::death_return_frame(scene.entered.client_id));

            let raised = if let Some(index) = scene.player {
                let actors = &mut scene.world.actors;

                let hidden = actors.actors[index].flags.get(0xB55) != 0
                    || cast::hidden(&actors.actors[index]);

                let at = actors.actors[index].position;

                actors.actors[index].is_dead = false;

                actors.actors[index].play(appearance::ACTION_LEVELUP, now_ms, &actors.models);

                (!hidden).then_some(at)
            } else {
                None
            };

            if let Some(at) = raised {
                super::timers::spawn(scene, at, 0);
            }
        }
    }

    let resurrect_source = super::pending::ResurrectSource {
        client_id: scene.entered.client_id,
        position: scene
            .player
            .and_then(|index| scene.world.actors.actors.get(index))
            .map_or((0.0, 0.0), |actor| (actor.position.x, actor.position.z)),
        from: scene.attack.from,
    };

    scene.revive_pending.shown = Some(scene.countdown_quotient);

    let resurrect_step =
        scene
            .revive_pending
            .tick_resurrect(&mut scene.lock, resurrect_source, now_ms);

    if scene.revive_pending.shown != Some(scene.countdown_quotient) {
        scene.automatic_movement = false;
    }

    scene.countdown_quotient = scene
        .revive_pending
        .shown
        .unwrap_or(scene.countdown_quotient);

    match resurrect_step {
        super::pending::ResurrectStep::Nothing => {}
        super::pending::ResurrectStep::Expired => {
            tracing::info!("NWYD: the Resurrect stamp expired unarmed")
        }
        super::pending::ResurrectStep::Echo(frame) => {
            attack::apply(scene, protocol::server::ATTACK, &frame, now_ms);
        }
        super::pending::ResurrectStep::Fire(frame) => scene.outbox.push(frame),
    }

    drain_staged(scene, now_ms);

    scene.pending.shown = Some(scene.countdown_quotient);

    let pending_step = scene.pending.tick(&mut scene.lock, now_ms);

    scene.countdown_quotient = scene.pending.shown.unwrap_or(scene.countdown_quotient);

    match pending_step {
        super::pending::Step::Nothing => {}
        super::pending::Step::Expired => {
            tracing::info!("NWYD: the pending-action stamp expired unarmed")
        }
        super::pending::Step::Tick(seconds) => {
            scene.automatic_movement = false;

            let local = scene
                .world
                .actors
                .viewer
                .and_then(|id| scene.world.actors.entity(id))
                .map(|index| scene.world.actors.actors[index].position);

            if let Some(at) = local {
                let column = glam::Vec3::new(at.x, at.y + super::pending::COLUMN_LIFT, at.z);

                let portal = super::town_portal::TownPortal::new(
                    column,
                    super::pending::COLUMN_KIND,
                    now_ms,
                );

                scene.world.effects.spawn(Box::new(portal));
            }

            if seconds <= super::pending::FIRE_MS / 1_000 {
                tracing::info!("NWYD: teleport in {seconds}");
            }
        }
        super::pending::Step::Fire(request) => {
            let client = scene.world.actors.viewer.unwrap_or(0);

            scene
                .outbox
                .push(request.to_bytes(client, scene.cargo_npc.unwrap_or(0)));

            tracing::info!("NWYD: the staged 0x373 went out — the teleport scroll is spent");
        }
        super::pending::Step::Transfer(channel) => {
            scene.redirect.arm_transfer(now_ms);

            scene.outbox.push(super::pending::server_frame(
                scene.entered.client_id,
                channel,
            ));

            tracing::info!("NWYD: delayed 0x334 channel transfer sent for row {channel}");
        }
    }

    scene.summon.shown = Some(scene.countdown_quotient);

    let summon_step = scene
        .summon
        .tick(&mut scene.lock, &mut scene.outbox, now_ms);

    scene.countdown_quotient = scene.summon.shown.unwrap_or(scene.countdown_quotient);

    match summon_step {
        super::summon::Step::Nothing => {}
        super::summon::Step::Expired => {
            tracing::info!("NWYD: the summon stamp expired unarmed")
        }
        super::summon::Step::Tick(seconds) => {
            scene.automatic_movement = false;

            let local = scene
                .world
                .actors
                .viewer
                .and_then(|id| scene.world.actors.entity(id))
                .map(|index| scene.world.actors.actors[index].position);

            if let Some(at) = local {
                let column = glam::Vec3::new(at.x, at.y + super::summon::COLUMN_LIFT, at.z);

                let portal =
                    super::town_portal::TownPortal::new(column, super::summon::COLUMN_KIND, now_ms);

                scene.world.effects.spawn(Box::new(portal));
            }

            tracing::info!("NWYD: summon relocation in {seconds}");
        }
        super::summon::Step::Sent { requester } => {
            tracing::info!("NWYD: delayed 0x334 summon relocation sent for {requester:?}")
        }
    }

    scene.command_pending.shown = Some(scene.countdown_quotient);

    let command_step = scene.command_pending.tick(&mut scene.lock, now_ms);

    scene.countdown_quotient = scene
        .command_pending
        .shown
        .unwrap_or(scene.countdown_quotient);

    match command_step {
        super::pending::CommandStep::Nothing => {}
        super::pending::CommandStep::Expired => {
            tracing::info!("NWYD: the delayed-command stamp expired unarmed")
        }
        super::pending::CommandStep::Tick(seconds) => {
            scene.automatic_movement = false;

            super::message_panel::countdown(
                scene,
                super::pending::COMMAND_MESSAGE_RECORD,
                seconds,
                now_ms,
            );

            tracing::info!("NWYD: teleport after {seconds} seconds");
        }
        super::pending::CommandStep::Fire(frame) => {
            scene.outbox.push(frame.to_vec());

            tracing::info!("NWYD: delayed slash-command 0x334 sent");

            return None;
        }
    }

    if scene.world.force_load.is_some() {
        tick_quest_timer(scene, now_ms);

        if let Some(panels) = scene.panels.as_mut() {
            super::panels::refine_tick(panels, &mut scene.entered, now_ms);

            panels.guild_pump(&scene.entered);
        }

        scene.marks.pump();

        return None;
    }

    if let Some(panels) = scene.panels.as_mut() {
        super::panels::refine_tick(panels, &mut scene.entered, now_ms);
    }

    let hunted = cc::tick(scene, now_ms);

    if let Some(actor) = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
    {
        scene.name_display.refresh(actor.position);
    }

    if scene.death_return_birth_ms != 0
        && now_ms.wrapping_sub(scene.death_return_birth_ms) > 180_000
        && scene
            .player
            .and_then(|i| scene.world.actors.actors.get(i))
            .is_some_and(|a| a.is_dead)
    {
        scene
            .outbox
            .push(super::pending::death_return_frame(scene.entered.client_id));

        scene.death_return_birth_ms = 0;
    }

    if let Some(line) = hunted.swung.as_ref().and_then(cc::Swung::describe) {
        tracing::info!("{line}");
    }

    if let Some(line) = cc::describe(&hunted) {
        tracing::info!("{line}");
    }

    super::automatic_move::tick(scene, now_ms);

    super::movement::tick(scene);

    if let Some(line) = super::cast::retry_pickup(scene, now_ms) {
        tracing::info!("{line}");
    }

    if let Some(panels) = scene.panels.as_mut() {
        panels.guild_pump(&scene.entered);
    }

    scene.marks.pump();

    let in_castle_zone = super::city::castle_zone(scene.world.actors.zone);

    match super::siege::zone_tick(&mut scene.siege, in_castle_zone) {
        super::siege::ZoneStep::Nothing => {}
        super::siege::ZoneStep::Raised => {
            tracing::info!(
                "NWYD: castle war raised by the zone block — the ground is a CastleWar pair"
            )
        }
        super::siege::ZoneStep::Lowered => {
            tracing::info!("NWYD: castle war lowered by the zone block — the ground is not a pair")
        }
    }

    let arena_board = scene.arena.visible;

    let ticked = super::event_hud::tick(
        &mut scene.event_hud,
        now_ms,
        scene.world.env_global,
        scene.world.actors.zone,
        arena_board,
    );

    if let Some(line) = super::event_hud::describe_tick(&ticked) {
        tracing::info!("{line}");
    }

    let music_player = scene.player.map(|player| {
        let cell = attack::cell_of(&scene.world.actors.actors[player]);

        let safe = attack::safe_here(scene.world.actors.attributes(), cell);

        (cell, safe)
    });

    let field_music_before_city = scene.city.music_slot;

    let saved = super::city::tick(scene);

    if let Some(line) = super::city::describe(&saved) {
        tracing::info!("{line}");
    }

    if let super::city::Saved::Sent { slot, position, .. } = saved {
        scene.saved_position = Some((slot, position));
    }

    let switched = super::music::tick(scene, music_player, field_music_before_city);

    if let Some(line) = super::music::describe(&switched) {
        tracing::info!("{line}");
    }

    tick_quest_timer(scene, now_ms);

    let cast = super::skill_fx::tick(scene, now_ms);

    if let Some(line) = super::skill_fx::describe(&cast) {
        tracing::info!("{line}");
    }

    let raised = super::cast_fx::build(scene, &cast);

    if let Some(line) = super::cast_fx::describe(&raised) {
        tracing::info!("{line}");
    }

    let swung = attack::auto_tick(scene, now_ms);

    if let Some(line) = attack::describe(&swung) {
        tracing::info!("{line}");
    }

    let stepped = super::gate::tick(scene);

    if let Some(line) = super::gate::describe(&stepped) {
        tracing::info!("{line}");
    }

    let layout = scene.layout;

    scene.damage.tick(now_ms, &layout);

    super::message_panel::tick_pk_entry(scene, now_ms);

    balloon::sweep(scene, now_ms);

    scene.world.objects.roof_hidden = scene.player.is_some_and(|index| {
        let cell = attack::cell_of(&scene.world.actors.actors[index]);

        super::house::roof_hidden(scene.world.actors.attributes(), cell)
    });

    if let Some(index) = scene.player {
        let centre = attack::cell_of(&scene.world.actors.actors[index]);

        for entity in scene.drops.despawn(centre) {
            if scene.hovered_drop == Some(entity) {
                scene.hovered_drop = None;
            }

            if scene.pending_pickup == Some(entity) {
                scene.pending_pickup = None;
            }

            tracing::info!("NWYD: ground item {entity} left the despawn box");
        }
    }

    let centre = scene
        .player
        .map(|index| attack::cell_of(&scene.world.actors.actors[index]));

    for left in scene.gates.tick(scene.time.now_ms, centre) {
        if scene.hovered_drop == Some(left.entity) {
            scene.hovered_drop = None;
        }

        tracing::info!("{}", super::placed::describe_left(&left));
    }

    super::placed::restamp(scene);

    let tile = player_tile(scene);

    scene.world.env_global = weather::env_global_after(scene.world.env_global, tile);

    let env_type = scene.world.env_type;

    let ambient_cycle = scene.world.weather.code == 0 && matches!(env_type, 0 | 3 | 4);

    scene
        .world
        .weather
        .tick(&mut scene.world.sky, env_type, tile, now_ms);

    let blend = scene.world.sky.update_blend(now_ms);

    scene.world.apply_day(blend);

    if let Some(index) = scene.player {
        let at = scene.world.actors.actors[index].position;

        let batch = scene.ambient.tick(
            now_ms,
            ambient_cycle,
            scene.world.env_global,
            at,
            &mut scene.world.actors.effect_rng,
        );

        if batch.ambient {
            for sprite in super::field_ambient::billboards(
                env_type,
                scene.world.sky.day,
                at,
                scene.world.actors.effects_detail.effects,
                &mut scene.world.actors.effect_rng,
            ) {
                scene.world.effects.spawn(Box::new(sprite));
            }
        }

        for seed in batch.seeds {
            scene.world.effects.spawn(Box::new(seed));
        }
    }

    let gusts_eligible = scene.player.is_some_and(|index| {
        let at = scene.world.actors.actors[index].position;

        world::is_special_tile(tile)
            && scene.siege.war_flag == 0
            && scene.world.sky.day == 1
            && !super::snow::roofed(scene.world.actors.attributes(), at.x, at.z)
    });

    if scene
        .screen_weather
        .emit(now_ms, gusts_eligible, &mut scene.world.actors.effect_rng)
    {
        scene
            .unconditional_sounds
            .push(super::audio::UnconditionalSound::new(304));
    }

    let spend_body = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .map(|actor| spend::Raise {
            dead: actor.is_dead,
            absent: false,
        });

    if let Some(screens) = scene.panels.as_mut() {
        notice::tick(screens, now_ms);

        spend::tick(screens, &scene.entered, spend_body, now_ms);

        for slot in std::mem::take(&mut screens.notice.sounds) {
            scene.world.play_sound(slot);
        }
    }

    let long_message = scene.long_message.tick(now_ms);

    if long_message.panel_hidden {
        tracing::info!("NWYD: the 0x7B1 long-message panel expired");
    }

    if long_message.event_rows != 0 {
        tracing::info!(
            "NWYD: 0x7B1 retained-event clock appended {} row(s)",
            long_message.event_rows
        );
    }

    None
}

pub fn has_cp(scene: &FieldScene) -> bool {
    scene.entered.cp > 0
}

pub const PK_MODE: bool = false;

pub const GUILD_CHAT_START: bool = true;

pub const GUILD_MARK_START: bool = false;

pub fn experience(scene: &FieldScene) -> hud::Experience {
    let status = &scene.entered.status;

    let transcendent = scene.entered.equipment.is_transcendent();

    match formats::net::exp::level_span(status.level, transcendent) {
        Some(span) => hud::Experience::new(scene.entered.exp as i64, span),
        None => hud::Experience::default(),
    }
}

fn event_hud_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    out.extend(event_label_quad(scene, false).into_iter().flatten());

    out.extend(event_label_quad(scene, true).into_iter().flatten());

    out
}

fn event_label_quad(scene: &mut FieldScene, clock: bool) -> Option<[PlannedQuad; 2]> {
    let (label, key) = match clock {
        true => (&scene.event_hud.clock, super::event_hud::CLOCK_KEY),
        false => (&scene.event_hud.counter, super::event_hud::COUNTER_KEY),
    };

    if !label.visible || label.text.is_empty() {
        return None;
    }

    let (control, colour, text) = (
        label.rect(),
        label.colour,
        super::stext::display_text(&label.text),
    );

    let (x, y) = super::stext::origin(&scene.layout, (0.0, 0.0), control);

    if !on_screen(
        &scene.layout,
        Rect {
            x,
            y,
            w: control.w,
            h: control.h,
        },
    ) {
        return None;
    }

    if !scene.text.resolve(key, &text, colour) {
        return None;
    }

    let line = scene.text.get(key)?;

    let mut shadow = super::stext::shadow_quad(line, (x, y));

    let mut foreground = super::stext::quad(line, (x, y));

    shadow.dst_w *= 2.0;

    shadow.dst_h *= 2.0;

    foreground.dst_w *= 2.0;

    foreground.dst_h *= 2.0;

    Some([
        PlannedQuad {
            mode: RenderMode::Ui2dPoint,
            texture: TextureKey::SceneText(key),
            quad: shadow,
            uv: UvRule::TexelCentre,
        },
        PlannedQuad {
            mode: S_MODE,
            texture: TextureKey::SceneText(key),
            quad: foreground,
            uv: UvRule::TexelCentre,
        },
    ])
}

fn map_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    if !scene.minimap_visible {
        return Vec::new();
    }

    let Some(map) = live_minimap_rect(scene) else {
        return Vec::new();
    };

    let mut out = Vec::new();

    out.extend(minimap_quad(scene, map));

    out.extend(actor_marker_quads(scene, map));

    out.extend(marker_quads(scene, map));

    out.extend(landmark_text_quads(scene, map));

    out.extend(compass_quad(scene, map));

    out.extend(title_quad(scene, map));

    out.extend(minimap_button_draws(scene));

    out.extend(title_text_quad(scene, map));

    out.extend(coord_text_quad(scene, map));

    out
}

pub fn minimap_button_draws(scene: &FieldScene) -> Vec<PlannedQuad> {
    minimap_button_rects(scene)
        .into_iter()
        .filter_map(|(_, rect)| {
            on_screen(&scene.layout, rect).then_some(PlannedQuad {
                mode: RenderMode::NuiPanel2d,
                texture: TextureKey::White,
                uv: UvRule::TexelCentre,
                quad: Quad {
                    normalized_corners: None,
                    clip: None,
                    dst_x: rect.x,
                    dst_y: rect.y,
                    dst_w: rect.w,
                    dst_h: rect.h,
                    src_x: 0.0,
                    src_y: 0.0,
                    src_w: 1.0,
                    src_h: 1.0,
                    color: 0x01010101,
                    rotation: 0.0,
                },
            })
        })
        .collect()
}

fn minimap_button_rects(scene: &FieldScene) -> Vec<(usize, Rect)> {
    if !scene.minimap_visible {
        return Vec::new();
    }

    let Some(map) = live_minimap_rect(scene) else {
        return Vec::new();
    };

    let (sx, sy) = (scene.layout.sx, scene.layout.sy);

    [0x1653, 0x1652]
        .into_iter()
        .filter_map(|id| {
            let [x, y, w, h] = control_rect(&scene.ui, id)?;

            let (x, y) = if !scene.map_moved {
                (x as f32 * sx, y as f32 * sy)
            } else if scene.minimap.zoom >= 1.0 {
                if id != 0x1653 {
                    return None;
                }

                (map.w - 40.0, 0.0)
            } else {
                if id != 0x1652 {
                    return None;
                }

                (118.0 * sx, -18.0 * sy)
            };

            Some((
                (id - 0x1652) as usize,
                Rect {
                    x: map.x + x,
                    y: map.y + y,
                    w: w as f32 * sx,
                    h: h as f32 * sy,
                },
            ))
        })
        .collect()
}

fn minimap_buttons_mouse(
    scene: &mut FieldScene,
    event: ui::input::Mouse,
    x: i32,
    y: i32,
    now: u32,
) -> bool {
    let mut consumed = false;

    for (index, r) in minimap_button_rects(scene) {
        let hit =
            x as f32 >= r.x && y as f32 >= r.y && (x as f32) < r.x + r.w && (y as f32) < r.y + r.h;

        match event {
            ui::input::Mouse::LeftDown if hit => {
                scene.map_button_pressed[index] = true;

                consumed = true;
            }
            ui::input::Mouse::LeftUp => {
                if hit && scene.map_button_pressed[index] {
                    consumed = true;

                    if scene.map_button_stamps[index].wrapping_add(500) < now {
                        scene
                            .unconditional_sounds
                            .push(super::audio::UnconditionalSound::new(53));

                        scene.map_button_stamps[index] = now;

                        scene.map_button_pressed[index] = false;
                    }
                } else {
                    scene.map_button_pressed[index] = false;
                }
            }
            _ => {}
        }
    }

    consumed
}

pub fn plate(scene: &FieldScene) -> Plate {
    let class = usize::from(scene.entered.class);

    let evolution = cc::evolution_tier(scene);

    let mount =
        super::mount_hp::hud_shown(&scene.entered.equipment).then(|| super::mount_hp::pair(scene));

    let refine = scene
        .entered
        .equipment
        .items
        .get(13)
        .filter(|item| item.index == hud::POISON_AFFECT)
        .map(|item| formats::mesh::appearance::refine_level(item, &scene.statics.items) as i32);

    Plate::new(
        scene.entered.name(),
        &scene.entered.status,
        class,
        evolution,
        refine,
        mount,
    )
}

pub fn minimap_quad(scene: &FieldScene, rect: Rect) -> Option<PlannedQuad> {
    let item = scene.sets.item(-MINIMAP_CONTROL_ID as usize, 0)?;

    let window = scene.minimap.window(scene.target());

    let quad = textured(rect, item, MINIMAP_COLOR, MINIMAP_ROTATION);

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::Scene(minimap::DYNAMIC_SLOT),
        quad: Quad {
            clip: None,
            src_x: window.x as f32,
            src_y: window.y as f32,
            src_w: window.w as f32,
            src_h: window.h as f32,
            ..quad
        },
        uv: UvRule::TexelCentre,
    })
}

pub fn marker_quads(scene: &FieldScene, map: Rect) -> Vec<PlannedQuad> {
    let (sx, sy) = (scene.layout.sx, scene.layout.sy);

    landmarks::place(
        scene.minimap.labels.slots(),
        scene.minimap.zoom,
        scene.target(),
    )
    .into_iter()
    .filter(|p| p.marker)
    .filter_map(|p| marker_quad(scene, map, p, sx, sy))
    .collect()
}

fn actor_marker_quads(scene: &FieldScene, map: Rect) -> Vec<PlannedQuad> {
    let target = scene.target();

    let zoom = scene.minimap.zoom.trunc();

    let centre = if zoom < 1.0 { 74.0 } else { 86.0 };

    let shift = f64::from(zoom) * 120.0;

    scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .filter_map(|(index, actor)| {
            if Some(index) == scene.player {
                return None;
            }

            let color = actor.party_tint?;

            let dx = f64::from(target.x - actor.position.x) * 0.7071070075035095;

            let dz = f64::from(target.z - actor.position.z) * 0.7071070075035095;

            let at = Placed {
                slot: 0,
                x: (centre - (dx - dz) * 2.0 + shift) as f32,
                y: (centre + (dx + dz) * 2.0 + shift) as f32,
                color,
                marker: true,
                label: false,
            };

            marker_quad(scene, map, at, scene.layout.sx, scene.layout.sy)
        })
        .collect()
}

pub fn landmark_text_quads(scene: &mut FieldScene, map: Rect) -> Vec<PlannedQuad> {
    if !scene.minimap_visible {
        return Vec::new();
    }

    let labels = scene.minimap.labels.slots();

    let planned: Vec<_> = landmarks::place(labels, scene.minimap.zoom, scene.target())
        .into_iter()
        .filter(|at| at.label)
        .filter_map(|at| {
            let label = &labels[at.slot];

            let text = super::stext::display_text(&label.name(scene.minimap.label_codepage));

            if text.is_empty() {
                return None;
            }

            let (x, y) = at.label_at(label);

            Some((
                landmarks::LABEL_KEY + at.slot as u32,
                text,
                at.color,
                scene.layout.control(x, y, 8.0, 12.0),
            ))
        })
        .collect();

    planned
        .into_iter()
        .filter_map(|(key, text, colour, control)| {
            let (x, y) = super::stext::origin(&scene.layout, (map.x, map.y), control);

            if !on_screen(
                &scene.layout,
                Rect {
                    x,
                    y,
                    w: control.w,
                    h: control.h,
                },
            ) {
                return None;
            }

            if !scene.text.resolve(key, &text, colour) {
                return None;
            }

            let line = scene.text.get(key)?;

            Some([
                PlannedQuad {
                    mode: RenderMode::Ui2dPoint,
                    texture: TextureKey::SceneText(key),
                    quad: super::stext::shadow_quad(line, (x, y)),
                    uv: UvRule::TexelCentre,
                },
                PlannedQuad {
                    mode: S_MODE,
                    texture: TextureKey::SceneText(key),
                    quad: super::stext::quad(line, (x, y)),
                    uv: UvRule::TexelCentre,
                },
            ])
        })
        .flatten()
        .collect()
}

fn marker_quad(scene: &FieldScene, map: Rect, at: Placed, sx: f32, sy: f32) -> Option<PlannedQuad> {
    let rect = Rect {
        x: map.x + at.x * sx,
        y: map.y + at.y * sy,
        w: landmarks::MARKER.0 * sx,
        h: landmarks::MARKER.1 * sy,
    };

    on_screen(&scene.layout, rect).then_some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::White,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: rect.x,
            dst_y: rect.y,
            dst_w: rect.w,
            dst_h: rect.h,
            src_x: 0.0,
            src_y: 0.0,
            src_w: 1.0,
            src_h: 1.0,
            color: at.color,
            rotation: 0.0,
        },
        uv: UvRule::TexelCentre,
    })
}

pub fn compass_rect(scene: &FieldScene) -> Rect {
    let layout = &scene.layout;

    let (w, h) = (
        layout.sx * landmarks::COMPASS_SIZE.0,
        layout.sy * landmarks::COMPASS_SIZE.1,
    );

    let (x, y) = match (scene.map_moved, scene.minimap.zoom >= 1.0) {
        (false, _) => match control_rect(&scene.ui, landmarks::COMPASS_NAME_ID) {
            Some([x, y, _, _]) => (layout.sx * x as f32, layout.sy * y as f32),
            None => (0.0, 0.0),
        },
        (true, false) => (
            layout.sx * landmarks::COMPASS_CYCLED.0,
            layout.sy * landmarks::COMPASS_CYCLED.1,
        ),
        (true, true) => (
            layout.sx * landmarks::COMPASS_CYCLED_WIDE.0,
            layout.sy * landmarks::COMPASS_CYCLED_WIDE.1,
        ),
    };

    Rect { x, y, w, h }
}

pub fn compass_quad(scene: &FieldScene, map: Rect) -> Option<PlannedQuad> {
    let local = compass_rect(scene);

    child_quad_at(
        scene,
        Rect {
            x: map.x + local.x,
            y: map.y + local.y,
            w: local.w,
            h: local.h,
        },
        landmarks::COMPASS_NAME_ID,
        landmarks::compass_rotation(scene.player_yaw()),
    )
}

pub fn title_quad(scene: &FieldScene, map: Rect) -> Option<PlannedQuad> {
    let bar = title_rect(scene)?;

    child_quad_at(
        scene,
        Rect {
            x: map.x + bar.x,
            y: map.y + bar.y,
            ..bar
        },
        landmarks::TITLE_PANEL_NAME_ID,
        0.0,
    )
}

fn title_rect(scene: &FieldScene) -> Option<Rect> {
    if scene.map_moved && scene.minimap.zoom >= 1.0 {
        return None;
    }

    let [x, y, w, h] = control_rect(&scene.ui, landmarks::TITLE_PANEL_NAME_ID)?;

    let mut rect = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

    if scene.map_moved {
        rect.x = 0.0;

        rect.y = 130.0 * scene.layout.sy;
    }

    Some(rect)
}

pub fn coord_rect(scene: &FieldScene) -> Rect {
    let layout = &scene.layout;

    let (w, h) = (
        layout.sx * landmarks::TEXT_BOX.0 as f32,
        layout.sy * landmarks::TEXT_BOX.1 as f32,
    );

    match (scene.map_moved, scene.minimap.zoom >= 1.0) {
        (false, _) => match control_rect(&scene.ui, landmarks::COORD_NAME_ID) {
            Some([x, y, w, h]) => layout.control(x as f32, y as f32, w as f32, h as f32),
            None => Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            },
        },
        (true, false) => Rect {
            x: layout.sx * landmarks::COORD_CYCLED.0,
            y: layout.sy * landmarks::COORD_CYCLED.1,
            w,
            h,
        },
        (true, true) => Rect {
            x: landmarks::COORD_CYCLED_WIDE.0,
            y: landmarks::COORD_CYCLED_WIDE.1,
            w,
            h,
        },
    }
}

pub fn coord_text_quad(scene: &mut FieldScene, map: Rect) -> Option<PlannedQuad> {
    if !scene.coords_visible {
        return None;
    }

    let text = landmarks::coordinate_text(scene.target());

    scene_text_quad(
        scene,
        map,
        landmarks::COORD_NAME_ID,
        &text,
        coord_rect(scene),
    )
}

pub fn title_text_quad(scene: &mut FieldScene, map: Rect) -> Option<PlannedQuad> {
    refresh_map_title(scene);

    let [x, y, w, h] = control_rect(&scene.ui, landmarks::TITLE_TEXT_NAME_ID)?;

    let bar = title_rect(scene)?;

    let own = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

    let local = Rect {
        x: bar.x + own.x,
        y: bar.y + own.y,
        w: own.w,
        h: own.h,
    };

    let text = scene.map_title_text.clone()?;

    scene_text_quad(scene, map, landmarks::TITLE_TEXT_NAME_ID, &text, local)
}

fn refresh_map_title(scene: &mut FieldScene) {
    if scene.coords_visible {
        scene.map_title_text = Some(scene.session.title());
    }
}

fn scene_text_quad(
    scene: &mut FieldScene,
    map: Rect,
    name_id: u32,
    text: &str,
    control: Rect,
) -> Option<PlannedQuad> {
    let (x, y) = super::stext::origin(&scene.layout, (map.x, map.y), control);

    if !on_screen(
        &scene.layout,
        Rect {
            x,
            y,
            w: control.w,
            h: control.h,
        },
    ) {
        return None;
    }

    if !scene.text.resolve(name_id, text, landmarks::TEXT_COLOR) {
        return None;
    }

    let line = scene.text.get(name_id)?;

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::SceneText(name_id),
        quad: super::stext::quad(line, (x, y)),
        uv: UvRule::TexelCentre,
    })
}

fn child_quad_at(
    scene: &FieldScene,
    rect: Rect,
    name_id: u32,
    rotation: f32,
) -> Option<PlannedQuad> {
    let record = panel(&scene.ui, name_id)?;

    if !on_screen(&scene.layout, rect) {
        return None;
    }

    let item = set_item(&scene.sets, record)?;

    Some(PlannedQuad {
        mode: S_MODE,
        texture: TextureKey::Scene(u32::try_from(item.texture_index).ok()?),
        quad: textured(rect, item, MINIMAP_COLOR, rotation),
        uv: UvRule::TexelCentre,
    })
}

fn set_item<'a>(sets: &'a TextureSetList, record: &Record) -> Option<&'a Item> {
    let control_id = record.control_id()?;

    let set = if control_id < -2 {
        -control_id
    } else {
        control_id
    };

    sets.item(usize::try_from(set).ok()?, 0)
}

fn texture_slot(sets: &TextureSetList, record: &Record) -> Option<u32> {
    u32::try_from(set_item(sets, record)?.texture_index).ok()
}

fn panel(ui: &SceneUi, name_id: u32) -> Option<&Record> {
    ui.records
        .iter()
        .find(|r| r.control == ControlType::Panel && r.header.name_id == name_id)
}

fn control_rect(ui: &SceneUi, name_id: u32) -> Option<[i32; 4]> {
    ui.records
        .iter()
        .find(|r| r.header.name_id == name_id)?
        .rect()
}

fn textured(rect: Rect, item: &Item, color: u32, rotation: f32) -> Quad {
    Quad {
        normalized_corners: None,
        clip: None,
        dst_x: rect.x + item.dst_offset[0] as f32,
        dst_y: rect.y + item.dst_offset[1] as f32,
        dst_w: rect.w,
        dst_h: rect.h,
        src_x: item.x as f32,
        src_y: item.y as f32,
        src_w: item.w as f32,
        src_h: item.h as f32,
        color,
        rotation,
    }
}

pub fn on_screen(layout: &Layout, rect: Rect) -> bool {
    if rect.x > layout.sx * login_static::AUTHORED_WIDTH
        || rect.y > layout.sy * login_static::AUTHORED_HEIGHT
    {
        return false;
    }

    rect.x + rect.w >= 0.0 && rect.y + rect.h >= 0.0
}

pub fn minimap_rect(layout: &Layout, ui: &SceneUi) -> Option<Rect> {
    let record = panel(ui, MINIMAP_NAME_ID)?;

    let [x, y, w, h] = record.rect()?;

    Some(layout.control(x as f32, y as f32, w as f32, h as f32))
}

pub const MAP_SIZE_DEFAULT: f32 = 137.0;

pub const MAP_SIZE_WIDE: f32 = 400.0;

pub const MAP_INSET: f32 = 2.0;

pub const MAP_TOP: f32 = 33.0;

pub const MAP_LIFT: f32 = 15.0;

pub fn map_placement(layout: &Layout, wide: bool) -> Rect {
    let side = if wide {
        MAP_SIZE_WIDE
    } else {
        MAP_SIZE_DEFAULT
    };

    let w = layout.sx * side;

    let h = layout.sy * side;

    let (width, height) = (layout.width as f32, layout.height as f32);

    match wide {
        true => Rect {
            x: (width - w) * 0.5,
            y: (height - h) * 0.5 - MAP_LIFT,
            w,
            h,
        },
        false => Rect {
            x: width - w - MAP_INSET,
            y: MAP_TOP,
            w,
            h,
        },
    }
}

pub fn live_minimap_rect(scene: &FieldScene) -> Option<Rect> {
    match scene.map_moved {
        true => Some(map_placement(&scene.layout, scene.minimap.zoom >= 1.0)),
        false => minimap_rect(&scene.layout, &scene.ui),
    }
}
