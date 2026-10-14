use crate::fonts;
use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::scene::login_static::{self, Layout, LoginPresentation, LoginScene, SceneTime};
use crate::scene::{listener, msgbox, status};
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::config::{config_bin::Language, serverlist, sn};
use formats::text::Codepage;
use formats::texture::{self, Texture};
use render::device::Gpu;
use render::preset::RenderMode;
use render::texture::AlphaPolicy;
use std::collections::BTreeMap;
use ui::Strings;
use ui::draw::Interaction;
use ui::input::{Event, EventId, Manager, Mouse, PanelMut, PanelRef};

pub const PANEL_LOGIN: &[u8] = br"UI\Panel\login\GameLogin.pane";

pub const PANEL_LOGIN2: &[u8] = br"UI\Panel\login\GameLogin2.pane";

pub const PANEL_COPYRIGHT: &[u8] = br"UI\Panel\login\GameLogin3.pane";

pub const FIELD_ACCOUNT: &str = "id";

pub const FIELD_PASSWORD: &str = "password";

#[path = "login_aggregate.rs"]
pub mod aggregate;

pub const NUI_MODE: RenderMode = RenderMode::NuiPanel2d;

pub const BOX_CAPTION_ID: PanelId = 0x0001_0001;

pub const OFFLINE_TAG_PASS: &[(&str, bool)] = &[
    ("loginbg", false),
    ("loginserver", true),
    ("loginidpwd", false),
];

#[cfg(test)]
pub(crate) mod language_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Form {
    ServerSelect,
    Login,
}

#[derive(Debug)]

pub struct PlacedPanel {
    pub ui: PanelUi,
    pub origin: (i32, i32),
}

#[derive(Debug)]

pub struct GuiTexture {
    pub slot: i32,
    pub path: String,
    pub color_key: i32,
    pub keyed_texels: usize,
    pub texture: Texture,
}

#[derive(Debug)]

pub struct LoginPanels {
    pub legacy_boxes: super::legacy_message_box::Dialogs,
    pub notice: super::notice::Overlay,
    pub message_panel: super::message_panel::Overlay,
    pub layout: Layout,
    pub time: SceneTime,
    pub s_controls: LoginScene,
    pub login: PlacedPanel,
    pub login2: PlacedPanel,
    login_fields: [ui::element::Instance; 2],
    login_button: ui::element::Instance,
    login_attempt_ms: u32,
    pub copyright: PlacedPanel,
    pub form: Form,
    pub captions: bool,
    pub guimat_textures: BTreeMap<i32, GuiTexture>,
    pub missing: Vec<i32>,
    pub notes: Vec<String>,
    pub manager: Manager,
    pub names: sn::ServerNames,
    pub servers: serverlist::ServerList,
    pub bounds: listener::Bounds,
    pub strings: Strings,
    pub status: status::Status,
    pub channels: Vec<listener::Channel>,
    pub session: listener::Session,
    pub last_login_ms: u32,
    pub clipboard: ui::text_entry::Local,
    pub status_timeout: Option<std::time::Duration>,
    pub status_rx: Option<std::sync::mpsc::Receiver<(usize, status::Status)>>,
    status_tx: Option<std::sync::mpsc::Sender<(usize, status::Status)>>,
    pub showing: Option<usize>,
    pub status_for: Option<usize>,
    pub status_requested: Vec<(usize, String)>,
    pub aggregate: aggregate::State,
    pub boxes: msgbox::Boxes,
    pub msgbox: msgbox::BoxPanel,
    pub box_manager: Manager,
    pub quit_requested: bool,
    pub settings_requested: bool,
    pub registration_requests: Vec<String>,
    pub launcher_login: bool,
    pub launcher_handoff: bool,
}

#[derive(Debug)]

pub struct LanguageUpdate {
    game: GameDir,
    strings: Strings,
    notice: super::notice::LanguageUpdate,
    server_labels: Vec<(i64, String)>,
    channel_labels: Vec<(usize, String)>,
}

impl LanguageUpdate {
    pub fn prepare(scene: &LoginPanels, game: &GameDir, strings: &Strings) -> Result<Self> {
        let mut server_labels = Vec::new();

        for (slot, &value) in scene.names.slots.iter().enumerate() {
            if value < 1 {
                continue;
            }

            let record = value as usize - 1;

            let Some(label) = scene.names.labels.get(record) else {
                continue;
            };

            let label = if label.is_empty() {
                strings
                    .translate("ss_66")
                    .replacen("%d", &(record + 1).to_string(), 1)
            } else {
                label.clone()
            };

            server_labels.push((slot as i64, label));
        }

        server_labels.push((
            listener::SETTINGS_ROW,
            match game.language() {
                Language::Portuguese => "Configurações",
                _ => "Settings",
            }
            .to_string(),
        ));

        let channel_labels = scene
            .showing
            .map(|record| {
                let missing = status::Status::default();

                let status = if scene.status_for == Some(record) {
                    &scene.status
                } else {
                    &missing
                };

                listener::channels(
                    &listener::RowInput {
                        table: &scene.servers,
                        names: &scene.names,
                        status,
                        strings,
                        bounds: scene.bounds,
                        w_day: scene.time.calendar.server_day_of_month as i32,
                        epoch_day: scene.time.calendar.epoch_day,
                    },
                    record,
                )
                .into_iter()
                .map(|row| (row.index, row.caption))
                .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            game: game.clone(),
            strings: strings.clone(),
            notice: super::notice::LanguageUpdate::prepare(&scene.notice, game)?,
            server_labels,
            channel_labels,
        })
    }
}

