pub mod action;
pub mod actor_effects;
pub mod actors;
pub mod affect;
pub mod affect_time;
pub mod arena;
mod arena_draw;
pub mod arrow;
pub mod attack;
pub mod audio;
pub mod aura;
pub mod automatic_move;
pub mod backpack;
pub mod balloon;
pub mod beam;
pub mod bike;
pub mod body_effects;
pub mod body_glow;
pub mod body_plate;
pub mod body_points;
pub mod body_shadows;
pub mod body_sprites;
pub mod burst;
pub mod butterflies;
pub mod camera;
pub mod cannon;
pub mod cape_flag;
pub mod carry;
pub mod cast;
pub mod cast_fx;
pub mod cc;
pub mod challenge;
pub mod char_listener;
pub mod character_name;
pub mod chat;
pub mod city;
pub mod collection;
pub mod combine;
pub mod combo_box;
pub mod connect;
mod console_notice;
pub mod consume;
pub mod corpse;
pub mod crafting;
pub mod create;
pub mod cursor;
pub mod damage;
pub mod delete;
pub mod destroy;
pub mod double_swing;
pub mod drip;
pub mod drop_presentation;
pub mod drops;
pub mod dust;
pub mod effect_context;
pub mod effect_mesh;
pub mod effect_tree;
pub mod effects;
pub mod emitters;
pub mod emote;
pub mod enter;
pub mod equip;
pub mod event_hud;
pub mod field;
pub mod field_ambient;
pub mod field_fx;
pub mod fire;
pub mod fire_ring;
pub mod firecracker;
pub mod firework;
pub mod fish;
pub mod floats;
pub(crate) mod footprints;
pub mod freeze_blade;
pub mod gate;
pub mod glow;
pub mod glow_burst;
pub mod grass;
pub mod ground;
pub mod grouping;
pub mod guild;
pub mod guild_http;
pub mod guild_invite;
pub mod guild_mark;
pub mod guild_panel;
pub mod guild_request;
pub mod hit_reaction;
pub mod house;
pub mod hover;
pub mod hover_tip;
pub mod hud;
mod human_visibility;
pub mod ice_spear;
pub mod ime;
pub(crate) mod impact_delayed;
pub mod impact_particles;
pub mod impact_scene;
pub mod impact_special;
pub mod init_place;
pub mod interact;
pub mod inventory;
pub(crate) mod item_announcements;
pub mod item_move;
pub mod item_preview;
pub mod item_prices;
pub mod item_table;
pub mod item_tip;
pub mod item_use;
pub mod kept;
pub mod keypad;
pub mod kickout;
pub mod kill;
pub mod landmarks;
mod language;
pub mod leave_world;
pub mod legacy_message_box;
mod light_shafts;
pub mod lights;
pub mod listener;
pub mod live;
pub mod lock;
pub mod lod;
pub mod login_panels;
pub mod login_static;
pub mod long_message;
pub mod magic_arrow;
pub mod magic_damage;
pub mod magic_shield;
pub mod memo;
pub mod merge_item;
pub mod mesh_fx;
pub mod message_panel;
pub mod meteor_storm;
pub mod minimap;
pub mod minipopup;
pub mod mission;
pub mod mix;
pub mod motion;
pub mod mount_hp;
mod movement;
pub mod movement_dust;
pub mod msgbox;
pub mod music;
pub mod nameplate;
pub mod notice;
pub mod npc_line;
pub mod objects;
pub mod panels;
pub mod party;
pub mod pending;
pub mod pgt;
pub mod pick;
pub mod placed;
pub mod portal;
pub mod quads;
pub mod quest_timer;
pub mod question;
pub mod quick_bar;
pub mod rain;
pub mod recall;
pub mod redirect;
pub mod reel;
pub mod refine;
pub mod registries;
mod resize;
pub mod rng;
pub mod roster;
pub mod rps;
pub mod scenery_fade;
pub mod screen_weather;
pub mod screens;
pub mod script;
pub mod sea;
pub mod seal_name;
pub mod sel_info;
pub mod select_char;
pub mod server_select;
pub mod shade;
pub mod ships;
pub mod shop;
pub mod siege;
pub mod siege_effects;
pub mod siege_mesh;
pub mod skill;
pub mod skill_anim;
pub mod skill_bar;
pub mod skill_fx;
pub mod skill_master;
pub mod skin_inputs;
pub mod skinfx;
pub mod sky;
pub mod slow_slash;
pub mod snow;
pub mod sp_change;
pub mod spark;
pub mod spawn;
pub mod spend;
pub mod stack_split;
pub mod start;
pub mod status;
pub mod stext;
pub mod store;
pub mod summon;
pub mod sun;
pub mod swing;
pub mod targets;
pub mod terrain_ambient;
pub mod timers;
pub mod tooltip;
pub mod town_portal;
pub mod trade;
pub mod transformation;
pub mod trees;
pub mod vanish;
pub mod vfx;
pub mod view_distance;
pub mod walk;
pub mod warehouse;
pub mod weapon_glow;
pub mod weather;
pub mod world;
pub mod zone_time;

use crate::frame::{PlannedQuad, SceneRenderer};
use render::device::Gpu;

pub enum Scene {
    LoginStatic(login_static::LoginScene),
    LoginPanels(Box<login_panels::LoginPanels>),
    ServerSelect(Box<server_select::ServerSelect>),
    SelectChar(Box<select_char::SelectChar>),
    Field(Box<field::FieldScene>),
}

fn report(actions: Vec<panels::Action>) {
    for action in actions {
        match action {
            panels::Action::Unbuilt(content) => {
                tracing::info!(
                    "NWYD: {} (content {content:#04x}) is not built",
                    panels::screen_name(content)
                );
            }
            panels::Action::Unmodelled(what) => {
                tracing::info!("NWYD: not modelled — {what}")
            }
            other => tracing::info!("NWYD: unhandled {other:?}"),
        }
    }
}

impl Scene {
    pub(crate) fn view_distance(&self) -> Option<view_distance::ViewDistance> {
        match self {
            Scene::ServerSelect(scene) => Some(scene.world.view_distance),
            Scene::SelectChar(scene) => Some(scene.world.view_distance),
            Scene::Field(scene) => Some(scene.world.view_distance),
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => None,
        }
    }

    pub(crate) fn set_view_distance(&mut self, distance: view_distance::ViewDistance) {
        let world = match self {
            Scene::ServerSelect(scene) => &mut scene.world,
            Scene::SelectChar(scene) => &mut scene.world,
            Scene::Field(scene) => &mut scene.world,
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => return,
        };

        world.set_view_distance(distance);
    }

    pub(crate) fn effect_options(&self) -> Option<lod::EffectOptions> {
        match self {
            Scene::ServerSelect(scene) => scene.world.actors.effect_options,
            Scene::SelectChar(scene) => scene.world.actors.effect_options,
            Scene::Field(scene) => scene.world.actors.effect_options,
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => None,
        }
    }

    pub(crate) fn set_effect_options(&mut self, options: lod::EffectOptions) {
        let world = match self {
            Scene::ServerSelect(scene) => &mut scene.world,
            Scene::SelectChar(scene) => &mut scene.world,
            Scene::Field(scene) => &mut scene.world,
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => return,
        };

        world.actors.set_effect_options(options);

        world.effects.old_skill_vfx = options.old_skill_vfx;
    }

    pub fn set_render_lod(&mut self, inputs: lod::Inputs, quality: i16) {
        let world = match self {
            Scene::ServerSelect(scene) => &mut scene.world,
            Scene::SelectChar(scene) => &mut scene.world,
            Scene::Field(scene) => &mut scene.world,
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => return,
        };

        world.actors.render_lod = inputs;

        if let Some(options) = world.actors.effect_options {
            world.actors.effects_detail = options.detail;
        } else {
            world.actors.effects_detail.effects = lod::effects_enabled(quality);
        }
    }

    pub fn mouse(&mut self, event: ui::input::Mouse, x: i32, y: i32) -> Vec<ui::input::Event> {
        if self.legacy_modal() {
            match self {
                Scene::LoginPanels(s) => {
                    s.legacy_boxes.mouse(event, x, y, &s.layout, s.time.now_ms);
                }
                Scene::ServerSelect(s) => {
                    s.panels
                        .legacy_boxes
                        .mouse(event, x, y, &s.layout, s.time.now_ms);
                }
                Scene::SelectChar(s) => {
                    s.legacy_boxes.mouse_with_capture(
                        event,
                        (x, y),
                        &s.layout,
                        s.time.now_ms,
                        s.name_dialog.owns_drag_capture(),
                    );
                }
                Scene::Field(s) => {
                    field::mouse(s, event, x, y);
                }
                _ => {}
            }

            return Vec::new();
        }

        let panels_at = |scale: f32| ((x as f32 / scale) as i32, (y as f32 / scale) as i32);

        match self {
            Scene::LoginStatic(_) => Vec::new(),
            Scene::LoginPanels(scene) => {
                let (x, y) = panels_at(scene.layout.ui_scale());

                scene.mouse(event, x, y)
            }
            Scene::ServerSelect(scene) => {
                let (x, y) = panels_at(scene.layout.ui_scale());

                scene.panels.mouse(event, x, y)
            }
            Scene::SelectChar(scene) => {
                scene.pointer(Some((x, y)));

                let (x, y) = panels_at(scene.layout.ui_scale());

                scene.panels.mouse(event, x, y)
            }
            Scene::Field(scene) => {
                field::mouse(scene, event, x, y);

                Vec::new()
            }
        }
    }