impl LoginPanels {
    pub fn apply_language(&mut self, update: LanguageUpdate) {
        let LanguageUpdate {
            game,
            strings,
            notice,
            server_labels,
            channel_labels,
        } = update;

        for placed in [&mut self.login, &mut self.login2, &mut self.copyright] {
            placed.ui.relocalize(&game, &strings);
        }

        self.msgbox.relocalize(&game, &strings);

        self.legacy_boxes.relocalize(&game, &strings);

        self.notice.relocalize(&game, &strings);

        self.notice.apply_language(notice);

        self.message_panel.relocalize(&game, &strings);

        if let Some(list) = self
            .login
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|element| element.name == listener::SERVER_VIEW)
            .and_then(|element| element.widget.list_mut())
        {
            for row in &mut list.items {
                if let Some((_, label)) = server_labels
                    .iter()
                    .find(|(payload, _)| *payload == row.payload)
                {
                    for child in &mut row.children {
                        if child.name == "Name" {
                            child.set_display(label);
                        }
                    }
                }
            }
        }

        for channel in &mut self.channels {
            if let Some((_, caption)) = channel_labels
                .iter()
                .find(|(index, _)| *index == channel.index)
            {
                channel.caption = caption.clone();
            }
        }

        if let Some(list) = self
            .login
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|element| element.name == listener::CHANNEL_VIEW)
            .and_then(|element| element.widget.list_mut())
        {
            for (row, channel) in list.items.iter_mut().zip(&self.channels) {
                for child in &mut row.children {
                    if child.name == "Name" {
                        child.set_display(&channel.caption);
                    }
                }
            }
        }

        self.strings = strings;
    }

    pub(super) fn resize(&mut self, chains: &crate::fonts::Chains, size: (u32, u32)) {
        let next = Layout::new(size.0, size.1);

        self.s_controls.resize(size);

        self.legacy_boxes.resize(chains, &self.layout, &next);

        self.message_panel.resize(chains, size.0);

        self.notice.resize(&next);

        self.msgbox.reposition(&next);

        for placed in [&mut self.login, &mut self.login2, &mut self.copyright] {
            placed.ui.scale = next.ui_scale();

            placed.origin = placed.ui.reposition(next.virtual_screen());
        }

        self.manager.screen = next.virtual_screen();

        self.box_manager.screen = next.virtual_screen();

        self.layout = next;
    }

    fn login_enabled(&mut self, button: bool, password: Option<bool>) {
        for e in &mut self.login2.ui.panel.elements {
            if e.instance == self.login_button {
                e.enabled = button;
            }

            if e.instance == self.login_fields[1]
                && let Some(enabled) = password
            {
                e.enabled = enabled;
            }
        }
    }

    pub(super) fn login_started(&mut self, now: u32) {
        self.login_enabled(false, Some(false));

        self.login_attempt_ms = now;
    }

    pub fn tick_login(&mut self, now: u32) {
        if self.splash_visible() {
            return;
        }

        if now.wrapping_sub(self.login_attempt_ms) > 6000 {
            self.login_enabled(true, Some(true));
        }
    }

    pub fn login_connection_failed(&mut self) {
        self.login_enabled(true, None);
    }

    pub(super) fn login_reply(&mut self) {
        self.login_enabled(true, Some(true));
    }

    pub fn login_denied(&mut self) {
        let text = self.strings.translate("ss_12");

        if self.launcher_login {
            self.legacy_boxes.show(
                super::legacy_message_box::Owner::Question,
                &text,
                0x10152,
                None,
            );

            self.manager.pressed = None;

            self.manager.capture = None;

            self.manager.drag = None;

            self.box_manager.pressed = None;

            self.box_manager.capture = None;

            self.box_manager.drag = None;
        } else {
            self.notice.post(&text, 4000, true, self.time.now_ms);

            self.login_connection_failed();

            self.login2.ui.panel.visible = true;
        }
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut PanelUi)) {
        visit(&mut self.login.ui);

        visit(&mut self.login2.ui);

        visit(&mut self.copyright.ui);

        self.msgbox.for_each_ui(visit);

        visit(&mut self.notice.placed.ui);
    }

    fn draw_order(&mut self) -> [&mut PlacedPanel; 3] {
        [&mut self.login, &mut self.login2, &mut self.copyright]
    }

    pub fn panel_refs(&self) -> Vec<PanelRef<'_>> {
        [&self.login, &self.login2, &self.copyright]
            .into_iter()
            .map(|p| PanelRef {
                panel: &p.ui.panel,
                origin: p.origin,
            })
            .collect()
    }

    pub fn mouse(&mut self, event: Mouse, x: i32, y: i32) -> Vec<Event> {
        if self.msgbox.placed.ui.panel.visible {
            let placed = &mut self.msgbox.placed;

            let mut refs = [PanelMut {
                panel: &mut placed.ui.panel,
                origin: &mut placed.origin,
            }];

            let events = self.box_manager.mouse(&mut refs, event, x, y);

            for event in events {
                if event.id == EventId::Click {
                    let yes = match event.name.as_str() {
                        msgbox::ELEMENT_OK => true,
                        msgbox::ELEMENT_CANCEL => false,
                        _ => continue,
                    };

                    if let Some(slot) = self.msgbox.slot {
                        self.answer_box(slot, yes);
                    }
                }
            }

            return Vec::new();
        }

        let mut manager = std::mem::take(&mut self.manager);

        let mut refs = [
            PanelMut {
                panel: &mut self.login.ui.panel,
                origin: &mut self.login.origin,
            },
            PanelMut {
                panel: &mut self.login2.ui.panel,
                origin: &mut self.login2.origin,
            },
            PanelMut {
                panel: &mut self.copyright.ui.panel,
                origin: &mut self.copyright.origin,
            },
        ];

        let events = manager.mouse(&mut refs, event, x, y);

        if let Some(target) = manager.capture {
            let panel = match target.panel {
                0 => Some(&mut self.login),
                1 => Some(&mut self.login2),
                2 => Some(&mut self.copyright),
                _ => None,
            };

            if let Some(panel) = panel {
                panel
                    .ui
                    .edit_pointer(target.owner, event, x - panel.origin.0);
            }
        }

        self.manager = manager;

        events
    }

    pub fn focus_login_field(&mut self) {
        let index = self.field_index(FIELD_ACCOUNT);

        self.set_focus(index.map(|i| ui::input::Target {
            panel: 1,
            owner: ui::draw::Owner::Element(i),
        }));
    }

    fn set_focus(&mut self, target: Option<ui::input::Target>) {
        self.manager.set_focus(
            target,
            [
                (0, &mut self.login.ui.panel),
                (1, &mut self.login2.ui.panel),
                (2, &mut self.copyright.ui.panel),
            ],
        );
    }

    pub fn field_text(&self, name: &str) -> String {
        self.field_index(name)
            .and_then(|i| self.login2.ui.panel.elements.get(i))
            .and_then(|e| e.widget.edit())
            .map(|edit| edit.text.clone())
            .unwrap_or_default()
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut ui::widget::EditState> {
        let index = self.field_index(name)?;

        self.login2
            .ui
            .panel
            .elements
            .get_mut(index)
            .and_then(|e| e.widget.edit_mut())
    }

    fn field_index(&self, name: &str) -> Option<usize> {
        let retained = match name {
            FIELD_ACCOUNT => Some(self.login_fields[0]),
            FIELD_PASSWORD => Some(self.login_fields[1]),
            _ => None,
        };

        self.login2
            .ui
            .panel
            .elements
            .iter()
            .position(|e| retained.map_or(e.name == name, |owner| e.instance == owner))
    }

    fn panes_mut(&mut self) -> [&mut PlacedPanel; 3] {
        [&mut self.login, &mut self.login2, &mut self.copyright]
    }

    pub fn key_prefix(&mut self, vk: u32) {
        if self.msgbox.placed.ui.panel.visible {
            ui::key_prefix::visit_manager(
                &mut [&mut self.msgbox.placed.ui.panel],
                &[0],
                &self.box_manager,
                vk,
            );

            return;
        }

        let mut panes = [
            &mut self.login.ui.panel,
            &mut self.login2.ui.panel,
            &mut self.copyright.ui.panel,
        ];

        let mut order = [0, 1, 2];

        order.sort_by_key(|&i| panes[i].z_order);

        ui::key_prefix::visit_manager(&mut panes, &order, &self.manager, vk);
    }

    pub fn focused_edit(&mut self) -> Option<&mut ui::widget::EditState> {
        if self.msgbox.placed.ui.panel.visible {
            return None;
        }

        let target = self.manager.focused?;

        let pane = match target.panel {
            0 => &mut self.login,
            1 => &mut self.login2,
            2 => &mut self.copyright,
            _ => return None,
        };

        self.manager.reconcile_panel(target.panel, &pane.ui.panel);

        let target = self.manager.focused?;

        ui::input::owner_element_mut(&mut pane.ui.panel, target.owner)?
            .0
            .widget
            .edit_mut()
    }

    pub(crate) fn modern_keyboard(&mut self) -> bool {
        if self.msgbox.placed.ui.panel.visible {
            return true;
        }

        if self.focused_edit().is_none() {
            return false;
        }

        let Some(target) = self.manager.focused else {
            return false;
        };

        let panel = match target.panel {
            0 => &self.login.ui.panel,
            1 => &self.login2.ui.panel,
            2 => &self.copyright.ui.panel,
            _ => return false,
        };

        panel.visible
            && ui::input::owner_element(panel, target.owner).is_some_and(|(e, _)| {
                e.visible && e.enabled && self.login_fields.contains(&e.instance)
            })
    }

    pub fn character(&mut self, ch: char) -> Vec<ui::text_entry::Raised> {
        if self.msgbox.placed.ui.panel.visible {
            if let Some(raised) = self.msgbox.character(ch, &mut self.clipboard) {
                return raised;
            }

            self.box_key(ch);

            return Vec::new();
        }

        if ch == '\u{1b}' {
            self.escape();

            return Vec::new();
        }

        let mut clipboard = std::mem::take(&mut self.clipboard);

        let raised = match self.focused_edit() {
            Some(edit) => ui::text_entry::character(edit, ch, &mut clipboard),
            None => Vec::new(),
        };

        self.clipboard = clipboard;

        raised
    }

    pub fn key(
        &mut self,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
    ) -> Vec<ui::text_entry::Raised> {
        self.key_prefix(super::combo_box::edit_virtual_key(key));

        if self.msgbox.placed.ui.panel.visible {
            if let Some(raised) = self.msgbox.key(key, mods, &mut self.clipboard) {
                return raised;
            }

            if key == ui::text_entry::Key::Return {
                self.box_key('\r');
            }

            return Vec::new();
        }

        if key == ui::text_entry::Key::Escape {
            return self.character('\u{1b}');
        }

        let mut clipboard = std::mem::take(&mut self.clipboard);

        let raised = match self.focused_edit() {
            Some(edit) => ui::text_entry::key(edit, key, mods, &mut clipboard),
            None => {
                if key == ui::text_entry::Key::Tab {
                    self.focus_next(mods.shift);
                }

                Vec::new()
            }
        };

        self.clipboard = clipboard;

        if raised.contains(&ui::text_entry::Raised::FocusNext) {
            self.focus_tab();
        }

        raised
    }

    fn escape(&mut self) {
        let consumes = if self.legacy_boxes.active().is_some() {
            self.modern_keyboard()
        } else {
            self.focused_edit().is_some()
        };

        if consumes {
            return;
        }

        let (context, record) = match self.form {
            Form::ServerSelect => (0x0c, 22),
            Form::Login if !self.launcher_login => (0x0d, 2),
            Form::Login => return,
        };

        self.raise_box(&msgbox::Request::server_select(
            context,
            msgbox::Body::StrDef(record),
        ));
    }

    pub fn focus_tab(&mut self) {
        let name = self.focused_name().to_owned();

        self.focus_tab_from(&name);
    }

    pub fn focus_tab_from(&mut self, name: &str) {
        self.manager.reconcile_panel(0, &self.login.ui.panel);

        self.manager.reconcile_panel(1, &self.login2.ui.panel);

        self.manager.reconcile_panel(2, &self.copyright.ui.panel);

        if !self.login.ui.panel.allow_focus {
            return;
        }

        let to = match name == FIELD_ACCOUNT {
            true => FIELD_PASSWORD,
            false => FIELD_ACCOUNT,
        };

        let Some(index) = self.field_index(to) else {
            return;
        };

        if !ui::input::tab_stop(&self.login2.ui.panel.elements[index]) {
            return;
        }

        self.manager.request_focus(
            Some(ui::input::Target {
                panel: 1,
                owner: ui::draw::Owner::Element(index),
            }),
            [
                (0, &mut self.login.ui.panel),
                (1, &mut self.login2.ui.panel),
                (2, &mut self.copyright.ui.panel),
            ],
        );
    }

    pub fn focus_next(&mut self, backwards: bool) {
        let Some(target) = self.manager.focused else {
            return;
        };

        let ui::draw::Owner::Element(from) = target.owner else {
            return;
        };

        let Some(pane) = self.panes_mut().into_iter().nth(target.panel) else {
            return;
        };

        let elements = &pane.ui.panel.elements;

        let n = elements.len();

        if from >= n {
            return;
        }

        let order: Vec<usize> = if backwards {
            (0..from).rev().take(0xffff).collect()
        } else {
            (from + 1..n).take(0xffff).collect()
        };

        if let Some(next) = order
            .into_iter()
            .find(|&i| ui::input::tab_stop(&elements[i]))
        {
            self.set_focus(Some(ui::input::Target {
                panel: target.panel,
                owner: ui::draw::Owner::Element(next),
            }));

            let element = &mut self.panes_mut()[target.panel].ui.panel.elements[next];

            if matches!(
                element.element_type,
                formats::ui::pane::ElementType::EditBox
                    | formats::ui::pane::ElementType::ImeEditBox
            ) && let Some(edit) = element.widget.edit_mut()
                && !edit.text.is_empty()
            {
                edit.anchor = 0;

                edit.caret = edit.text.chars().count() as i32;
            }
        }
    }

    pub fn focused_name(&self) -> String {
        let Some(target) = self.manager.focused else {
            return String::new();
        };

        [&self.login, &self.login2, &self.copyright]
            .into_iter()
            .nth(target.panel)
            .and_then(|pane| {
                let target = self.manager.retained_target(target, &pane.ui.panel)?;

                ui::input::owner_element(&pane.ui.panel, target.owner)
            })
            .map(|(element, _)| element.name.clone())
            .unwrap_or_default()
    }

    pub fn submit(&mut self) -> Option<Event> {
        if self.launcher_handoff {
            return None;
        }

        if self.msgbox.placed.ui.panel.visible {
            return None;
        }

        let name = self.focused_name();

        self.submit_from(&name)
    }

    pub fn submit_from(&mut self, name: &str) -> Option<Event> {
        if name != FIELD_PASSWORD {
            return None;
        }

        let owner = self
            .login2
            .ui
            .panel
            .element_or_create(
                listener::BTN_LOGIN,
                formats::ui::pane::ElementType::Button,
                &self.login2.ui.resource,
            )
            .instance;

        let index = self
            .login2
            .ui
            .panel
            .elements
            .iter()
            .position(|e| e.instance == owner)?;

        Some(Event {
            panel: 1,
            id: EventId::Click,
            owner: Some(ui::draw::Owner::Element(index)),
            name: listener::BTN_LOGIN.to_string(),
            extra: -1,
        })
    }

    fn apply_form_visibility(&mut self) {
        if self.splash_visible() {
            return;
        }

        match self.form {
            Form::ServerSelect => self.login.ui.panel.visible = true,
            Form::Login => self.login2.ui.panel.visible = !self.launcher_handoff,
        }
    }

    fn splash_visible(&self) -> bool {
        self.s_controls
            .draws
            .iter()
            .any(|draw| draw.control == login_static::Control::Splash)
    }

    pub fn raise_box(&mut self, request: &msgbox::Request) -> Option<usize> {
        debug_assert!(
            self.msgbox.covers(request),
            "a login raiser named another .pane"
        );

        let slot = self.boxes.raise(request, &self.strings)?;

        self.msgbox.raised(&self.boxes, slot);

        self.box_manager = Manager {
            screen: self.layout.virtual_screen(),
            ..Manager::default()
        };

        Some(slot)
    }

    fn box_key(&mut self, ch: char) {
        let yes = match ch {
            '\r' | '\n' | 'y' | 'Y' => true,
            'n' | 'N' => false,
            _ => return,
        };

        let name = if yes {
            msgbox::ELEMENT_OK
        } else {
            msgbox::ELEMENT_CANCEL
        };

        let live = self
            .msgbox
            .placed
            .ui
            .panel
            .elements
            .iter()
            .any(|e| e.name == name && e.visible && e.enabled);

        if live && let Some(slot) = self.msgbox.slot {
            self.answer_box(slot, yes);
        }
    }

    pub fn answer_box(&mut self, slot: usize, yes: bool) -> Option<(i32, u32, i32)> {
        let answered = self.boxes.answer(slot, yes)?;

        self.msgbox.follow(&self.boxes);

        self.box_manager = Manager {
            screen: self.layout.virtual_screen(),
            ..Manager::default()
        };

        if answered.0 == msgbox::KIND_SERVER_SELECT && msgbox::answered(answered.2) {
            match answered.1 {
                0x0c | 0x0d if self.form == Form::Login && !self.launcher_login => {
                    self.form = Form::ServerSelect;

                    self.login.ui.panel.visible = true;

                    self.login2.ui.panel.visible = false;

                    self.manager = Manager {
                        screen: self.layout.virtual_screen(),
                        ..Manager::default()
                    };
                }
                0x0c | 0x0d => self.quit_requested = true,
                0x0e => self
                    .registration_requests
                    .push(self.strings.translate("ss_263")),
                _ => {}
            }
        }

        Some(answered)
    }
}

pub fn server_tables(game: &GameDir) -> listener::Tables {
    let names = game
        .read(b"sn.bin")
        .map(|b| sn::parse(&b, Codepage::Windows1252))
        .unwrap_or_else(|_| sn::parse(&[], Codepage::Windows1252));

    let table = game
        .read(b"serverlist.bin")
        .ok()
        .and_then(|b| serverlist::parse(&b).ok())
        .unwrap_or_default();

    listener::Tables {
        bounds: listener::bounds(&names, &table),
        names,
        table,
    }
}

pub fn server_labels(game: &GameDir) -> Vec<String> {
    let names = game
        .read(b"sn.bin")
        .map(|b| sn::parse(&b, Codepage::Windows1252))
        .unwrap_or_else(|_| sn::parse(&[], Codepage::Windows1252));

    let table = game
        .read(b"serverlist.bin")
        .ok()
        .and_then(|b| serverlist::parse(&b).ok())
        .unwrap_or_default();

    names
        .labels
        .iter()
        .enumerate()
        .map(|(record, label)| {
            match table
                .servers
                .get(record)
                .is_some_and(|s| !s.status_url.is_empty())
            {
                true => label.clone(),
                false => String::new(),
            }
        })
        .collect()
}

pub(crate) fn strings(game: &GameDir) -> Strings {
    crate::ui::strings(game)
}