    pub fn poll_status(&mut self) -> bool {
        match self {
            Scene::LoginStatic(_) => false,
            Scene::LoginPanels(scene) => login_panels::poll_status(scene),
            Scene::ServerSelect(scene) => login_panels::poll_status(&mut scene.panels),
            Scene::SelectChar(_) => false,
            Scene::Field(scene) => field::poll_channel_status(scene),
        }
    }

    pub fn cache_redirect(
        &mut self,
        ticket: protocol::server::Redirect,
    ) -> Option<redirect::Cached> {
        match self {
            Scene::Field(scene) => Some(field::cache_redirect(scene, ticket)),
            _ => None,
        }
    }

    pub fn redirect_after_disconnect(
        &mut self,
        socket_closed: bool,
        now_ms: u32,
    ) -> Option<field::RedirectFollow> {
        match self {
            Scene::Field(scene) => field::redirect_after_disconnect(scene, socket_closed, now_ms),
            _ => None,
        }
    }

    pub fn status_timeout(&self) -> Option<std::time::Duration> {
        match self {
            Scene::LoginPanels(scene) => scene.status_timeout,
            Scene::ServerSelect(scene) => scene.panels.status_timeout,
            Scene::Field(scene) => scene.aggregate_fetch.timeout,
            Scene::LoginStatic(_) | Scene::SelectChar(_) => None,
        }
    }

    pub fn allow_status_fetch(&mut self, timeout: std::time::Duration) {
        match self {
            Scene::LoginStatic(_) => {}
            Scene::LoginPanels(scene) => scene.status_timeout = Some(timeout),
            Scene::ServerSelect(scene) => scene.panels.status_timeout = Some(timeout),
            Scene::SelectChar(_) => {}
            Scene::Field(scene) => scene.aggregate_fetch.timeout = Some(timeout),
        }
    }

    pub fn character(&mut self, ch: char) -> Vec<ui::text_entry::Raised> {
        if let Scene::SelectChar(s) = self {
            s.sync_name_focus();
        }

        if self.legacy_character(ch) {
            return Vec::new();
        }

        match self {
            Scene::LoginStatic(_) => Vec::new(),
            Scene::LoginPanels(scene) => scene.character(ch),
            Scene::ServerSelect(scene) => scene.panels.character(ch),
            Scene::SelectChar(scene) => {
                if scene.name_dialog.visible {
                    scene.name_dialog.character(ch)
                } else {
                    scene.panels.character(ch)
                }
            }
            Scene::Field(scene) => {
                if field::air_move_active(scene) {
                    return Vec::new();
                }

                if !field::type_char(scene, ch) && !ch.is_control() {
                    report(field::character(scene, ch));
                }

                Vec::new()
            }
        }
    }

    pub fn modifiers(&mut self, mods: ui::text_entry::Mods) {
        match self {
            Scene::LoginStatic(_) => {}
            Scene::LoginPanels(scene) => scene.manager.keys = mods,
            Scene::ServerSelect(scene) => scene.panels.manager.keys = mods,
            Scene::SelectChar(scene) => scene.panels.manager.keys = mods,
            Scene::Field(scene) => {
                if let Some(hud) = scene.hud.as_mut() {
                    hud.manager.keys = mods;
                }
            }
        }
    }

    pub fn widget_key(&mut self, vk: u32, repeat: bool, now_ms: u32) -> bool {
        combo_box::key_prefix(self, vk);

        if self.legacy_modal() && !matches!(self, Scene::Field(_)) && !self.server_modern_keyboard()
        {
            self.cancel_legacy_presses();

            return true;
        }

        let Some(events) = combo_box::key(self, vk, repeat) else {
            return false;
        };

        if let Scene::Field(scene) = self {
            field::widget_selection(scene, &events, now_ms);

            field::widget_checkable(scene, &events, now_ms);
        } else {
            self.handle(&events, now_ms);
        }

        true
    }

    pub fn widget_wheel(&mut self, delta: i32, lines: u32, now_ms: u32) -> bool {
        if self.legacy_modal() && !matches!(self, Scene::Field(_)) {
            self.cancel_legacy_presses();

            return true;
        }

        let Some(events) = combo_box::wheel(self, delta, lines) else {
            return false;
        };

        if let Scene::Field(scene) = self {
            field::widget_wheel(scene, &events, now_ms);
        } else {
            self.handle(&events, now_ms);
        }

        true
    }

    pub fn widget_space(&mut self, down: bool, now_ms: u32) -> bool {
        if down {
            combo_box::key_prefix(self, 32);
        }

        if self.legacy_modal() && !matches!(self, Scene::Field(_)) && !self.server_modern_keyboard()
        {
            self.cancel_legacy_presses();

            return true;
        }

        let Some(events) = combo_box::checkable_key(self, down) else {
            return false;
        };

        if let Scene::Field(scene) = self {
            field::widget_checkable(scene, &events, now_ms);
        } else {
            self.handle(&events, now_ms);
        }

        true
    }

    pub fn key(
        &mut self,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
    ) -> Vec<ui::text_entry::Raised> {
        self.key_event(key, mods, false)
    }

    pub fn key_event(
        &mut self,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
        repeat: bool,
    ) -> Vec<ui::text_entry::Raised> {
        combo_box::key_prefix(self, combo_box::edit_virtual_key(key));

        if key == ui::text_entry::Key::Escape && matches!(self, Scene::SelectChar(_)) {
            self.legacy_character('\u{1b}');

            return Vec::new();
        }

        if let Scene::SelectChar(s) = self {
            s.sync_name_focus();

            if s.name_dialog.has_focus() && s.legacy_boxes.active().is_some() {
                return s.name_dialog.key(key, mods);
            }
        }

        if self.legacy_modal()
            && let Scene::Field(scene) = self
        {
            if field::air_move_active(scene) {
                return Vec::new();
            }

            if field::edit_key(scene, key, mods) {
                return Vec::new();
            }
        }

        if self.legacy_modal() && !self.server_modern_keyboard() {
            self.cancel_legacy_presses();

            match key {
                ui::text_entry::Key::Return => {
                    self.legacy_character('\r');
                }
                ui::text_entry::Key::Escape => {
                    self.legacy_character('\u{1b}');
                }
                _ => {}
            }

            return Vec::new();
        }

        self.modifiers(mods);

        let now_ms = match self {
            Scene::Field(scene) => scene.time.now_ms,
            _ => 0,
        };

        if combo_box::virtual_key(key).is_some_and(|vk| self.widget_key(vk, false, now_ms)) {
            return Vec::new();
        }

        match self {
            Scene::LoginStatic(_) => Vec::new(),
            Scene::LoginPanels(scene) => scene.key(key, mods),
            Scene::ServerSelect(scene) => scene.panels.key(key, mods),
            Scene::SelectChar(scene) => {
                if scene.name_dialog.visible {
                    scene.name_dialog.key(key, mods)
                } else {
                    scene.panels.key(key, mods)
                }
            }
            Scene::Field(scene) => {
                if field::edit_key(scene, key, mods) {
                    return Vec::new();
                }

                match key {
                    ui::text_entry::Key::Escape => report(field::character(scene, '\u{1b}')),
                    // Retail repeats Enter through character input, preserving modal
                    // priority and chat toggling while the key is held.
                    ui::text_entry::Key::Return => report(field::character(scene, '\r')),
                    ui::text_entry::Key::Tab => field::camera_cycle(scene),
                    ui::text_entry::Key::Delete => field::delete_hud_slot(scene),
                    _ if !repeat => field::unclaimed_key(scene, key),
                    _ => {}
                }

                Vec::new()
            }
        }
    }

    pub fn camera_input(&mut self, mouse: camera::MouseInput) -> bool {
        if self.legacy_modal() {
            return false;
        }

        let Scene::Field(scene) = self else {
            return false;
        };

        let before = scene.camera;

        field::camera_input(scene, mouse);

        before != scene.camera
    }

    pub fn inherit_message(&mut self, previous: &Scene) {
        let (old, strings) = match previous {
            Scene::LoginPanels(s) => (&s.message_panel.state, &s.strings),
            Scene::ServerSelect(s) => (&s.panels.message_panel.state, &s.panels.strings),
            Scene::SelectChar(s) => (&s.message_panel.state, &s.panels.strings),
            Scene::Field(s) => (&s.message_panel, &s.message_strings),
            Scene::LoginStatic(_) => return,
        };

        let next = match self {
            Scene::LoginPanels(s) => &mut s.message_panel.state,
            Scene::ServerSelect(s) => &mut s.panels.message_panel.state,
            Scene::SelectChar(s) => &mut s.message_panel.state,
            Scene::Field(s) => &mut s.message_panel,
            Scene::LoginStatic(_) => return,
        };

        *next = old.clone();

        next.text = Some(message_panel::resolved_text(old, strings));

        next.life_ms = old.life_ms.wrapping_add(6_000);
    }

    pub fn login_accepted(&mut self) {
        match self {
            Scene::ServerSelect(s) => s.panels.message_panel.state.visible = false,
            Scene::LoginPanels(s) => s.message_panel.state.visible = false,
            _ => {}
        }
    }

    pub fn connection_lost_message(&mut self, launcher: bool, now_ms: u32) -> bool {
        if launcher {
            return matches!(self, Scene::SelectChar(_) | Scene::Field(_))
                && self.launcher_message(13);
        }

        let overlay = match self {
            Scene::LoginPanels(scene) => &mut scene.message_panel,
            Scene::ServerSelect(scene) => &mut scene.panels.message_panel,
            Scene::SelectChar(scene) => &mut scene.message_panel,
            Scene::Field(scene) => {
                if scene.message_panel.visible {
                    return false;
                }

                message_panel::post(scene, 13, 4000, now_ms);

                return true;
            }
            Scene::LoginStatic(_) => return false,
        };

        if overlay.state.visible {
            return false;
        }

        overlay.post(13, 4000, now_ms, true);

        true
    }

    pub fn redirect_failed_message(&mut self, now_ms: u32) -> bool {
        let Scene::Field(scene) = self else {
            return false;
        };

        message_panel::post(scene, 8, 4000, now_ms);

        true
    }

    pub fn tick(&mut self) -> Option<field::Transition> {
        let legacy = match self {
            Scene::LoginPanels(s) => s.legacy_boxes.take_transition(),
            Scene::ServerSelect(s) => s.panels.legacy_boxes.take_transition(),
            Scene::SelectChar(s) => s.legacy_boxes.take_transition(),
            Scene::Field(s) => s.legacy_boxes.take_transition(),
            _ => None,
        };

        if legacy.is_some() {
            return legacy;
        }

        match self {
            Scene::Field(scene) => field::tick(scene),
            Scene::SelectChar(scene) => {
                scene.message_panel.state.tick(scene.time.now_ms);

                scene.notice.tick(scene.time.now_ms);

                scene.tick_buttons(scene.time.now_ms);

                scene.tick_create_alt(scene.time.now_ms);

                None
            }
            Scene::LoginPanels(scene) => {
                scene.message_panel.state.tick(scene.time.now_ms);

                scene.notice.tick(scene.time.now_ms);

                scene.tick_login(scene.time.now_ms);

                std::mem::take(&mut scene.quit_requested).then_some(field::Transition::Quit)
            }
            Scene::ServerSelect(scene) => {
                scene.panels.message_panel.state.tick(scene.time.now_ms);

                scene.panels.notice.tick(scene.time.now_ms);

                scene.panels.tick_login(scene.time.now_ms);

                std::mem::take(&mut scene.panels.quit_requested).then_some(field::Transition::Quit)
            }
            _ => None,
        }
    }

    pub fn take_registration_requests(&mut self) -> Vec<String> {
        match self {
            Scene::LoginPanels(scene) => std::mem::take(&mut scene.registration_requests),
            Scene::ServerSelect(scene) => std::mem::take(&mut scene.panels.registration_requests),
            _ => Vec::new(),
        }
    }

    pub fn stream_zones(&mut self, game: Option<&assets::GameDir>) -> bool {
        match self {
            Scene::Field(scene) => field::stream_zones(scene, game),
            _ => false,
        }
    }

    pub fn stream_zones_background(&mut self, game: Option<&assets::GameDir>) -> bool {
        match self {
            Scene::Field(scene) => field::stream_zones_background(scene, game),
            _ => false,
        }
    }

    pub fn legacy_modal(&self) -> bool {
        let boxes = match self {
            Scene::LoginPanels(s) => &s.legacy_boxes,
            Scene::ServerSelect(s) => &s.panels.legacy_boxes,
            Scene::SelectChar(s) => &s.legacy_boxes,
            Scene::Field(s) => &s.legacy_boxes,
            _ => return false,
        };

        boxes.active().is_some()
    }

    pub(crate) fn server_modern_keyboard(&mut self) -> bool {
        match self {
            Scene::LoginPanels(s) => s.modern_keyboard(),
            Scene::ServerSelect(s) => s.panels.modern_keyboard(),
            _ => false,
        }
    }

    pub fn login_denied(&mut self, opcode: u16) -> bool {
        if !matches!(opcode, 0x11C | 0x11D) {
            return false;
        }

        match self {
            Scene::LoginPanels(s) => s.login_denied(),
            Scene::ServerSelect(s) => s.panels.login_denied(),
            _ => return false,
        }

        true
    }

    pub fn launcher_connection_failed(&mut self) -> bool {
        let panels = match self {
            Scene::LoginPanels(s) => s.as_mut(),
            Scene::ServerSelect(s) => &mut s.panels,
            _ => return false,
        };

        panels.legacy_boxes.show(
            legacy_message_box::Owner::Question,
            &panels.strings.translate("ss_8"),
            0x1203,
            None,
        );

        panels.manager.pressed = None;

        panels.manager.capture = None;

        panels.manager.drag = None;

        panels.box_manager.pressed = None;

        panels.box_manager.capture = None;

        panels.box_manager.drag = None;

        true
    }

    pub fn server_return_notice(&mut self, replay: bool) -> bool {
        let (boxes, strings) = match self {
            Scene::LoginPanels(s) => (&mut s.legacy_boxes, &s.strings),
            Scene::ServerSelect(s) => (&mut s.panels.legacy_boxes, &s.panels.strings),
            Scene::SelectChar(s) if !replay => (&mut s.legacy_boxes, &s.panels.strings),
            Scene::Field(s) if !replay => (&mut s.legacy_boxes, &s.message_strings),
            _ => return false,
        };

        boxes.show(
            legacy_message_box::Owner::Question,
            &strings.translate("ss_132"),
            if replay { 0 } else { 0x10153 },
            None,
        );

        self.cancel_legacy_presses();

        true
    }

    fn launcher_message(&mut self, record: usize) -> bool {
        let (boxes, strings, close) = match self {
            Scene::LoginPanels(s) => (&mut s.legacy_boxes, &s.strings, false),
            Scene::ServerSelect(s) => (&mut s.panels.legacy_boxes, &s.panels.strings, false),
            Scene::SelectChar(s) => (&mut s.legacy_boxes, &s.panels.strings, true),
            Scene::Field(s) => (&mut s.legacy_boxes, &s.message_strings, true),
            _ => return false,
        };

        let text = strings.translate(&format!("ss_{record}"));

        boxes.show(legacy_message_box::Owner::Question, &text, 0x10152, None);

        if close {
            boxes.close_after_answer(legacy_message_box::Owner::Question);
        }

        self.cancel_legacy_presses();

        true
    }

    pub fn announce_for_login(
        &mut self,
        message: &protocol::server::SystemMessage,
        launcher: bool,
        hide_item_announcements: bool,
    ) -> Option<chat::Announced> {
        let record = message.strdef();

        if launcher
            && !message.has_argument
            && matches!(message.kind, 0 | 4)
            && matches!(
                record,
                0x46C | 0x46D | 0x46E | 0x489 | 0x4A7 | 0x46A | 0x468 | 0x40B | 0x3FF | 13 | 12 | 8
            )
            && self.launcher_message(record)
        {
            match self {
                Scene::LoginPanels(s) => s.login_reply(),
                Scene::ServerSelect(s) => s.panels.login_reply(),
                _ => {}
            }

            let text = match self {
                Scene::LoginPanels(s) => s.legacy_boxes.states[0].lines.concat(),
                Scene::ServerSelect(s) => s.panels.legacy_boxes.states[0].lines.concat(),
                Scene::SelectChar(s) => s.legacy_boxes.states[0].lines.concat(),
                Scene::Field(s) => s.legacy_boxes.states[0].lines.concat(),
                _ => unreachable!(),
            };

            return Some(chat::Announced {
                record,
                text,
                ..Default::default()
            });
        }

        self.announce(message, hide_item_announcements)
    }

    pub fn cancel_legacy_presses(&mut self) {
        if !self.legacy_modal() {
            return;
        }

        let manager = match self {
            Scene::LoginPanels(s) => Some(&mut s.manager),
            Scene::ServerSelect(s) => Some(&mut s.panels.manager),
            Scene::SelectChar(s) => Some(&mut s.panels.manager),
            Scene::Field(s) => {
                if let Some(p) = &mut s.panels {
                    p.spend_repeat.left_down = false;
                }

                s.hud.as_mut().map(|hud| &mut hud.manager)
            }
            _ => None,
        };

        if let Some(manager) = manager {
            manager.pressed = None;

            manager.capture = None;

            manager.drag = None;
        }
    }

    pub fn legacy_character(&mut self, ch: char) -> bool {
        if let Scene::SelectChar(s) = self {
            if ch == '\u{1b}' {
                s.character_escape();

                return true;
            }

            s.sync_name_focus();

            if s.name_dialog.has_focus() {
                return false;
            }
        }

        if !self.legacy_modal() {
            return false;
        }

        self.cancel_legacy_presses();

        if let Scene::Field(scene) = self {
            if field::air_move_active(scene) {
                return true;
            }

            if !field::type_char(scene, ch) {
                report(field::character(scene, ch));
            }

            return true;
        }

        let server = match self {
            Scene::LoginPanels(s) => Some(s.as_mut()),
            Scene::ServerSelect(s) => Some(s.panels.as_mut()),
            _ => None,
        };

        if let Some(server) = server {
            let modern = server.modern_keyboard();

            if modern || ch == '\u{1b}' {
                server.character(ch);
            }

            if modern {
                return true;
            }
        }

        let boxes = match self {
            Scene::LoginPanels(s) => &mut s.legacy_boxes,
            Scene::ServerSelect(s) => &mut s.panels.legacy_boxes,
            Scene::SelectChar(s) => &mut s.legacy_boxes,
            Scene::Field(s) => &mut s.legacy_boxes,
            _ => return false,
        };

        if boxes.active().is_none() {
            return false;
        }

        boxes.character(ch);

        true
    }