pub fn populate_server_list(
    panel: &mut ui::Panel,
    names: &sn::ServerNames,
    table: &serverlist::ServerList,
    strings: &Strings,
    language: Language,
) {
    let Some(list) = panel
        .elements
        .iter_mut()
        .find(|e| e.name == "ServerListView")
        .and_then(|e| e.widget.list_mut())
    else {
        return;
    };

    list.clear_items();

    let bound = listener::bounds(names, table).slot;

    for slot in (0..=bound).rev() {
        let Some(&value) = names.slots.get(slot as usize) else {
            break;
        };

        if value < 1 {
            continue;
        }

        let record = value as usize - 1;

        if table
            .servers
            .get(record)
            .is_none_or(|s| s.status_url.is_empty())
        {
            continue;
        }

        let label = &names.labels[record];

        let fallback = label.is_empty().then(|| {
            strings
                .translate("ss_66")
                .replacen("%d", &(record + 1).to_string(), 1)
        });

        let label = fallback.as_deref().unwrap_or(label);

        let index = list.add_item("");

        let row = &mut list.items[index];

        row.payload = slot as i64;

        for child in &mut row.children {
            if child.name == "Name" {
                child.set_display(label);
            }
        }
    }

    let label = match language {
        Language::Portuguese => "Configurações",
        _ => "Settings",
    };

    let index = list.add_item("");

    let row = &mut list.items[index];

    row.payload = listener::SETTINGS_ROW;

    for child in &mut row.children {
        if child.name == "Name" {
            child.set_display(label);
        }
    }
}

pub fn build(
    game: &GameDir,
    backbuffer: (u32, u32),
    presentation: &LoginPresentation,
    time: &SceneTime,
    captions: bool,
) -> Result<LoginPanels> {
    let layout = Layout::new(backbuffer.0, backbuffer.1);

    let s_controls = login_static::build(game, backbuffer, presentation, time)
        .context("building the S* half of the scene")?;

    let strings = strings(game);

    let chains = fonts::load(game, fonts::system_font_dir().as_deref());

    let zoom = layout.ui_scale();

    let screen = layout.virtual_screen();

    let place = |script: &[u8]| -> Result<PlacedPanel> {
        let ui = PanelUi::build(game, script, &strings, &chains, zoom)
            .with_context(|| String::from_utf8_lossy(script).into_owned())?;

        let origin = ui.origin(screen);

        Ok(PlacedPanel { ui, origin })
    };

    let mut login = place(PANEL_LOGIN)?;

    let mut login2 = place(PANEL_LOGIN2)?;

    let login_fields = [
        (FIELD_ACCOUNT, formats::ui::pane::ElementType::ImeEditBox),
        (FIELD_PASSWORD, formats::ui::pane::ElementType::EditBox),
    ]
    .map(|(name, kind)| {
        login2
            .ui
            .panel
            .element_or_create(name, kind, &login2.ui.resource)
            .instance
    });

    initialize_credentials(&mut login2.ui.panel, login_fields);

    let login_button = login2
        .ui
        .panel
        .element_or_create(
            listener::BTN_LOGIN,
            formats::ui::pane::ElementType::Button,
            &login2.ui.resource,
        )
        .instance;

    let mut copyright = place(PANEL_COPYRIGHT)?;

    login.ui.panel.visible = false;

    login2.ui.panel.visible = false;

    copyright.ui.panel.visible = false;

    let names = game
        .read(b"sn.bin")
        .map(|b| sn::parse(&b, Codepage::Windows1252))
        .unwrap_or_else(|_| sn::parse(&[], Codepage::Windows1252));

    let table = game
        .read(b"serverlist.bin")
        .ok()
        .and_then(|b| serverlist::parse(&b).ok())
        .unwrap_or_default();

    populate_server_list(
        &mut login.ui.panel,
        &names,
        &table,
        &strings,
        game.language(),
    );

    for &(tag, visible) in OFFLINE_TAG_PASS {
        login.ui.panel.set_visible_by_tagname(tag, visible);
    }

    let msgbox = msgbox::BoxPanel::build(game, &strings, &chains, zoom, layout.virtual_extent())?;

    let mut scene = LoginPanels {
        notice: super::notice::Overlay::build(game, backbuffer, &strings)?,
        legacy_boxes: super::legacy_message_box::Dialogs::build(game, backbuffer.0)?,
        message_panel: super::message_panel::Overlay::build(game, backbuffer.0)?,
        layout,
        time: *time,
        s_controls,
        login,
        login2,
        login_fields,
        login_button,
        login_attempt_ms: 0,
        copyright,
        form: Form::ServerSelect,
        captions,
        guimat_textures: BTreeMap::new(),
        missing: Vec::new(),
        notes: Vec::new(),
        manager: Manager {
            screen,
            ..Manager::default()
        },
        bounds: listener::bounds(&names, &table),
        aggregate: aggregate::State::new(&table, time.calendar.server_day_of_month as i32),
        names,
        servers: table,
        strings,
        status: status::Status::default(),
        channels: Vec::new(),
        session: listener::Session::default(),
        last_login_ms: 0,
        clipboard: ui::text_entry::Local::default(),
        status_timeout: None,
        status_rx: None,
        status_tx: None,
        showing: None,
        status_for: None,
        status_requested: Vec::new(),
        boxes: msgbox::Boxes::new(),
        msgbox,
        box_manager: Manager {
            screen,
            ..Manager::default()
        },
        quit_requested: false,
        settings_requested: false,
        registration_requests: Vec::new(),
        launcher_login: false,
        launcher_handoff: false,
    };

    scene.apply_form_visibility();

    let interaction = Interaction::default();

    let form = (scene.login.ui.panel.visible, scene.login2.ui.panel.visible);

    scene.login.ui.panel.visible = true;

    scene.login2.ui.panel.visible = true;

    let referenced: Vec<(usize, i32)> = [&scene.login, &scene.login2, &scene.copyright]
        .into_iter()
        .enumerate()
        .flat_map(|(i, p)| {
            p.ui.referenced_textures(&interaction)
                .into_iter()
                .map(move |slot| (i, slot))
        })
        .collect();

    scene.login.ui.panel.visible = form.0;

    scene.login2.ui.panel.visible = form.1;

    for (panel, slot) in referenced {
        if scene.guimat_textures.contains_key(&slot) || scene.missing.contains(&slot) {
            continue;
        }

        let ui = &[&scene.login, &scene.login2, &scene.copyright][panel].ui;

        match load_guimat_texture(game, ui, slot)? {
            Some(loaded) => {
                scene.guimat_textures.insert(slot, loaded);
            }
            None => scene.missing.push(slot),
        }
    }

    for loaded in scene.msgbox.textures(game, &mut scene.missing)? {
        scene.guimat_textures.entry(loaded.slot).or_insert(loaded);
    }

    scene.notes = [&scene.login, &scene.login2, &scene.copyright]
        .into_iter()
        .flat_map(|p| p.ui.notes.iter().cloned())
        .collect();

    Ok(scene)
}