    pub fn set_launcher_login(&mut self, enabled: bool) {
        match self {
            Scene::LoginPanels(s) => s.launcher_login = enabled,
            Scene::ServerSelect(s) => s.panels.launcher_login = enabled,
            Scene::SelectChar(s) => s.launcher_login = enabled,
            _ => {}
        }
    }

    pub fn hear(&mut self, line: &protocol::server::ChatLine) -> Option<chat::Heard> {
        let channel = match self {
            Scene::LoginPanels(s) => s.session.channel.unwrap_or(0) as i32,
            Scene::ServerSelect(s) => s.panels.session.channel.unwrap_or(0) as i32,
            Scene::Field(s) => s.session.channel,
            _ => 0,
        };

        self.hear_on_channel(line, channel, false)
    }

    pub fn hear_on_channel(
        &mut self,
        line: &protocol::server::ChatLine,
        channel: i32,
        hide_item_announcement: bool,
    ) -> Option<chat::Heard> {
        if line.opcode == 0x101 && line.speaker == 0 {
            match self {
                Scene::LoginPanels(s) => {
                    s.login_reply();

                    s.login2.ui.panel.visible = true;
                }
                Scene::ServerSelect(s) => {
                    s.panels.login_reply();

                    s.panels.login2.ui.panel.visible = true;
                }
                _ => {}
            }

            let selection = matches!(self, Scene::SelectChar(_));

            if !hide_item_announcement
                && let Some((text, life)) =
                    console_notice::banner(&line.text, channel, None, selection)
            {
                match self {
                    Scene::LoginPanels(s) => {
                        s.notice.post(&text, life, true, s.time.now_ms);
                    }
                    Scene::ServerSelect(s) => {
                        s.panels.notice.post(&text, life, true, s.time.now_ms);
                    }
                    Scene::SelectChar(s) => {
                        s.notice.post(&text, life, true, s.time.now_ms);
                    }
                    _ => {}
                }
            }
        }

        let heard = match self {
            Scene::Field(scene) => field::hear(scene, line, hide_item_announcement),
            _ if hide_item_announcement => {
                Some(chat::Heard::Dropped("item announcements are hidden"))
            }
            Scene::SelectChar(scene) => scene
                .legacy_boxes
                .selection_console(line)
                .then_some(chat::Heard::Modal),
            _ => None,
        };

        self.cancel_legacy_presses();

        heard
    }

    pub fn announce(
        &mut self,
        message: &protocol::server::SystemMessage,
        hide_item_announcements: bool,
    ) -> Option<chat::Announced> {
        let login_consumed = !message.has_argument
            || !matches!(message.kind, 0 | 3 | 4)
            || !matches!(message.strdef(), 0x61A | 0x61B | 0x40C);

        let announced = match self {
            Scene::Field(scene) => field::announce(scene, message, hide_item_announcements),
            Scene::LoginPanels(scene) => {
                if login_consumed {
                    scene.login_reply();
                }

                Some(scene.notice.announce(
                    &scene.strings,
                    message,
                    scene.time.now_ms,
                    hide_item_announcements,
                ))
            }
            Scene::ServerSelect(scene) => {
                if login_consumed {
                    scene.panels.login_reply();
                }

                Some(scene.panels.notice.announce(
                    &scene.panels.strings,
                    message,
                    scene.time.now_ms,
                    hide_item_announcements,
                ))
            }
            Scene::SelectChar(scene) => Some(scene.notice.announce(
                &scene.panels.strings,
                message,
                scene.time.now_ms,
                hide_item_announcements,
            )),
            _ => None,
        };

        if announced
            .as_ref()
            .is_some_and(|report| report.would_have_exited)
        {
            let request = msgbox::Request::restart_notice();

            let slot = match self {
                Scene::Field(s) => s.panels.as_mut().and_then(|p| p.raise_box(&request, None)),
                Scene::LoginPanels(s) => s.raise_box(&request),
                Scene::ServerSelect(s) => s.panels.raise_box(&request),
                Scene::SelectChar(s) => s.raise(&request),
                _ => None,
            };

            if slot.is_none() {
                let msgbox::Body::Text(text) = &request.body else {
                    unreachable!()
                };

                match self {
                    Scene::Field(s) => {
                        if let Some(p) = s.panels.as_mut() {
                            notice::post(p, text, 10_000, false, s.time.now_ms);
                        }
                    }
                    Scene::LoginPanels(s) => {
                        s.notice.post(text, 10_000, false, s.time.now_ms);
                    }
                    Scene::ServerSelect(s) => {
                        s.panels.notice.post(text, 10_000, false, s.time.now_ms);
                    }
                    Scene::SelectChar(s) => {
                        s.notice.post(text, 10_000, false, s.time.now_ms);
                    }
                    _ => {}
                }
            }
        }

        announced
    }

    pub fn spawn(
        &mut self,
        game: &assets::GameDir,
        opcode: u16,
        entity: formats::net::spawn::Spawn,
    ) -> Option<anyhow::Result<spawn::Spawned>> {
        match self {
            Scene::Field(scene) => Some(spawn::apply(scene, game, opcode, entity)),
            _ => None,
        }
    }

    pub fn ground_item(
        &mut self,
        game: &assets::GameDir,
        packet: &formats::net::ground_item::Drop,
    ) -> Option<anyhow::Result<placed::Arrival>> {
        match self {
            Scene::Field(scene) => Some(placed::apply(scene, game, packet)),
            _ => None,
        }
    }

    pub fn gate_command(
        &mut self,
        packet: &formats::net::gate_command::Command,
    ) -> Option<placed::Commanded> {
        match self {
            Scene::Field(scene) => placed::commanded(scene, packet),
            _ => None,
        }
    }

    pub fn affects(
        &mut self,
        entity: u16,
        affects: &formats::net::affect::Affects,
    ) -> Option<affect::Applied> {
        match self {
            Scene::Field(scene) => Some(affect::apply(scene, entity, affects)),
            _ => None,
        }
    }

    pub fn animation_probe(
        &mut self,
        game: &assets::GameDir,
        request: formats::net::animation_probe::Request,
    ) -> Option<Option<formats::net::animation_probe::Response>> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let response = scene.world.actors.answer_animation_probe(game, request);

        if let Some(response) = response {
            scene.outbox.push(response.to_frame().to_vec());
        }