fn initialize_credentials(panel: &mut ui::Panel, fields: [ui::element::Instance; 2]) {
    for (index, owner) in fields.into_iter().enumerate() {
        let Some(edit) = panel
            .elements
            .iter_mut()
            .find(|e| e.instance == owner)
            .and_then(|e| e.widget.edit_mut())
        else {
            continue;
        };

        if edit.range <= 0 {
            edit.range = if index == 1 { 11 } else { 14 };
        }

        edit.text.clear();

        edit.preedit = None;

        edit.caret = 0;

        edit.anchor = 0;

        edit.scroll = 0;

        if index == 1 {
            edit.password = true;
        }
    }
}

pub fn request_status(scene: &mut LoginPanels, record: usize) {
    if record as i32 == scene.bounds.last_record && record < scene.servers.servers.len() {
        let requests = scene
            .aggregate
            .start(&mut scene.servers, record, scene.status_timeout);

        scene.status_requested.extend(requests);

        scene.status = scene.aggregate.result();

        scene.status_for = Some(record);

        return;
    }

    scene.aggregate.cancel();

    let Some(url) = scene
        .servers
        .servers
        .get(record)
        .map(|s| s.status_url.clone())
        .filter(|u| !u.is_empty())
    else {
        return;
    };

    scene.status_requested.push((record, url.clone()));

    let Some(timeout) = scene.status_timeout else {
        return;
    };

    let tx = match scene.status_tx.as_ref() {
        Some(tx) => tx.clone(),
        None => {
            let (tx, rx) = std::sync::mpsc::channel();

            scene.status_rx = Some(rx);

            scene.status_tx = Some(tx.clone());

            tx
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(
        move || match status::fetch_fields(&url, timeout).map(|p| p.status) {
            Ok(s) => {
                let _ = tx.send((record, s));
            }
            Err(e) => tracing::info!(% url, error = % e, "server status unavailable"),
        },
    );

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        match status::fetch_fields_async(&url, timeout)
            .await
            .map(|p| p.status)
        {
            Ok(status) => {
                let _ = tx.send((record, status));
            }
            Err(error) => tracing::info!(%url, %error, "server status unavailable"),
        }
    });
}

pub fn poll_status(scene: &mut LoginPanels) -> bool {
    let aggregate = scene.aggregate.poll();

    let landed: Vec<(usize, status::Status)> = scene
        .status_rx
        .as_ref()
        .map(|rx| rx.try_iter().collect())
        .unwrap_or_default();

    let mut apply = None;

    for (record, s) in landed {
        if scene.showing == Some(record) && !scene.aggregate.active(record) {
            apply = Some((record, s));
        }
    }

    if aggregate && let Some(record) = scene.showing.filter(|r| scene.aggregate.active(*r)) {
        apply = Some((record, scene.aggregate.result()));
    }

    let Some((record, s)) = apply else {
        return false;
    };

    scene.status = s;

    scene.status_for = Some(record);

    let selected = scene
        .login
        .ui
        .panel
        .elements
        .iter()
        .find(|e| e.name == listener::CHANNEL_VIEW)
        .and_then(|e| e.widget.list())
        .map(|l| l.selected)
        .unwrap_or(-1);

    listener::populate_channels(scene, record);

    if selected >= 0
        && let Some(list) = scene
            .login
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.name == listener::CHANNEL_VIEW)
            .and_then(|e| e.widget.list_mut())
        && (selected as usize) < list.items.len()
    {
        list.selected = selected;
    }

    true
}