        Some(response)
    }

    pub fn question(&mut self, question: &formats::net::question::Question) -> bool {
        let Scene::Field(scene) = self else {
            return false;
        };

        scene
            .question
            .replace(question.text.clone(), question.answers.clone());

        true
    }

    pub fn long_message(
        &mut self,
        message: &formats::net::long_message::LongMessage,
    ) -> Option<long_message::Applied> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let [quiz, guild] = scene.long_message_captions.clone();

        let applied =
            scene
                .long_message
                .apply(message, scene.time.now_ms, quiz.as_str(), guild.as_str());

        if applied.chime {
            scene.world.play_sound(long_message::CHIME);
        }

        Some(applied)
    }

    pub fn reel_result(
        &mut self,
        result: &formats::net::reel::ResultMessage,
    ) -> Option<reel::AppliedResult> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let applied = scene.reel.apply_result(result, scene.time.now_ms);

        if let Some(gold) = applied.jackpot_gold {
            scene.entered.gold = scene.entered.gold.wrapping_add(gold);

            field::refresh_screens(scene);
        }

        if let Some(sound) = applied.sound {
            scene
                .unconditional_sounds
                .push(audio::UnconditionalSound::new(sound));
        }

        Some(applied)
    }

    pub fn action(
        &mut self,
        opcode: u16,
        entity: u16,
        act: &formats::net::action::Action,
    ) -> Option<action::Applied> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let pending_move = movement::has_pending_report(&scene.outbox);

        let applied = action::apply_op(scene, opcode, entity, act);

        if pending_move
            && applied.walked
            && applied.index.is_some()
            && applied.index == scene.player
        {
            movement::discard_spatial_outbox(&mut scene.outbox);

            scene.cc.auto_move = act.to();

            scene.sent_dest = Some(act.to());

            scene.attack.remember_move(act);
        }

        if applied.index.is_none() && protocol::server::ASKS_WHO.contains(&opcode) {
            let ask = scene
                .roster
                .saw(entity, roster::FROM_MOVE, scene.time.now_ms);

            if let Some(line) = ask.describe() {
                tracing::info!("{line}");
            }

            if let Some(packet) = ask.packet() {
                scene.outbox.push(packet);
            }
        }

        Some(applied)
    }

    pub fn item_moved(&mut self, dst: (u8, u8), src: (u8, u8)) -> Option<item_move::Swapped> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let field::FieldScene {
            panels, entered, ..
        } = &mut **scene;

        item_move::apply_inbound(panels.as_mut()?, entered, dst, src)
    }

    pub fn shop_closed(&mut self) -> Option<bool> {
        match self {
            Scene::Field(scene) => Some(scene.panels.as_mut().is_some_and(shop::close)),
            _ => None,
        }
    }

    pub fn crafting_closed(&mut self) -> Option<crafting::Closed> {
        match self {
            Scene::Field(scene) => Some(crafting::close(scene)),
            _ => None,
        }
    }

    pub fn shop_stock(&mut self, stock: &formats::net::shop::Stock) -> Option<shop::Opened> {
        match self {
            Scene::Field(scene) => Some(shop::stocked(scene, stock)),
            _ => None,
        }
    }

    pub fn shop_contents(
        &mut self,
        contents: &formats::net::autotrade::Contents,
    ) -> Option<store::Opened> {
        match self {
            Scene::Field(scene) => Some(store::contents(scene, contents)),
            _ => None,
        }
    }

    pub fn shop_slot_sold(&mut self, sold: &formats::net::autotrade::Sold) -> Option<store::Sale> {
        match self {
            Scene::Field(scene) => Some(store::slot_sold(scene, sold)),
            _ => None,
        }
    }

    pub fn sold(
        &mut self,
        game: &assets::GameDir,
        sale: &formats::net::shop::Sale,
    ) -> Option<anyhow::Result<shop::Cleared>> {
        match self {
            Scene::Field(scene) => Some(shop::sold(scene, game, sale)),
            _ => None,
        }
    }

    pub fn vitals(
        &mut self,
        entity: u16,
        vitals: &formats::net::live::Vitals,
    ) -> Option<live::Vitalised> {
        match self {
            Scene::Field(scene) => Some(live::vitals(scene, entity, vitals)),
            _ => None,
        }
    }

    pub fn hp_delta(
        &mut self,
        entity: u16,
        delta: &formats::net::live::HpDelta,
    ) -> Option<live::HpChanged> {
        match self {
            Scene::Field(scene) => Some(live::hp_delta(scene, entity, delta)),
            _ => None,
        }
    }

    pub fn firework(
        &mut self,
        entity: u16,
        glyph: &formats::net::firework::Glyph,
    ) -> Option<live::Sparked> {
        match self {
            Scene::Field(scene) => Some(live::firework(scene, entity, glyph)),
            _ => None,
        }
    }

    pub fn emote(
        &mut self,
        entity: u16,
        packet: &formats::net::emote::Emote,
    ) -> Option<emote::Emoted> {
        match self {
            Scene::Field(scene) => Some(emote::emote(scene, entity, packet)),
            _ => None,
        }
    }

    pub fn numpad(&mut self, vk: u8) -> Option<emote::Press> {
        match self {
            Scene::Field(scene) => Some(field::numpad(scene, vk)),
            _ => None,
        }
    }

    pub fn recall(&mut self) -> Option<recall::Outcome> {
        match self {
            Scene::Field(scene) => Some(field::recall(scene)),
            _ => None,
        }
    }

    pub fn killed(&mut self, kill: &formats::net::kill::Kill) -> Option<kill::Killed> {
        match self {
            Scene::Field(scene) => {
                let now_ms = scene.time.now_ms;

                Some(kill::apply(scene, kill, now_ms))
            }
            _ => None,
        }
    }

    pub fn apply_actor_status(
        &mut self,
        entity: u16,
        score: &formats::net::live::ActorStatus,
    ) -> Option<live::StatusApplied> {
        match self {
            Scene::Field(scene) => Some(live::apply_actor_status(scene, entity, score)),
            _ => None,
        }
    }

    pub fn apply_player_progression(
        &mut self,
        entity: u16,
        score: &formats::net::live::PlayerProgression,
    ) -> Option<live::ProgressionApplied> {
        match self {
            Scene::Field(scene) => Some(live::apply_player_progression(scene, entity, score)),
            _ => None,
        }
    }

    pub fn quick_bar(
        &mut self,
        bar: &[u8; formats::net::enter::QUICK_SLOTS],
    ) -> Option<live::PushedBar> {
        match self {
            Scene::Field(scene) => Some(live::quick_bar(scene, bar)),
            _ => None,
        }
    }

    pub fn gold(&mut self, entity: u16, gold: &formats::net::live::Gold) -> Option<live::Golden> {
        match self {
            Scene::Field(scene) => Some(live::gold(scene, entity, gold)),
            _ => None,
        }
    }

    pub fn name_mark(&mut self, entity: u16, mode: i32) -> Option<live::NameMarked> {
        match self {
            Scene::Field(scene) => Some(live::name_mark(scene, entity, mode)),
            _ => None,
        }
    }

    pub fn plate_level(
        &mut self,
        change: &formats::net::plate::LevelChange,
    ) -> Option<body_plate::LevelApplied> {
        match self {
            Scene::Field(scene) => Some(body_plate::level(scene, change)),
            _ => None,
        }
    }

    pub fn plate_guild(
        &mut self,
        line: &formats::net::plate::GuildLine,
    ) -> Option<body_plate::GuildApplied> {
        match self {
            Scene::Field(scene) => Some(body_plate::guild(scene, line)),
            _ => None,
        }
    }

    pub fn glow_burst(&mut self, entity: u16, mode: i32) -> Option<glow_burst::Burst> {
        match self {
            Scene::Field(scene) => Some(glow_burst::apply(scene, entity, mode)),
            _ => None,
        }
    }

    pub fn guild_invite(
        &mut self,
        entity: u16,
        invite: &formats::net::guild_invite::Invite,
    ) -> Option<guild_invite::Raised> {
        match self {
            Scene::Field(scene) => Some(guild_invite::raise(scene, entity, invite)),
            _ => None,
        }
    }

    pub fn guild_application(&mut self) -> Option<interact::GuildApplicationRaised> {
        match self {
            Scene::Field(scene) => Some(interact::raise_guild_application(scene)),
            _ => None,
        }
    }

    pub fn challenge(
        &mut self,
        entity: u16,
        challenged: &formats::net::challenge::Challenge,
    ) -> Option<challenge::Raised> {
        match self {
            Scene::Field(scene) => Some(challenge::raise(scene, entity, challenged)),
            _ => None,
        }
    }

    pub fn inventory(
        &mut self,
        entity: u16,
        bag: &formats::net::inventory::Bag,
    ) -> Option<inventory::Replaced> {
        match self {
            Scene::Field(scene) => Some(inventory::replace(scene, entity, bag)),
            _ => None,
        }
    }

    pub fn collection(
        &mut self,
        entity: u16,
        progress: &formats::net::collection::Progress,
    ) -> Option<collection::Restated> {
        match self {
            Scene::Field(scene) if scene.world.actors.entity(entity).is_some() => {
                Some(collection::replace(scene, entity, progress))
            }
            _ => None,
        }
    }

    pub fn warehouse_gold(
        &mut self,
        moved: &formats::net::warehouse::Move,
    ) -> Option<warehouse::Moved> {
        match self {
            Scene::Field(scene) => Some(warehouse::moved(scene, moved)),
            _ => None,
        }
    }

    pub fn weather(
        &mut self,
        forecast: &formats::net::weather::Forecast,
        now_ms: u32,
    ) -> Option<weather::Commanded> {
        match self {
            Scene::Field(scene) => Some(weather::changed(scene, forecast, now_ms)),
            _ => None,
        }
    }

    pub fn guild(
        &mut self,
        membership: &formats::net::guild::Membership,
    ) -> Option<guild::Applied> {
        match self {
            Scene::Field(scene) => guild::changed(scene, membership),
            _ => None,
        }
    }

    pub fn final_guild_offline(&mut self) -> Option<guild_panel::ConnectionReport> {
        let Scene::Field(scene) = self else {
            return None;
        };

        guild::report_offline(scene)
    }

    pub fn leave_world(
        &mut self,
        client_id: u16,
        block: &mut formats::net::account::Block,
    ) -> Option<leave_world::Applied> {
        let Scene::Field(scene) = self else {
            return None;
        };

        leave_world::apply(scene, client_id, block)
    }

    pub fn retire_account_experience(
        &self,
        block: &mut formats::net::account::Block,
    ) -> Option<usize> {
        let Scene::Field(scene) = self else {
            return None;
        };

        leave_world::retire_experience(scene, block)
    }

    pub fn siege_state(&mut self, frame: &formats::net::siege::Siege) -> Option<siege::Applied> {
        match self {
            Scene::Field(scene) => Some(siege::apply(scene, frame)),
            _ => None,
        }
    }

    pub fn kickout(
        &mut self,
        frame: &formats::net::kickout::Kickout,
        launcher_login: bool,
        now_ms: u32,
    ) -> Option<kickout::Applied> {
        match self {
            Scene::Field(scene) => Some(kickout::apply(scene, frame, launcher_login, now_ms)),
            _ => None,
        }
    }

    pub fn field_carpet(
        &mut self,
        packet: &formats::net::field_fx::Carpet,
    ) -> Option<field_fx::Carried> {
        match self {
            Scene::Field(scene) => Some(field_fx::raise(scene, packet)),
            _ => None,
        }
    }

    pub fn field_sound(
        &mut self,
        packet: &formats::net::field_fx::Sound,
    ) -> Option<field_fx::Carried> {
        match self {
            Scene::Field(scene) => {
                let carried = field_fx::sound(packet);

                if let field_fx::Carried::Sound { slot } = carried {
                    scene.world.play_sound(slot);
                }

                Some(carried)
            }
            _ => None,
        }
    }

    pub fn field_line(&mut self, line: &formats::net::kept::Line) -> Option<usize> {
        match self {
            Scene::Field(scene) => Some(scene.lines.store(line)),
            _ => None,
        }
    }

    pub fn keyed_record(&mut self, record: formats::net::kept::Record) -> Option<usize> {
        match self {
            Scene::Field(scene) => {
                let slot = scene.records.store(record);

                if let Some(panels) = scene.panels.as_mut() {
                    panels.sealed_records = scene.records.clone();
                }

                Some(slot)
            }
            _ => None,
        }
    }

    pub fn lines(&self) -> Option<&kept::Lines> {
        match self {
            Scene::Field(scene) => Some(&scene.lines),
            _ => None,
        }
    }

    pub fn records(&self) -> Option<&kept::Records> {
        match self {
            Scene::Field(scene) => Some(&scene.records),
            _ => None,
        }
    }

    pub fn arena_round(
        &mut self,
        packet: &formats::net::arena::Round,
        now_ms: u32,
    ) -> Option<arena::Rounded> {
        match self {
            Scene::Field(scene) => Some(arena::round(
                &mut scene.arena,
                &mut scene.event_hud,
                &mut scene.message_panel,
                Some(&scene.message_strings),
                packet,
                now_ms,
            )),
            _ => None,
        }
    }

    pub fn arena_kill(
        &mut self,
        packet: &formats::net::arena::Kill,
        now_ms: u32,
    ) -> Option<arena::Killed> {
        match self {
            Scene::Field(scene) => Some(arena::kill(
                &mut scene.arena,
                &mut scene.message_panel,
                Some(&scene.message_strings),
                packet,
                now_ms,
            )),
            _ => None,
        }
    }

    pub fn arena_list(&mut self, packet: &formats::net::arena::List) -> Option<arena::Listed> {
        match self {
            Scene::Field(scene) => {
                let created = scene
                    .panels
                    .as_ref()
                    .is_some_and(|p| p.screen(panels::CONTENT_ARENA).is_some());

                let strings = scene.panels.as_ref().map(|p| &p.strings);

                let mut listed = arena::list(&mut scene.arena, strings, packet, created);

                if let Some(panels) = scene.panels.as_mut() {
                    panels.arena_cost = scene.arena.cost;
                }

                if let (Some(fill), Some(panels)) = (&listed.fill, scene.panels.as_mut()) {
                    listed.opened = screens::arena_refresh(panels, fill);
                }

                Some(listed)
            }
            _ => None,
        }
    }

    pub fn arena(&self) -> Option<&arena::Board> {
        match self {
            Scene::Field(scene) => Some(&scene.arena),
            _ => None,
        }
    }

    pub fn war_flag(&mut self, flag: formats::net::siege::WarFlag) -> Option<siege::WarApplied> {
        match self {
            Scene::Field(scene) => {
                let in_castle_zone = city::castle_zone(scene.world.actors.zone);

                Some(siege::war(scene, flag, in_castle_zone))
            }
            _ => None,
        }
    }

    pub fn event_countdown(
        &mut self,
        countdown: &formats::net::event_hud::Countdown,
        now_ms: u32,
    ) -> Option<event_hud::Armed> {
        match self {
            Scene::Field(scene) => Some(event_hud::armed(&mut scene.event_hud, countdown, now_ms)),
            _ => None,
        }
    }

    pub fn event_remaining(
        &mut self,
        remaining: &formats::net::event_hud::Remaining,
        now_ms: u32,
    ) -> Option<event_hud::Wrote> {
        match self {
            Scene::Field(scene) => Some(event_hud::remaining(
                &mut scene.event_hud,
                remaining,
                now_ms,
            )),
            _ => None,
        }
    }

    pub fn event_packed(
        &mut self,
        packed: &formats::net::event_hud::Packed,
        now_ms: u32,
    ) -> Option<event_hud::Wrote> {
        match self {
            Scene::Field(scene) => Some(event_hud::packed(&mut scene.event_hud, packed, now_ms)),
            _ => None,
        }
    }

    pub fn event_target(
        &mut self,
        target: &formats::net::event_hud::Target,
    ) -> Option<event_hud::Wrote> {
        match self {
            Scene::Field(scene) => {
                let layout = scene.layout;

                Some(event_hud::target(&mut scene.event_hud, target, &layout))
            }
            _ => None,
        }
    }

    pub fn event_score(
        &mut self,
        score: &formats::net::event_hud::Score,
    ) -> Option<event_hud::Wrote> {
        match self {
            Scene::Field(scene) => {
                let layout = scene.layout;

                Some(event_hud::score(&mut scene.event_hud, score, &layout))
            }
            _ => None,
        }
    }

    pub fn summon_requested(
        &mut self,
        request: &protocol::server::SummonRequest,
    ) -> Option<summon::Notice> {
        match self {
            Scene::Field(scene) => Some(summon::requested(&mut scene.summon, request)),
            _ => None,
        }
    }

    pub fn summon_cancelled(&mut self) -> Option<summon::Notice> {
        match self {
            Scene::Field(scene) => Some(summon::cancelled(&mut scene.summon)),
            _ => None,
        }
    }

    pub fn item_arrived(
        &mut self,
        arrival: &formats::net::backpack::Arrival,
    ) -> Option<backpack::Arrived> {
        match self {
            Scene::Field(scene) => Some(backpack::arrived(scene, arrival)),
            _ => None,
        }
    }

    pub fn item_cleared(
        &mut self,
        clear: &formats::net::backpack::Clear,
    ) -> Option<backpack::Cleared> {
        match self {
            Scene::Field(scene) => Some(backpack::cleared(scene, clear)),
            _ => None,
        }
    }

    pub fn party_member(
        &mut self,
        opcode: u16,
        member: &formats::net::party::Member,
    ) -> Option<party::Change> {
        let Scene::Field(scene) = self else {
            return None;
        };

        Some(match opcode {
            formats::net::party::INVITED => party::Change::Invited(party::invited(scene, member)),
            _ => party::Change::Joined(party::joined(scene, member)),
        })
    }

    pub fn party_left(&mut self, leave: &formats::net::party::Leave) -> Option<party::Leaving> {
        match self {
            Scene::Field(scene) => Some(party::leaving(scene, leave)),
            _ => None,
        }
    }

    pub fn party_marks(&mut self, marks: &formats::net::party::Marks) -> Option<party::Marked> {
        match self {
            Scene::Field(scene) => Some(party::marked(scene, marks)),
            _ => None,
        }
    }

    pub fn shop_sell_list(&mut self, list: &formats::net::shop::SellList) -> Option<shop::Stocked> {
        match self {
            Scene::Field(scene) => Some(shop::sell_list(scene, list)),
            _ => None,
        }
    }

    pub fn npc_line(
        &mut self,
        entity: u16,
        line: &formats::net::npc_line::NpcLine,
    ) -> Option<npc_line::Applied> {
        match self {
            Scene::Field(scene) => Some(npc_line::apply(scene, entity, line)),
            _ => None,
        }
    }

    pub fn trade_offer(
        &mut self,
        entity: u16,
        offer: &formats::net::trade::Offer,
    ) -> Option<trade::Offered> {
        match self {
            Scene::Field(scene) => Some(trade::offer(scene, entity, offer)),
            _ => None,
        }
    }

    pub fn trade_closed(&mut self, entity: u16) -> Option<trade::Closed> {
        match self {
            Scene::Field(scene) => Some(trade::closed(scene, entity)),
            _ => None,
        }
    }

    pub fn trade_toggle_for(&mut self, entity: u16) -> Option<trade::Toggled> {
        let Scene::Field(scene) = self else {
            return None;
        };

        scene.world.actors.entity(entity)?;

        Some(trade::toggle(scene))
    }

    pub fn trade_toggle(&mut self) -> Option<trade::Toggled> {
        match self {
            Scene::Field(scene) => Some(trade::toggle(scene)),
            _ => None,
        }
    }

    pub fn equip_changed(
        &mut self,
        game: &assets::GameDir,
        entity: u16,
        change: &formats::net::live::EquipChange,
    ) -> Option<anyhow::Result<live::Redressed>> {
        match self {
            Scene::Field(scene) => Some(live::equip(scene, game, entity, change)),
            _ => None,
        }
    }

    pub fn redress_player(&mut self, game: &assets::GameDir) -> Option<anyhow::Result<bool>> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let index = scene.player?;

        Some(live::redress_player(scene, game, index))
    }

    pub fn selection_slot_patch(
        &self,
        warehouse: Option<&mut enter::Warehouse>,
        patch: &formats::net::live::SlotPatch,
    ) -> Option<bool> {
        if !matches!(self, Scene::SelectChar(_)) {
            return None;
        }

        if patch.container != formats::net::live::Container::Cargo {
            return Some(false);
        }

        let Some(index) = patch.index() else {
            return Some(false);
        };

        let Some(warehouse) = warehouse else {
            return Some(false);
        };

        warehouse.cargo[index] = patch.item;

        Some(true)
    }

    pub fn slot_patch(
        &mut self,
        game: &assets::GameDir,
        entity: u16,
        patch: &formats::net::live::SlotPatch,
    ) -> Option<anyhow::Result<live::Patched>> {
        match self {
            Scene::Field(scene) => Some(live::slot_patch(scene, game, entity, patch)),
            _ => None,
        }
    }

    pub fn attacked(
        &mut self,
        opcode: u16,
        swing: &formats::net::attack::Attack,
    ) -> Option<attack::Applied> {
        match self {
            Scene::Field(scene) => {
                let now_ms = scene.time.now_ms;

                Some(attack::apply(scene, opcode, swing, now_ms))
            }
            _ => None,
        }
    }

    pub fn remove(&mut self, entity: u16) -> Option<action::Removed> {
        match self {
            Scene::Field(scene) => Some(action::remove(scene, entity)),
            _ => None,
        }
    }

    pub fn vanish(
        &mut self,
        entity: u16,
        packet: &formats::net::live::Vanish,
    ) -> Option<vanish::Vanished> {
        match self {
            Scene::Field(scene) => {
                let now_ms = scene.time.now_ms;

                Some(vanish::apply(scene, entity, packet, now_ms))
            }
            _ => None,
        }
    }

    pub fn answered(&mut self, entity: u16) {
        if let Scene::Field(scene) = self {
            scene.roster.answered(entity);
        }
    }

    pub fn take_outbox(&mut self) -> Vec<Vec<u8>> {
        match self {
            Scene::Field(scene) => field::take_outbox(scene),
            _ => Vec::new(),
        }
    }

    pub fn take_pending_outbox(&mut self) -> Vec<Vec<u8>> {
        match self {
            Scene::Field(scene) => field::take_pending_outbox(scene),
            _ => Vec::new(),
        }
    }

    pub(crate) fn discard_spatial_outbox(&mut self) {
        if let Scene::Field(scene) = self {
            if movement::has_pending_report(&scene.outbox)
                && let Some(actor) = scene
                    .player
                    .and_then(|index| scene.world.actors.actors.get(index))
            {
                let at = attack::cell_of(actor);

                scene.cc.auto_move = at;

                scene.sent_dest = Some(at);

                scene.attack.from = Some((at.0 as u16, at.1 as u16));
            }

            movement::discard_spatial_outbox(&mut scene.outbox);
        }
    }

    pub fn take_quick_slot_save(&mut self) -> Option<Vec<u8>> {
        match self {
            Scene::Field(scene) => scene.quick_bar.save.take(),
            _ => None,
        }
    }

    pub fn take_music(&mut self) -> Option<usize> {
        match self {
            Scene::Field(scene) => scene.pending_music.take(),
            _ => None,
        }
    }

    pub fn music_follows_outbox(&self) -> bool {
        match self {
            Scene::Field(scene) => {
                scene.siege.at_war() && scene.pending_music == Some(music::CASTLE_WAR as usize)
            }
            _ => false,
        }
    }

    pub fn take_saved_position(&mut self) -> Option<(usize, (u16, u16))> {
        match self {
            Scene::Field(scene) => scene.saved_position.take(),
            _ => None,
        }
    }

    pub fn zone_report(&self) -> Option<String> {
        let Scene::Field(scene) = self else {
            return None;
        };

        let world = &scene.world;

        let held: Vec<&str> = std::iter::once(world.name.as_str())
            .chain(world.streamed.iter().map(|s| s.name.as_str()))
            .collect();

        Some(format!(
            "streamed — standing in Field{:02}{:02}, holding {} ({} quads, {} instance(s), {} tile(s))",
            world.current.0,
            world.current.1,
            held.join(" + "),
            world.mesh.quad_count(),
            world.objects.instances.len(),
            world.tiles.len(),
        ))
    }

    pub fn handle(&mut self, events: &[ui::input::Event], now_ms: u32) -> Vec<listener::Refusal> {
        match self {
            Scene::LoginStatic(_) => Vec::new(),
            Scene::LoginPanels(scene) => listener::handle(scene, events, now_ms),
            Scene::ServerSelect(scene) => listener::handle(&mut scene.panels, events, now_ms),
            Scene::SelectChar(_) | Scene::Field(_) => Vec::new(),
        }
    }

    pub fn submit(&mut self) -> Vec<ui::input::Event> {
        match self {
            Scene::LoginPanels(scene) => scene.submit().into_iter().collect(),
            Scene::ServerSelect(scene) => scene.panels.submit().into_iter().collect(),
            Scene::LoginStatic(_) | Scene::SelectChar(_) | Scene::Field(_) => Vec::new(),
        }
    }

    pub fn viewport(&self) -> (u32, u32) {
        match self {
            Scene::LoginStatic(scene) => (scene.layout.width, scene.layout.height),
            Scene::LoginPanels(scene) => (scene.layout.width, scene.layout.height),
            Scene::ServerSelect(scene) => (scene.layout.width, scene.layout.height),
            Scene::SelectChar(scene) => (scene.layout.width, scene.layout.height),
            Scene::Field(scene) => (scene.layout.width, scene.layout.height),
        }
    }

    pub fn ui_scale(&self) -> f32 {
        match self {
            Scene::LoginStatic(scene) => scene.layout.ui_scale(),
            Scene::LoginPanels(scene) => scene.layout.ui_scale(),
            Scene::ServerSelect(scene) => scene.layout.ui_scale(),
            Scene::SelectChar(scene) => scene.layout.ui_scale(),
            Scene::Field(scene) => scene.layout.ui_scale(),
        }
    }

    pub fn upload(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        match self {
            Scene::LoginStatic(scene) => login_static::upload(scene, gpu, renderer),
            Scene::LoginPanels(scene) => login_panels::upload(scene, gpu, renderer),
            Scene::ServerSelect(scene) => server_select::upload(scene, gpu, renderer),
            Scene::SelectChar(scene) => select_char::upload(scene, gpu, renderer),
            Scene::Field(scene) => field::upload(scene, gpu, renderer),
        }
    }

    pub fn sync(&mut self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        match self {
            Scene::LoginStatic(_) => {}
            Scene::LoginPanels(scene) => login_panels::sync(scene, gpu, renderer),
            Scene::ServerSelect(scene) => server_select::sync(scene, gpu, renderer),
            Scene::SelectChar(scene) => select_char::sync(scene, gpu, renderer),
            Scene::Field(scene) => field::sync(scene, gpu, renderer),
        }
    }

    pub fn frame_draws(&mut self) -> Vec<PlannedQuad> {
        self.frame_draws_with_cursor(true)
    }

    fn frame_draws_with_cursor(&mut self, include_cursor: bool) -> Vec<PlannedQuad> {
        let mut out = Vec::new();

        self.frame_draws_into(include_cursor, &mut out);

        out
    }

    pub fn frame_draws_into(&mut self, include_cursor: bool, out: &mut Vec<PlannedQuad>) {
        out.clear();

        match self {
            Scene::LoginStatic(scene) => {
                login_static::append_frame_draws(scene, include_cursor, out)
            }
            Scene::LoginPanels(scene) => {
                login_panels::append_frame_draws(scene, include_cursor, out)
            }
            Scene::ServerSelect(scene) => {
                server_select::append_frame_draws(scene, include_cursor, out)
            }
            Scene::SelectChar(scene) => select_char::append_frame_draws(scene, out),
            Scene::Field(scene) => field::append_frame_draws(scene, out),
        }
    }

    pub fn queue_world(&mut self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        let viewport = self.viewport();

        let (world, now_ms, frame, markers, drops, drop_glows, drop_lights) = match self {
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => return,
            Scene::Field(scene) => {
                let now_ms = scene.time.now_ms;

                let frame = field::world_frame(scene, viewport);

                let markers = field::click_markers(scene, now_ms);

                let mut drops = field::drop_draws(scene);

                drops.extend(
                    field::gate_draws(scene)
                        .iter()
                        .map(placed::GateDraw::as_object),
                );

                let glows = drop_presentation::ground_glows(scene);

                let lights = drop_presentation::festive_lights(scene);

                (
                    &mut scene.world,
                    now_ms,
                    frame,
                    markers,
                    drops,
                    glows,
                    lights,
                )
            }
            Scene::ServerSelect(scene) => {
                let now_ms = scene.time.now_ms;

                let frame = scene.world.frame(now_ms, viewport);

                (
                    &mut scene.world,
                    now_ms,
                    frame,
                    field::MarkerDraws::default(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            }
            Scene::SelectChar(scene) => {
                let now_ms = scene.time.now_ms;

                let frame = select_char::world_frame(scene, viewport);

                (
                    &mut scene.world,
                    now_ms,
                    frame,
                    field::MarkerDraws::default(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            }
        };

        let skin_frame = world.skin_frame(&frame).with_scroll(now_ms);

        let alpha = renderer.skin_alpha_reference();

        let vendor = renderer.vendor();

        let scratch = std::mem::take(&mut world.actor_scratch);

        let mut actors =
            world.actor_frame_reusing(now_ms, &frame.view.matrix(), vendor, alpha, scratch);

        renderer.upload_world(gpu, world);

        let object_frustum = frame.object_frustum();

        let objects = world.object_frame_with(
            vendor,
            &actors.weapons,
            &drops,
            now_ms,
            &markers.meshes,
            &object_frustum,
        );

        renderer.temporal_scene_stamp(world.identity, now_ms);

        renderer.queue_world_at_with_history(
            gpu,
            frame.uniforms,
            &objects,
            now_ms,
            &actors.weapons,
        );

        renderer.set_terrain_enabled(world.terrain_draw_enabled());

        renderer.queue_seas(gpu, &world.sea_uniforms(&frame), &world.sea_frame());

        renderer.queue_skins(gpu, &skin_frame, &actors);

        let reach = world.pose_reach();

        let effects = &mut world.effect_scratch;

        world.effects.frame_into(effects, reach, now_ms);

        world.sky_ornaments.submit(
            world.sky,
            world.scene,
            world.environment.env_type,
            &world.effects,
            reach.eye,
            now_ms,
            effects,
        );

        body_shadows::submit(&world.actors, &world.effects, reach.eye, now_ms, effects);

        body_sprites::submit(&world.actors, &world.effects, reach.eye, now_ms, effects);

        drop_presentation::append_glows(&world.effects, effects, &drop_glows, reach, now_ms);

        drop_presentation::append_lights(&world.effects, effects, &drop_lights, reach, now_ms);

        world
            .effects
            .append_quads_of(effects, &markers.quads, now_ms);

        world.effects.append_trails_of(effects, &actors.trails);

        let view = frame.view.matrix();

        let fall = weather::Fall {
            emitters: world.weather.emitters,
            rain: &world.rain,
            snow: [&world.snow, &world.snow_fast],
            anchor: world.weather_anchor,
            map: world.actors.attributes(),
            view: &view,
        };

        world.effects.append_weather_of(effects, &fall);

        world.effects.append_drips(effects, &world.drips);

        renderer.queue_effects(gpu, &frame.uniforms, effects);

        actors.clear();

        world.actor_scratch = actors;
    }

    pub fn describe(&self) -> String {
        match self {
            Scene::LoginStatic(_) => "logos".to_string(),
            Scene::LoginPanels(_) => "logos + panels".to_string(),
            Scene::ServerSelect(scene) => {
                format!("{} (bg {})", scene.world.name, scene.world.variant)
            }
            Scene::SelectChar(scene) => {
                format!("{} ({:?})", scene.world.name, scene.mode)
            }
            Scene::Field(scene) => {
                format!("{} ({})", scene.world.name, scene.entered.name())
            }
        }
    }

    pub fn shown_at(&mut self, now_ms: u32) {
        self.set_now_ms(now_ms);

        match self {
            Scene::ServerSelect(scene) => scene.world.restart_camera(now_ms),
            Scene::SelectChar(scene) => scene.world.restart_camera(now_ms),
            Scene::Field(scene) => {
                scene.world.restart_camera(now_ms);

                if scene.camera.shake.is_some_and(|shake| shake.strength == 10) {
                    scene.camera.start_shake(10, now_ms);
                }
            }
            Scene::LoginStatic(_) | Scene::LoginPanels(_) => {}
        }
    }

    pub fn set_now_ms(&mut self, now_ms: u32) {
        match self {
            Scene::LoginStatic(scene) => scene.set_now_ms(now_ms),
            Scene::LoginPanels(scene) => {
                scene.time.now_ms = now_ms;

                scene.s_controls.set_now_ms(now_ms);
            }
            Scene::ServerSelect(scene) => {
                scene.time.now_ms = now_ms;

                scene.panels.time.now_ms = now_ms;

                scene.panels.s_controls.set_now_ms(now_ms);
            }
            Scene::SelectChar(scene) => scene.time.now_ms = now_ms,
            Scene::Field(scene) => scene.time.now_ms = now_ms,
        }
    }

    pub fn take_sounds(&mut self) -> Vec<i64> {
        match self {
            Scene::ServerSelect(scene) => {
                let mut sounds = scene.world.take_sounds();

                sounds.extend(scene.panels.message_panel.take_sounds());

                sounds.append(&mut scene.panels.notice.state.sounds);

                sounds
            }
            Scene::SelectChar(scene) => {
                let mut sounds = scene.world.take_sounds();

                sounds.extend(scene.message_panel.take_sounds());

                sounds.append(&mut scene.notice.state.sounds);

                sounds
            }
            Scene::LoginPanels(scene) => {
                let mut sounds = scene.message_panel.take_sounds();

                sounds.append(&mut scene.notice.state.sounds);

                sounds
            }
            Scene::Field(scene) => scene.world.take_sounds(),
            _ => Vec::new(),
        }
    }

    pub fn take_unconditional_sounds(&mut self) -> Vec<audio::UnconditionalSound> {
        let mut sounds = Vec::new();

        let manager = match self {
            Scene::LoginStatic(_) => None,
            Scene::LoginPanels(scene) => {
                sounds.extend(scene.legacy_boxes.take_click_sounds());

                Some(&mut scene.manager)
            }
            Scene::ServerSelect(scene) => {
                sounds.extend(scene.panels.legacy_boxes.take_click_sounds());

                Some(&mut scene.panels.manager)
            }
            Scene::SelectChar(scene) => {
                sounds.extend(scene.legacy_boxes.take_click_sounds());

                sounds.extend(scene.name_dialog.take_click_sounds());

                Some(&mut scene.panels.manager)
            }
            Scene::Field(scene) => {
                sounds.append(&mut scene.unconditional_sounds);

                sounds.extend(scene.legacy_boxes.take_click_sounds());

                if let Some(panels) = scene.panels.as_mut() {
                    sounds.extend(
                        panels
                            .unconditional_sounds
                            .drain(..)
                            .map(audio::UnconditionalSound::new),
                    );
                }

                scene.hud.as_mut().map(|hud| &mut hud.manager)
            }
        };

        if let Some(manager) = manager {
            sounds.extend(
                manager
                    .button_sounds
                    .drain(..)
                    .map(audio::UnconditionalSound::new),
            );
        }

        sounds
    }

    pub fn take_damage_sounds(&mut self) -> Vec<skill_anim::DamageSoundRequest> {
        match self {
            Scene::Field(scene) => scene.world.take_damage_sounds(),
            _ => Vec::new(),
        }
    }

    pub fn finish_damage_sound(
        &mut self,
        request: skill_anim::DamageSoundRequest,
        played: bool,
    ) -> bool {
        match self {
            Scene::Field(scene) => scene.world.finish_damage_sound(request, played),
            _ => false,
        }
    }

    pub fn queue_frame(&mut self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        self.queue_frame_inner(gpu, renderer, true);
    }

    pub fn queue_frame_with_pointer(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SceneRenderer,
        pointers: &cursor::Pointers,
        frame: u32,
        position: Option<(i32, i32)>,
    ) {
        self.queue_frame_under_overlay(gpu, renderer, position);

        pointers.queue(gpu, renderer, self.viewport(), frame, position);
    }

    pub(crate) fn queue_frame_under_overlay(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SceneRenderer,
        position: Option<(i32, i32)>,
    ) {
        if let Scene::SelectChar(scene) = self {
            scene.pointer(position);
        }

        self.queue_frame_inner(gpu, renderer, false);
    }

    fn queue_frame_inner(&mut self, gpu: &Gpu, renderer: &mut SceneRenderer, include_cursor: bool) {
        self.for_each_ui(|ui, now| ui.begin_frame_at(now));

        if let Self::SelectChar(scene) = self {
            scene.sync_name_focus();

            scene.name_dialog.begin_frame(scene.captions);
        }

        renderer.begin();

        self.queue_world(gpu, renderer);

        let mut draws = std::mem::take(&mut renderer.quad_scratch);

        self.frame_draws_into(include_cursor, &mut draws);

        self.sync(gpu, renderer);

        let preview = if let Self::Field(scene) = self {
            scene
                .panels
                .as_ref()
                .and_then(|panels| panels.tooltip.queue_preview(gpu, renderer))
        } else {
            None
        };

        for (index, quad) in draws.iter().enumerate() {
            if let Some((after, range)) = &preview
                && *after == index
            {
                renderer.push_ui_model(range.clone());
            }

            renderer.push(quad);
        }

        if let Some((after, range)) = preview
            && after == draws.len()
        {
            renderer.push_ui_model(range);
        }

        draws.clear();

        renderer.quad_scratch = draws;

        self.for_each_ui(|ui, now| ui.finish_frame(now));
    }

    fn for_each_ui(&mut self, mut visit: impl FnMut(&mut crate::ui::PanelUi, u32)) {
        match self {
            Self::LoginStatic(_) => {}
            Self::LoginPanels(scene) => {
                let now = scene.time.now_ms;

                scene.for_each_ui(&mut |ui| visit(ui, now));
            }
            Self::ServerSelect(scene) => {
                let now = scene.time.now_ms;

                scene.panels.for_each_ui(&mut |ui| visit(ui, now));
            }
            Self::SelectChar(scene) => {
                let now = scene.time.now_ms;

                for pane in &mut scene.panels.panes {
                    visit(&mut pane.ui, now);
                }

                scene.panels.msgbox.for_each_ui(&mut |ui| visit(ui, now));

                visit(&mut scene.notice.placed.ui, now);
            }
            Self::Field(scene) => {
                let now = scene.time.now_ms;

                if let Some(hud) = &mut scene.hud {
                    visit(&mut hud.bar.ui, now);

                    visit(&mut hud.plate.ui, now);

                    visit(&mut hud.target.ui, now);
                }

                if let Some(panels) = &mut scene.panels {
                    panels.for_each_ui(&mut |ui| visit(ui, now));
                }
            }
        }
    }
}