pub(crate) fn load_guimat_texture(
    game: &GameDir,
    ui: &PanelUi,
    slot: i32,
) -> Result<Option<GuiTexture>> {
    let Some(def) = ui.resource.texture(slot) else {
        return Ok(None);
    };

    let Ok(bytes) = game.read(def.path.as_bytes()) else {
        return Ok(None);
    };

    let mut decoded = texture::decode_image(&bytes)
        .with_context(|| format!("decoding guimat slot {slot}: {}", def.path))?;

    let keyed_texels = if def.color_key != 0 {
        render::texture::apply_guimat_color_key(&mut decoded, def.color_key as u32)
    } else {
        0
    };

    Ok(Some(GuiTexture {
        slot,
        path: def.path.clone(),
        color_key: def.color_key,
        keyed_texels,
        texture: decoded,
    }))
}

pub fn upload(scene: &LoginPanels, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.upload(gpu, renderer);

    scene.notice.upload(gpu, renderer);

    scene.message_panel.upload(gpu, renderer);

    login_static::upload(&scene.s_controls, gpu, renderer);

    for (slot, loaded) in &scene.guimat_textures {
        renderer.upload_once(
            gpu,
            TextureKey::GuiMat(*slot),
            &loaded.texture,
            AlphaPolicy::Stored,
            &format!("{} (guimat slot {slot})", loaded.path),
        );
    }
}

pub fn sync(scene: &mut LoginPanels, gpu: &Gpu, renderer: &mut SceneRenderer) {
    scene.legacy_boxes.sync(gpu, renderer);

    let captions = scene.captions;

    scene.notice.sync(gpu, renderer);

    scene.message_panel.sync(gpu, renderer);

    for (index, panel) in scene.draw_order().into_iter().enumerate() {
        if panel.ui.panel.visible {
            renderer.sync_named_icons(gpu, index as PanelId, &panel.ui);

            if captions {
                renderer.sync_captions(gpu, index as PanelId, &panel.ui);
            }
        }
    }

    for (slot, placed) in scene.msgbox.visible() {
        let id = msgbox::caption_id(BOX_CAPTION_ID, slot);

        renderer.sync_named_icons(gpu, id, &placed.ui);

        if scene.captions {
            renderer.sync_captions(gpu, id, &placed.ui);
        }
    }
}

pub fn frame_draws(scene: &mut LoginPanels) -> Vec<PlannedQuad> {
    frame_draws_with_cursor(scene, true)
}

pub(crate) fn frame_draws_with_cursor(
    scene: &mut LoginPanels,
    include_cursor: bool,
) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    append_frame_draws(scene, include_cursor, &mut out);

    out
}

pub(crate) fn append_frame_draws(
    scene: &mut LoginPanels,
    include_cursor: bool,
    out: &mut Vec<PlannedQuad>,
) {
    scene.apply_form_visibility();

    login_static::append_frame_draws(&scene.s_controls, include_cursor, out);

    out.extend(
        scene
            .message_panel
            .frame_draws(&scene.layout, &scene.strings, scene.captions),
    );

    let captions = scene.captions;

    let interactions: Vec<Interaction> = [&scene.login, &scene.login2, &scene.copyright]
        .into_iter()
        .enumerate()
        .map(|(i, p)| scene.manager.interaction_for(i, &p.ui.panel, p.origin))
        .collect();

    let screen = scene.layout.virtual_screen();

    for (index, panel) in scene.draw_order().into_iter().enumerate() {
        panel.ui.panel.set_combo_screen(screen, panel.origin);

        if !panel.ui.panel.visible {
            continue;
        }

        let interaction = interactions[index];

        for draw in panel.ui.frame(&interaction, panel.origin) {
            if !captions && matches!(draw.source, Source::Caption(..)) {
                continue;
            }

            let texture = match draw.source {
                Source::Texture(slot) => TextureKey::GuiMat(slot),
                Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                Source::Amul(sheet) => TextureKey::Amul(sheet),
                Source::Caption(owner, run) => TextureKey::Caption(index as PanelId, owner, run),
                Source::NamedIcon(owner) => TextureKey::NamedIcon(index as PanelId, owner),
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

    out.extend(scene.msgbox.frame_draws(
        BOX_CAPTION_ID,
        captions,
        &scene.box_manager.interaction(0),
    ));

    out.extend(scene.notice.frame_draws(captions));

    out.extend(
        scene
            .legacy_boxes
            .frame_draws(&scene.layout, &scene.strings, captions),
    );
}
