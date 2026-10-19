use crate::frame::PanelId;
use crate::scene::create::Reserved;
use crate::scene::notice;
use crate::scene::panels::{Action, CONTENT_STORE, Panels, Screen};
use formats::text::Codepage;
use formats::ui::pane::ElementType;
use std::collections::VecDeque;
use ui::Strings;
use ui::input::Manager;

pub const CONTENT_CHAT: PanelId = 0x0E;

pub const CONTENT_CHAT_MENU: PanelId = 0x0F;

pub const CHAT_INPUT: &[u8] = b"ui/Panel/game/chat.pane";

pub const CHAT_LOG: &[u8] = b"ui/Panel/game/chat2.pane";

pub const CHAT_MENU: &[u8] = b"ui/Panel/game/ChatMenu.pane";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    pub button: &'static str,
    pub prefix: &'static str,
    pub label: &'static str,
    pub color: u32,
}

pub const MODES: [Mode; 8] = [
    Mode {
        button: "normal",
        prefix: "",
        label: "uu_245",
        color: 0xFFFF_AAAA,
    },
    Mode {
        button: "whisper",
        prefix: "/",
        label: "uu_246",
        color: 0xFFFF_FF00,
    },
    Mode {
        button: "party",
        prefix: "=",
        label: "uu_247",
        color: 0xFFFF_99FF,
    },
    Mode {
        button: "guild",
        prefix: "-",
        label: "uu_248",
        color: 0xFFAA_FFFF,
    },
    Mode {
        button: "ally",
        prefix: "--",
        label: "uu_249",
        color: 0xFF00_FFFF,
    },
    Mode {
        button: "ch",
        prefix: "@@",
        label: "uu_250",
        color: 0xF0F6_0AFF,
    },
    Mode {
        button: "king",
        prefix: "@",
        label: "uu_251",
        color: 0xFF00_AAFF,
    },
    Mode {
        button: "shout",
        prefix: "",
        label: "uu_252",
        color: 0xFF00_CD00,
    },
];

pub const NORMAL: usize = 0;

pub const WHISPER: usize = 1;

pub const PARTY: usize = 2;

pub const GUILD: usize = 3;

pub const ALLY: usize = 4;

pub const CITIZEN: usize = 5;

pub const KINGDOM: usize = 6;

pub const SHOUT: usize = 7;

pub const BRIGHTNESS: [u32; 6] = [100, 0x8C, 0xAA, 0xD2, 0xF0, 0];

pub const SIZE_STEPS: i32 = 5;

pub const TABS: [(&str, Option<&str>); 5] = [
    ("TabBtn0", Some("kingdomchat")),
    ("TabBtn1", None),
    ("TabBtn2", Some("partychat")),
    ("TabBtn3", Some("whisper")),
    ("TabBtn4", Some("guildchat")),
];

pub const MAX_LINE: usize = 0x6B;

pub const COMMAND_RECORDS: [usize; 17] = [
    61, 234, 386, 387, 389, 390, 518, 519, 550, 1089, 1099, 1148, 1184, 1185, 1213, 1216, 1285,
];

pub const COMMAND_LITERALS: [&str; 5] = ["relo", "srv", "tab", "Relocate", "relocate"];

pub fn is_command(strings: &Strings, word: &str) -> bool {
    if COMMAND_LITERALS.contains(&word) || is_reply_command(strings, word) {
        return true;
    }

    COMMAND_RECORDS.iter().any(|record| {
        let text = strings.translate(&format!("ss_{record}"));

        !text.is_empty() && text == word
    })
}

fn is_reply_command(strings: &Strings, word: &str) -> bool {
    matches!(word, "r" | "re" | "\u{00b4}\u{00e4}" | "\u{00a4}\u{00a7}")
        || (!word.is_empty() && strings.translate("ss_61") == word)
}

pub fn is_relocate_command(strings: &Strings, word: &str) -> bool {
    matches!(word, "relo" | "Relocate" | "relocate") || strings.translate("ss_234") == word
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCommand {
    Kingdom,
    King,
}

impl PendingCommand {
    pub const fn wire(self) -> &'static str {
        match self {
            PendingCommand::Kingdom => "kingdom",
            PendingCommand::King => "king",
        }
    }
}

pub fn pending_command(strings: &Strings, word: &str) -> Option<PendingCommand> {
    let kingdom = strings.translate("ss_1184");

    let king = strings.translate("ss_1185");

    if !kingdom.is_empty() && kingdom == word {
        Some(PendingCommand::Kingdom)
    } else if !king.is_empty() && king == word {
        Some(PendingCommand::King)
    } else {
        None
    }
}

pub fn pending_command_allowed(move_alpha: f32) -> bool {
    f64::from(move_alpha) >= 0.9_f64 || move_alpha <= 0.0
}

pub const SHOUT_WORD_RECORD: usize = 389;

pub const FLOOD_WINDOW_MS: u32 = 4000;

pub const KINGDOM_COOLDOWN_MS: u32 = 6000;

pub const KINGDOM_LEVEL: i16 = 0x28;

pub const RECORD_CAP: usize = 0x65;

pub const ROW_CAP: usize = 100;

pub const ROW_TRIM_AT: usize = 0x6E;

pub const FOLLOW_MARGIN: i32 = 5;

pub const RECALL: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub color: u32,
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enter {
    Nothing,
    Sent(Sent),
    Command(String),
    Relocate(Option<String>),
    PendingCommand([u8; CHANNEL_LEN]),
    Local(&'static Local),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub mode: usize,
    pub text: String,
    pub packet: Vec<u8>,
    pub balloon: bool,
}

pub const OPCODE_COMMAND: u16 = 0x333;

pub const COMMAND_LEN: usize = 0x8C;

pub const OPCODE_CHANNEL: u16 = 0x334;

pub const CHANNEL_LEN: usize = 0xA0;

pub const CHANNEL_TARGET: usize = 0x0C;

pub const CHANNEL_TEXT: usize = 0x1C;

pub const CHANNEL_KIND: usize = 0x9C;

pub fn channel_kind(mode: usize) -> u16 {
    match mode {
        WHISPER => 0,
        PARTY => 1,
        _ => 3,
    }
}

fn packet_head(opcode: u16, len: usize, client_id: u16) -> Vec<u8> {
    let mut out = vec![0u8; len];

    out[0x04..0x06].copy_from_slice(&opcode.to_le_bytes());

    out[0x06..0x08].copy_from_slice(&client_id.to_le_bytes());

    out
}

fn put(out: &mut [u8], at: usize, limit: usize, text: &str) {
    let bytes = formats::text::codepage::encode(text, Codepage::Windows1252);

    let n = bytes.len().min(limit.saturating_sub(1));

    out[at..at + n].copy_from_slice(&bytes[..n]);
}

pub fn costume_command_packet(client_id: u16, mount: bool) -> Vec<u8> {
    command_packet(
        client_id,
        if mount {
            "mountcostumehide"
        } else {
            "costumehide"
        },
    )
}

pub fn command_packet(client_id: u16, text: &str) -> Vec<u8> {
    let mut out = packet_head(OPCODE_COMMAND, COMMAND_LEN, client_id);

    put(&mut out, 0x0C, COMMAND_LEN - 0x0C, text);

    out
}

pub const GUILD_CHAT_ON_WORD: &str = "guildon";

pub const GUILD_CHAT_OFF_WORD: &str = "guildoff";

pub fn guild_chat_frame(client_id: u16, on: bool) -> Vec<u8> {
    let word = match on {
        true => GUILD_CHAT_ON_WORD,
        false => GUILD_CHAT_OFF_WORD,
    };

    command_packet(client_id, word)
}

pub fn channel_packet(client_id: u16, mode: usize, target: &str, text: &str) -> Vec<u8> {
    let mut out = packet_head(OPCODE_CHANNEL, CHANNEL_LEN, client_id);

    put(
        &mut out,
        CHANNEL_TARGET,
        CHANNEL_TEXT - CHANNEL_TARGET,
        target,
    );

    put(&mut out, CHANNEL_TEXT, CHANNEL_KIND - CHANNEL_TEXT, text);

    out[CHANNEL_KIND..CHANNEL_KIND + 2].copy_from_slice(&channel_kind(mode).to_le_bytes());

    out
}

#[derive(Debug, Clone, Default)]
pub struct SavedTab {
    request: Option<(String, String)>,
}

impl SavedTab {
    pub fn remember(&mut self, chat: &Chat) {
        if let Some(request) = &chat.tab_request {
            self.request = Some(request.clone());
        }
    }

    pub fn entry_packet(&self, client_id: u16, name: &str) -> Option<Vec<u8>> {
        let (owner, body) = self.request.as_ref()?;

        (owner == name && !body.is_empty()).then(|| channel_packet(client_id, WHISPER, "tab", body))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Chat {
    input_bottom_offsets: Vec<(&'static str, ElementType, i32)>,
    pub mode: usize,
    pub whisper: String,
    pub shout_word: String,
    command_aliases: Vec<(usize, String)>,
    pub records: VecDeque<Line>,
    pub history: Vec<String>,
    pub history_at: usize,
    pub shouts: Vec<String>,
    pub shouts_at: usize,
    pub sent_at: [u32; 4],
    pub kingdom_at: u32,
    pub focused: bool,
    pub ramp: f32,
    pub base_alpha: f32,
    pub size_step: i32,
    pub bright_step: usize,
    pub tab1: bool,
    pub outbox: Vec<Vec<u8>>,
    tab_request: Option<(String, String)>,
    pub balloon: Option<(u16, String, u32)>,
    pub notes: Vec<String>,
}

impl Chat {
    pub fn new() -> Chat {
        Chat {
            mode: NORMAL,
            history: vec![String::new(); RECALL],
            shouts: vec![String::new(); RECALL],
            base_alpha: 1.0,
            tab1: true,
            ..Chat::default()
        }
    }

    fn remember_command(&mut self, record: usize, word: &str) {
        if !word.is_empty()
            && !Strings::is_key(word)
            && !self
                .command_aliases
                .iter()
                .any(|(id, alias)| *id == record && alias == word)
        {
            self.command_aliases.push((record, word.to_owned()));
        }
    }

    fn remember_commands(&mut self, strings: &Strings) {
        for record in COMMAND_RECORDS
            .into_iter()
            .chain([1951, GUILD_BOARD_RECORD])
        {
            self.remember_command(record, &command_translation(strings, record));
        }
    }

    pub fn prefix(&self, mode: usize) -> String {
        match mode {
            WHISPER => format!("/{} ", self.whisper),
            SHOUT => format!("/{} ", self.shout_word),
            other => MODES
                .get(other)
                .map_or(String::new(), |m| m.prefix.to_string()),
        }
    }

    pub fn flood(&mut self, now_ms: u32) -> bool {
        let oldest = self.sent_at[3];

        self.sent_at[3] = self.sent_at[2];

        self.sent_at[2] = self.sent_at[1];

        self.sent_at[1] = self.sent_at[0];

        self.sent_at[0] = now_ms;

        oldest != 0 && now_ms.wrapping_sub(oldest) < FLOOD_WINDOW_MS
    }

    pub fn kingdom_ok(&self, level: i16, now_ms: u32) -> bool {
        level >= KINGDOM_LEVEL && now_ms >= self.kingdom_at.wrapping_add(KINGDOM_COOLDOWN_MS)
    }

    fn remember(&mut self, text: &str) {
        if self.history.first().is_none_or(String::is_empty) {
            self.history = vec![text.to_string(); RECALL];
        } else if self.history.first().is_some_and(|newest| newest != text) {
            self.history.pop();

            self.history.insert(0, text.to_string());
        }

        self.history_at = 0;
    }

    fn remember_target(&mut self, word: &str) {
        if self.shouts.first().is_none_or(String::is_empty) {
            self.shouts = vec![word.to_string(); RECALL];
        } else if self.shouts.first().is_some_and(|newest| newest != word) {
            self.shouts.pop();

            self.shouts.insert(0, word.to_string());
        }

        self.shouts_at = 0;
    }

    pub fn recall(&mut self, up: bool) -> String {
        let (ring, index) = if up {
            (&self.history, &mut self.history_at)
        } else {
            (&self.shouts, &mut self.shouts_at)
        };

        let text = ring.get(*index).cloned().unwrap_or_default();

        *index = (*index + 1) % RECALL;

        if up { text } else { format!("/{text} ") }
    }

    pub fn frame_alpha(&mut self) -> Option<f32> {
        if !(self.ramp > 0.0 && self.ramp < 1.0) {
            return None;
        }

        self.ramp = match self.focused {
            true => (self.ramp + 0.01).min(1.0),
            false => (self.ramp - 0.01).max(0.0),
        };

        Some(self.base_alpha * self.ramp)
    }

    pub fn fade(&mut self) {
        self.ramp = match self.focused {
            true => (self.ramp + 0.001).min(0.999),
            false => (self.ramp - 0.001).max(0.001),
        };
    }
}

pub fn index_of(panels: &Panels, script: &[u8]) -> Option<usize> {
    panels.screens.iter().position(|s| s.script == script)
}

fn screen_mut<'a>(panels: &'a mut Panels, script: &[u8]) -> Option<&'a mut Screen> {
    let index = index_of(panels, script)?;

    panels.screens.get_mut(index)
}

pub fn bind(panels: &mut Panels, strings: &Strings) {
    panels.chat.remember_commands(strings);

    panels.chat.ramp = 0.0;

    panels.chat.focused = false;

    panels.chat.base_alpha = index_of(panels, CHAT_INPUT)
        .map(|i| (panels.screens[i].placed.ui.panel.background.color >> 24) as f32 / 255.0)
        .unwrap_or(1.0);

    let shout = strings.translate(&format!("ss_{SHOUT_WORD_RECORD}"));

    if !shout.is_empty() && !shout.starts_with("ss_") {
        panels.chat.shout_word = shout;
    }

    panels.chat.input_bottom_offsets = index_of(panels, CHAT_INPUT)
        .map(|index| {
            let panel = &panels.screens[index].placed.ui.panel;

            [
                ("BtnMenu", ElementType::Button),
                ("ChatEditBox", ElementType::ImeEditBox),
                ("backimg2", ElementType::Image),
            ]
            .into_iter()
            .filter_map(|(name, kind)| {
                panel
                    .element(name, kind)
                    .map(|element| (name, kind, panel.height() - element.y))
            })
            .collect()
        })
        .unwrap_or_default();

    if let Some(screen) = screen_mut(panels, CHAT_INPUT) {
        screen.placed.ui.panel.resize_tip =
            formats::ui::pane::resize_tip::TOP | formats::ui::pane::resize_tip::BOTTOM;

        if let Some(element) = screen.element_mut("ChatEditBox", ElementType::ImeEditBox)
            && let Some(edit) = element.widget.edit_mut()
        {
            edit.range = 200;

            edit.indicator = true;
        }

        for (name, _) in TABS {
            let Some(element) = screen.element_mut(name, ElementType::Button) else {
                continue;
            };

            let mut text = element.display.clone();

            let mut length = text.encode_utf16().count();

            if length > 3 {
                let mut removed = 0;

                while text.starts_with(' ') && removed < length {
                    text.remove(0);

                    removed += 1;

                    length -= 1;
                }
            }

            element.set_display(&text);

            if let Some(state) = element.widget.button_mut() {
                state.toggle = true;
            }
        }
    }

    if let Some(screen) = screen_mut(panels, CHAT_LOG)
        && let Some(element) = screen.element_mut("backimg", ElementType::Image)
    {
        set_layer_alpha(element, 0x3C);
    }

    set_mode(panels, NORMAL, false, strings);

    fade(panels, false);

    panels.chat.sent_at = [0; 4];

    panels.chat.history = vec![String::new(); RECALL];

    panels.chat.shouts = vec![String::new(); RECALL];

    panels.chat.history_at = 0;

    panels.chat.shouts_at = 0;

    panels.chat.whisper.clear();
}

pub fn relocalize(panels: &mut Panels, strings: &Strings) {
    let old_shout = panels.chat.shout_word.clone();

    panels.chat.remember_command(SHOUT_WORD_RECORD, &old_shout);

    panels.chat.remember_commands(strings);

    let shout = strings.translate(&format!("ss_{SHOUT_WORD_RECORD}"));

    if !shout.is_empty() && !shout.starts_with("ss_") {
        panels.chat.shout_word = shout;
    }

    let draft = edit_text(panels);

    if !draft.is_empty() {
        panels.chat.mode = mode_of(&panels.chat, strings, &draft);
    }

    let mode = panels.chat.mode;

    let Some(screen) = screen_mut(panels, CHAT_INPUT) else {
        return;
    };

    if let Some(mode) = MODES.get(mode)
        && let Some(element) = screen.element_mut("BtnMenu", ElementType::Button)
    {
        let label = strings.translate(mode.label);

        element.caption_key = mode.label.to_string();

        element.caption = label.clone();

        element.set_display(&label);
    }

    for (name, _) in TABS {
        if let Some(element) = screen.element_mut(name, ElementType::Button) {
            let text = if Strings::is_key(&element.caption_key) {
                strings.translate(&element.caption_key)
            } else {
                element.display.clone()
            };

            element.set_display(text.trim_start_matches(' '));
        }
    }
}

pub fn resized(panels: &mut Panels, index: usize) {
    if index_of(panels, CHAT_INPUT) != Some(index) {
        return;
    }

    let offsets = panels.chat.input_bottom_offsets.clone();

    let panel = &mut panels.screens[index].placed.ui.panel;

    let height = panel.height();

    for (name, kind, bottom) in offsets {
        if let Some(element) = panel.element_mut(name, kind) {
            element.y = height - bottom;
        }
    }
}

fn set_layer_alpha(element: &mut ui::Element, alpha: u32) {
    for layer in &mut element.layers {
        layer.texture_color.set_all(alpha << 24 | 0x00FF_FFFF);
    }
}

pub fn set_mode(panels: &mut Panels, mode: usize, insert: bool, strings: &Strings) {
    let mode = match mode < MODES.len() {
        true => mode,
        false => NORMAL,
    };

    let prefix = panels.chat.prefix(mode);

    panels.chat.mode = mode;

    let label = strings.translate(MODES[mode].label);

    let Some(screen) = screen_mut(panels, CHAT_INPUT) else {
        return;
    };

    if let Some(element) = screen.element_mut("BtnMenu", ElementType::Button) {
        element.caption_key = MODES[mode].label.to_string();

        element.caption = label.clone();

        element.set_display(&label);
    }

    if insert
        && let Some(element) = screen.element_mut("ChatEditBox", ElementType::ImeEditBox)
        && let Some(edit) = element.widget.edit_mut()
    {
        edit.text = prefix;

        edit.caret = edit.text.chars().count() as i32;

        edit.anchor = edit.caret;
    }
}

pub fn mode_of(chat: &Chat, strings: &Strings, text: &str) -> usize {
    if let Some(rest) = text.strip_prefix('/')
        && let Some((word, _)) = rest.split_once(' ')
        && command_record(chat, strings, word) == Some(SHOUT_WORD_RECORD)
    {
        return SHOUT;
    }

    let two: String = text.chars().take(2).collect();

    if two.chars().count() == 2 {
        match two.as_str() {
            "--" => return ALLY,
            "@@" => return CITIZEN,
            _ => {}
        }
    }

    match text.chars().next() {
        Some('-') => GUILD,
        Some('@') => KINGDOM,
        Some('=') => PARTY,
        Some('/') => WHISPER,
        _ => NORMAL,
    }
}

pub fn text_changed(panels: &mut Panels, strings: &Strings) {
    let text = edit_text(panels);

    let mode = mode_of(&panels.chat, strings, &text);

    set_mode(panels, mode, false, strings);
}

pub fn edit_text(panels: &Panels) -> String {
    let Some(index) = index_of(panels, CHAT_INPUT) else {
        return String::new();
    };

    panels.screens[index]
        .element("ChatEditBox", ElementType::ImeEditBox)
        .and_then(|e| e.widget.edit())
        .map(|edit| edit.text.clone())
        .unwrap_or_default()
}

pub fn edit_mut(panels: &mut Panels) -> Option<&mut ui::widget::EditState> {
    let index = index_of(panels, CHAT_INPUT)?;

    panels.screens[index]
        .element_mut("ChatEditBox", ElementType::ImeEditBox)?
        .widget
        .edit_mut()
}

pub fn set_text(panels: &mut Panels, text: &str) {
    if let Some(edit) = edit_mut(panels) {
        edit.text = text.to_string();

        edit.caret = edit.text.chars().count() as i32;

        edit.anchor = edit.caret;

        edit.scroll = 0;
    }
}

pub fn clear(panels: &mut Panels) {
    if let Some(screen) = screen_mut(panels, CHAT_INPUT)
        && let Some(element) = screen.element_mut("ChatEditBox", ElementType::ImeEditBox)
        && let Some(edit) = element.widget.edit_mut()
    {
        edit.text.clear();

        edit.caret = 0;

        edit.anchor = 0;

        edit.scroll = 0;
    }
}

pub(crate) fn set_hud_visible(panels: &mut Panels, hud: &mut super::hud::Hud, visible: bool) {
    if !visible {
        if panels.chat.focused {
            focus(panels, hud, false, false);
        } else {
            set_menu(panels, false);
        }
    }

    for script in [CHAT_INPUT, CHAT_LOG] {
        if let Some(screen) = screen_mut(panels, script) {
            screen.set_open(visible);
        }
    }
}

pub fn focus(panels: &mut Panels, hud: &mut super::hud::Hud, on: bool, clear_box: bool) {
    if on && index_of(panels, CHAT_INPUT).is_some_and(|index| !panels.screens[index].open) {
        return;
    }

    panels.chat.focused = on;

    fade(panels, on);

    if clear_box {
        clear(panels);
    }

    let index = index_of(panels, CHAT_INPUT);

    let target = match (on, index) {
        (true, Some(index)) => edit_target(panels, index),
        _ => None,
    };

    super::panels::set_focus(hud, panels, target);
}

fn fade(panels: &mut Panels, on: bool) {
    panels.chat.fade();

    let strings = panels.strings.clone();

    if let Some(screen) = screen_mut(panels, CHAT_INPUT) {
        screen.placed.ui.panel.movable = on;

        for (name, _) in TABS {
            if let Some(element) = screen.element_mut(name, ElementType::Button) {
                element.visible = !on;
            }
        }

        for (name, kind) in [
            ("backimg2", ElementType::Image),
            ("BtnMenu", ElementType::Button),
            ("ChatEditBox", ElementType::ImeEditBox),
        ] {
            if let Some(element) = screen.element_mut(name, kind) {
                element.visible = on;
            }
        }
    }

    if let Some(screen) = screen_mut(panels, CHAT_LOG) {
        screen.placed.ui.panel.visible = true;

        screen.open = true;

        if let Some(element) = screen.element_mut("backimg", ElementType::Image) {
            element.visible = true;
        }

        if let Some(list) = element_list_mut(screen) {
            list.draw_highlight = on;
        }
    }

    match on {
        true => set_mode(panels, panels.chat.mode, true, &strings),
        false => set_menu(panels, false),
    }
}

pub fn holds_caret(panels: &Panels, manager: &Manager) -> bool {
    let Some(index) = index_of(panels, CHAT_INPUT) else {
        return false;
    };

    manager.focused.is_some() && manager.focused == edit_target(panels, index)
}

fn edit_target(panels: &Panels, index: usize) -> Option<ui::input::Target> {
    let element = panels.screens[index]
        .placed
        .ui
        .panel
        .elements
        .iter()
        .position(|e| e.name.eq_ignore_ascii_case("ChatEditBox"))?;

    Some(ui::input::Target {
        panel: crate::scene::panels::input_slot(index),
        owner: ui::draw::Owner::Element(element),
    })
}

pub(crate) fn request_edit_focus(panels: &mut Panels, hud: &mut super::hud::Hud) {
    let Some(index) = index_of(panels, CHAT_INPUT) else {
        return;
    };

    let screen = &panels.screens[index];

    if !screen.placed.ui.panel.allow_focus
        || !screen
            .element("ChatEditBox", ElementType::ImeEditBox)
            .is_some_and(ui::input::tab_stop)
    {
        return;
    }

    if let Some(target) = edit_target(panels, index) {
        super::panels::set_focus(hud, panels, Some(target));
    }
}

pub fn set_menu(panels: &mut Panels, open: bool) {
    let anchor = index_of(panels, CHAT_INPUT).and_then(|index| {
        let screen = &panels.screens[index];

        let button = screen.element("BtnMenu", ElementType::Button)?;

        let rect = button.rect();

        Some((
            screen.placed.origin.0 + rect[0],
            screen.placed.origin.1 + rect[1],
        ))
    });

    let Some(screen) = screen_mut(panels, CHAT_MENU) else {
        return;
    };

    screen.set_open(open);

    if open {
        screen.placed.ui.panel.popup = true;

        screen.placed.ui.panel.keep_popup_on_click = false;
    }

    if let Some((x, y)) = anchor.filter(|_| open) {
        let height = screen.placed.ui.panel.height();

        screen.placed.origin = (x, y - height);
    }

    if open {
        panels.release_popup_focus();
    }
}

pub fn append(panels: &mut Panels, text: &str, color: u32, speaker: Option<&str>) {
    let color = match color {
        0 => 0xFF00_0000,
        other => other,
    };

    panels.chat.records.push_back(Line {
        text: text.to_string(),
        color,
        speaker: speaker.map(str::to_string),
    });

    while panels.chat.records.len() > RECORD_CAP {
        panels.chat.records.pop_front();
    }

    let Some(index) = index_of(panels, CHAT_LOG) else {
        return;
    };

    let width = panels.client_config.integer("chatlinepx");

    let lines = styled_lines_width(
        &mut panels.screens[index],
        text,
        i32::try_from(width).ok().filter(|&n| n > 0),
    );

    let Some(element) = panels.screens[index].element_mut("ChatList", ElementType::ListView) else {
        return;
    };

    let rect = element.rect();

    let Some(list) = element.widget.list_mut() else {
        return;
    };

    let page = list.rows_per_page(list.content_rect(rect));

    let follow = list.items.len() as i32 - FOLLOW_MARGIN <= list.scroll + page;

    if let Some(lines) = lines {
        for line in lines {
            list.add_rich_line(line, color);
        }
    } else {
        list.add_line(text, color);
    }

    if list.items.len() > ROW_TRIM_AT {
        let excess = list.items.len() - ROW_CAP;

        list.remove_front(excess);
    }

    if follow {
        scroll_to_end(&mut panels.screens[index]);
    }
}

pub fn wrap_line(screen: &mut Screen, text: &str) -> Vec<String> {
    styled_lines(screen, text)
        .map(|lines| lines.iter().map(ui::rich::Line::text).collect())
        .unwrap_or_else(|| vec![text.to_string()])
}

fn styled_lines(screen: &mut Screen, text: &str) -> Option<Vec<ui::rich::Line>> {
    styled_lines_width(screen, text, None)
}

fn styled_lines_width(
    screen: &mut Screen,
    text: &str,
    width: Option<i32>,
) -> Option<Vec<ui::rich::Line>> {
    let element = screen.element("ChatList", ElementType::ListView)?;

    let (Some(list), Some(material)) = (element.widget.list(), element.layers.first()) else {
        return None;
    };

    let content = list.content_rect(element.rect());

    let width = width.unwrap_or(content[2] - content[0]);

    let font = material.font_index;

    let gap = 0;

    let layout = ui::rich::wrap(text, font, width, gap, &mut screen.placed.ui.measure())?;

    (!layout.lines.is_empty()).then_some(layout.lines)
}

fn scroll_to_end(screen: &mut Screen) {
    let Some(element) = screen.element("ChatList", ElementType::ListView) else {
        return;
    };

    let rect = element.rect();

    let Some(list) = element.widget.list() else {
        return;
    };

    let content = list.content_rect(rect);

    let rows = list.items.len() as i32;

    let page = list.rows_per_page(content);

    let bottom = (rows - page).max(0);

    if let Some(list) = element_list_mut(screen) {
        list.scroll = bottom;
    }
}

fn element_list_mut(screen: &mut Screen) -> Option<&mut ui::widget::ListState> {
    screen
        .element_mut("ChatList", ElementType::ListView)
        .and_then(|e| e.widget.list_mut())
}

pub fn page_log(panels: &mut Panels, down: bool) {
    let Some(screen) = screen_mut(panels, CHAT_LOG) else {
        return;
    };

    let Some(element) = screen.element_mut("ChatList", ElementType::ListView) else {
        return;
    };

    let rect = element.rect();

    let Some(list) = element.widget.list_mut() else {
        return;
    };

    let content = list.content_rect(rect);

    list.scroll = list
        .clamped_scroll(content)
        .saturating_add(if down { 5 } else { -5 });

    list.scroll = list.clamped_scroll(content);
}

pub const SAY_COLOR: u32 = 0xFFFF_FFFF;

pub const SAY_COLOR_OFFSCREEN: u32 = 0xFFAA_FFAA;

pub const KIND_QUIET: u16 = 7;

pub const QUIET_COLOR: u32 = 0xFFBB_BBBB;

pub const BALLOON_MS: u32 = 3000;

pub const BALLOON_LONG_MS: u32 = 10_000;

pub const BALLOON_LONG_PREFIX: char = '*';

pub const SILENT_PREFIXES: [char; 3] = ['=', '-', '@'];

#[derive(Debug, Clone, Default)]
pub struct Speakers {
    pub mobs: Vec<(u16, String)>,
    pub party: Vec<(u16, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Mob,
    Party,
}

impl Speakers {
    pub fn just_me(client_id: u16, name: &str) -> Speakers {
        Speakers {
            mobs: vec![(client_id, name.to_string())],
            party: Vec::new(),
        }
    }

    pub fn name_of(&self, id: u16) -> Option<(Voice, &str)> {
        if let Some((_, name)) = self.mobs.iter().find(|(mob, _)| *mob == id) {
            return Some((Voice::Mob, name.as_str()));
        }

        let (_, name) = self.party.iter().find(|(member, _)| *member == id)?;

        Some((Voice::Party, name.as_str()))
    }

    pub fn party_size(&self) -> usize {
        self.party.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Modal,
    Memo,
    Posted { text: String, color: u32 },
    Dropped(&'static str),
}

pub fn channel_style(text: &str, kind: u16) -> Option<(u32, usize)> {
    let mut chars = text.chars();

    let (first, second) = (chars.next(), chars.next());

    if first == Some(NOTICE_PREFIX) {
        return None;
    }

    if first == Some('-') && kind == 3 {
        return Some(match second == Some('-') {
            true => (MODES[ALLY].color, 2),
            false => (MODES[GUILD].color, 1),
        });
    }

    if kind == KIND_QUIET {
        return Some((QUIET_COLOR, 0));
    }

    match first {
        Some('=') => Some((MODES[PARTY].color, 1)),
        Some('@') => Some(match second == Some('@') {
            true => (MODES[CITIZEN].color, 2),
            false => (MODES[KINGDOM].color, 1),
        }),
        _ => Some((MODES[WHISPER].color, 0)),
    }
}

pub const NOTICE_PREFIX: char = '!';

pub fn channel_line(
    text: &str,
    kind: u16,
    party_size: usize,
    guild_muted: bool,
    kingdom_muted: bool,
) -> Result<(u32, usize), &'static str> {
    let Some((color, skip)) = channel_style(text, kind) else {
        return Err("a ! notice, which goes to the notice board and not the log");
    };

    let first = text.chars().next();

    if first == Some('-') && kind == 3 && guild_muted {
        return Err("the guild channel is muted — TabBtn4 is checked");
    }

    if first == Some('@') && kind != KIND_QUIET && kingdom_muted {
        return Err("the kingdom channel is muted — TabBtn2 is checked");
    }

    if first == Some('=') && kind != KIND_QUIET && party_size <= 1 {
        return Ok((MODES[WHISPER].color, 0));
    }

    Ok((color, skip))
}

pub fn receive(
    panels: &mut Panels,
    line: &protocol::server::ChatLine,
    speakers: &Speakers,
    calendar: &crate::clock::Calendar,
) -> Heard {
    use protocol::server;

    match line.opcode {
        server::CHAT_SHOUT => shout(panels, &line.text),
        server::CHAT_CHANNEL => channel(panels, line, speakers, calendar),
        server::CHAT_SAY => say(panels, &line.text, line.speaker, speakers),
        server::CHAT_CONSOLE => console(panels, &line.text),
        server::SYSTEM_MESSAGE | server::SYSTEM_MESSAGE_ARG => {
            let text = canned(panels, line);

            say(panels, &text, line.speaker, speakers)
        }
        _ => Heard::Dropped("not one of the five chat opcodes"),
    }
}

pub fn say(panels: &mut Panels, text: &str, speaker: u16, speakers: &Speakers) -> Heard {
    let Some((voice, name)) = speakers.name_of(speaker) else {
        return Heard::Dropped("nobody with that client id is visible or in the party");
    };

    let name = name.to_string();

    if voice == Voice::Mob && !panels.chat.tab1 {
        return Heard::Dropped("spoken lines are hidden — TabBtn1 is unchecked");
    }

    let color = match voice {
        Voice::Mob => SAY_COLOR,
        Voice::Party => SAY_COLOR_OFFSCREEN,
    };

    let echo = format!("[{name}]> {text}");

    let named = (voice == Voice::Mob).then_some(name.as_str());

    append(panels, &echo, color, named);

    if voice == Voice::Mob {
        panels.chat.balloon = balloon_of(text).map(|(body, life)| (speaker, body, life));
    }

    Heard::Posted { text: echo, color }
}

pub fn take_balloon(panels: &mut Panels) -> Option<(u16, String, u32)> {
    panels.chat.balloon.take()
}

pub fn balloon_of(text: &str) -> Option<(String, u32)> {
    let long = text.starts_with(BALLOON_LONG_PREFIX);

    let first = text.chars().next();

    if first.is_some_and(|c| SILENT_PREFIXES.contains(&c)) {
        return None;
    }

    let body = match long {
        true => text[BALLOON_LONG_PREFIX.len_utf8()..].to_string(),
        false => text.to_string(),
    };

    Some((body, if long { BALLOON_LONG_MS } else { BALLOON_MS }))
}

fn channel(
    panels: &mut Panels,
    line: &protocol::server::ChatLine,
    speakers: &Speakers,
    calendar: &crate::clock::Calendar,
) -> Heard {
    let known = speakers.mobs.iter().any(|(id, _)| *id == line.speaker);

    if line.text.starts_with(NOTICE_PREFIX) {
        if known && line.kind == KIND_QUIET {
            let text = format!("[{}]> {}", line.name, line.text);

            append(panels, &text, QUIET_COLOR, None);

            return Heard::Posted {
                text,
                color: QUIET_COLOR,
            };
        }

        super::memo::channel(panels, &line.name, &line.text[1..], calendar);

        return Heard::Memo;
    }

    let (guild_muted, kingdom_muted) = mutes(panels);

    let party = speakers.party_size();

    match channel_line(&line.text, line.kind, party, guild_muted, kingdom_muted) {
        Err(refusal) => Heard::Dropped(refusal),
        Ok((color, skip)) => {
            let body: String = line.text.chars().skip(skip).collect();

            let echo = format!("[{}]> {body}", line.name);

            append(panels, &echo, color, None);

            Heard::Posted { text: echo, color }
        }
    }
}

fn shout(panels: &mut Panels, text: &str) -> Heard {
    if !panels.chat.tab1 {
        return Heard::Dropped("shouts are hidden — TabBtn1 is unchecked");
    }

    let color = MODES[SHOUT].color;

    append(panels, text, color, None);

    Heard::Posted {
        text: text.to_string(),
        color,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Echo {
    pub sentence: &'static str,
    pub tab: Option<usize>,
    pub checked: bool,
    pub name: usize,
    pub state: usize,
    pub format: &'static str,
}

pub const ECHO_FORMAT: &str = "%s%s";

pub const ECHO_FORMAT_COSTUME: &str = "%s :%s";

pub const STATE_ON: usize = 446;

pub const STATE_OFF: usize = 447;

pub const CONSOLE_ECHOES: [Echo; 12] = [
    Echo {
        sentence: "Whisper : Off",
        tab: Some(3),
        checked: true,
        name: 448,
        state: STATE_OFF,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Whisper : On",
        tab: Some(3),
        checked: false,
        name: 448,
        state: STATE_ON,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Citizen Chatting : Off",
        tab: Some(2),
        checked: true,
        name: 449,
        state: STATE_OFF,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Citizen Chatting : On",
        tab: Some(2),
        checked: false,
        name: 449,
        state: STATE_ON,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Guild Chatting : Off",
        tab: Some(4),
        checked: true,
        name: 450,
        state: STATE_OFF,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Guild Chatting : On",
        tab: Some(4),
        checked: false,
        name: 450,
        state: STATE_ON,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Kingdom Chatting : On",
        tab: Some(0),
        checked: false,
        name: 451,
        state: STATE_ON,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Kingdom Chatting : Off",
        tab: Some(0),
        checked: true,
        name: 451,
        state: STATE_OFF,
        format: ECHO_FORMAT,
    },
    Echo {
        sentence: "Costume Hide : On",
        tab: None,
        checked: false,
        name: 518,
        state: STATE_OFF,
        format: ECHO_FORMAT_COSTUME,
    },
    Echo {
        sentence: "Costume Hide : Off",
        tab: None,
        checked: false,
        name: 518,
        state: STATE_ON,
        format: ECHO_FORMAT_COSTUME,
    },
    Echo {
        sentence: "Mount Costume Hide : On",
        tab: None,
        checked: false,
        name: 519,
        state: STATE_OFF,
        format: ECHO_FORMAT_COSTUME,
    },
    Echo {
        sentence: "Mount Costume Hide : Off",
        tab: None,
        checked: false,
        name: 519,
        state: STATE_ON,
        format: ECHO_FORMAT_COSTUME,
    },
];

pub fn console_echo(text: &str) -> Option<&'static Echo> {
    CONSOLE_ECHOES.iter().find(|echo| echo.sentence == text)
}

pub fn console(panels: &mut Panels, text: &str) -> Heard {
    let echo = console_echo(text).copied();

    let line = match &echo {
        Some(echo) => {
            if let Some(tab) = echo.tab {
                set_tab_checked(panels, TABS[tab].0, echo.checked);
            }

            let name = panels.strings.translate(&format!("ss_{}", echo.name));

            let state = panels.strings.translate(&format!("ss_{}", echo.state));

            format_tokens(echo.format, &[&name, &state])
        }
        None => text.to_string(),
    };

    append(panels, &line, NOTICE_COLOR, None);

    Heard::Posted {
        text: line,
        color: NOTICE_COLOR,
    }
}

fn set_tab_checked(panels: &mut Panels, name: &str, checked: bool) {
    let Some(index) = index_of(panels, CHAT_INPUT) else {
        return;
    };

    let Some(element) = panels.screens[index].element_mut(name, ElementType::Button) else {
        return;
    };

    if let Some(state) = element.widget.button_mut() {
        state.checked = checked;
    }
}

fn canned(panels: &Panels, line: &protocol::server::ChatLine) -> String {
    let Some(record) = line.strdef() else {
        return String::new();
    };

    resolve(
        panels,
        record,
        line.strdef_code.unwrap_or_default(),
        line.opcode == protocol::server::SYSTEM_MESSAGE_ARG,
        &line.argument,
    )
}

pub fn resolve(
    panels: &Panels,
    record: usize,
    code: i16,
    has_argument: bool,
    argument: &str,
) -> String {
    resolve_strings(&panels.strings, record, code, has_argument, argument)
}

pub fn resolve_strings(
    strings: &ui::Strings,
    record: usize,
    code: i16,
    has_argument: bool,
    argument: &str,
) -> String {
    let text = strings.translate(&format!("ss_{record}"));

    let text = match text.is_empty() || text == format!("ss_{record}") {
        true => code.to_string(),
        false => text,
    };

    if !has_argument {
        return text;
    }

    let spaced = argument.replace(',', " ");

    let tokens: Vec<&str> = spaced.split_whitespace().take(6).collect();

    if tokens.first().is_some_and(|token| token.starts_with('@')) {
        let index = tokens
            .get(1)
            .and_then(|token| token.parse::<i64>().ok())
            .unwrap_or(0);

        let named = strings.translate(&format!("ss_{}", (index + STRDEF_BASE as i64).max(0)));

        return format_tokens(&text, &[named.as_str()]);
    }

    format_tokens(&text, &tokens)
}

use protocol::server::STRDEF_BASE;

pub fn refine_broadcast(panels: &Panels, record: usize, argument: &str) -> String {
    let fields = refine_fields(argument);

    let name = panels
        .items
        .name(fields.item_id.max(0) as usize)
        .unwrap_or_default();

    fields.format(&panels.strings.translate(&format!("ss_{record}")), &name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineFields {
    pub item_id: i32,
    pub player: String,
    pub level: String,
}

impl RefineFields {
    pub fn format(&self, record_text: &str, item_name: &str) -> String {
        format_tokens(record_text, &[&self.player, item_name, &self.level])
    }
}

pub fn refine_fields(argument: &str) -> RefineFields {
    let spaced = argument.replace(',', " ");

    let tokens: Vec<&str> = spaced.split_whitespace().take(6).collect();

    let token = |n: usize| tokens.get(n).copied().unwrap_or("").to_string();

    RefineFields {
        item_id: parse_decimal_prefix(&token(0)),
        player: token(2),
        level: token(1),
    }
}

fn parse_decimal_prefix(text: &str) -> i32 {
    let bytes = text.as_bytes();

    let mut i = 0;

    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }

    let negative = bytes.get(i) == Some(&b'-');

    if matches!(bytes.get(i), Some(b'+' | b'-')) {
        i += 1;
    }

    let mut value: i32 = 0;

    while let Some(digit) = bytes.get(i).and_then(|b| char::from(*b).to_digit(10)) {
        value = value.wrapping_mul(10).wrapping_add(digit as i32);

        i += 1;
    }

    match negative {
        true => value.wrapping_neg(),
        false => value,
    }
}

pub fn format_tokens(format: &str, tokens: &[&str]) -> String {
    let mut out = String::with_capacity(format.len());

    let mut rest = format;

    let mut next = tokens.iter();

    while let Some(at) = rest.find("%s") {
        out.push_str(&rest[..at]);

        out.push_str(next.next().copied().unwrap_or(""));

        rest = &rest[at + 2..];
    }

    out.push_str(rest);

    out
}

fn mutes(panels: &Panels) -> (bool, bool) {
    let checked = |name: &str| {
        index_of(panels, CHAT_INPUT)
            .and_then(|index| panels.screens[index].element(name, ElementType::Button))
            .and_then(|element| element.widget.button())
            .is_some_and(|button| button.checked)
    };

    (checked("TabBtn4"), checked("TabBtn2"))
}

pub fn system(panels: &mut Panels, text: &str, color: u32) {
    append(panels, text, color, None);
}

pub const SYSTEM_COLOR: u32 = 0xFFFF_FFAA;

pub const NOTICE_COLOR: u32 = 0xFFCC_AAFF;

pub const NOTICE_RECORD: usize = 0x64E;

pub const MODAL_ONLY: [usize; 4] = [0x1D1, 0x1D2, 0x1E4, 0x1E5];

pub const HANDLED_ELSEWHERE: [usize; 2] = [0x61A, 0x61B];

pub const PLAIN: u8 = 0;

pub const MODAL: u8 = 4;

pub fn announce(
    panels: &mut Panels,
    message: &protocol::server::SystemMessage,
    now_ms: u32,
    calendar: &crate::clock::Calendar,
    hide_item_announcements: bool,
) -> Announced {
    let announced = prepare_announcement(
        &panels.strings,
        &panels.items,
        message,
        true,
        hide_item_announcements,
    );

    for (text, life_ms) in &announced.banners {
        notice::post(panels, text, *life_ms, NOTICE_SOUND, now_ms);
    }

    for (text, color) in &announced.posted {
        system(panels, text, *color);
    }

    if let Some(body) = &announced.memo {
        super::memo::numbered(panels, body, calendar);
    }

    announced
}

pub fn prepare_announcement(
    strings: &ui::Strings,
    items: &formats::data::item_list::ItemList,
    message: &protocol::server::SystemMessage,
    field: bool,
    hide_item_announcements: bool,
) -> Announced {
    let record = message.strdef();

    let mut announced = Announced {
        record,
        text: String::new(),
        posted: Vec::new(),
        banners: Vec::new(),
        memo: None,
        would_have_exited: false,
        world_changed: false,
    };

    if (!message.has_argument && !matches!(message.kind, PLAIN | MODAL))
        || (message.has_argument && !matches!(message.kind, PLAIN | FORMATTED | RELAYED | MODAL))
    {
        return announced;
    }

    if hide_item_announcements && matches!(record, 1462 | 1562 | 1584) {
        return announced;
    }

    let special =
        message.has_argument && message.kind != FORMATTED && HANDLED_ELSEWHERE.contains(&record);

    announced.text = if message.has_argument && message.kind == FORMATTED {
        format_tokens(
            &strings.translate(&format!("ss_{record}")),
            &[&message.argument],
        )
    } else {
        match special {
            true => {
                let fields = refine_fields(&message.argument);

                fields.format(
                    &strings.translate(&format!("ss_{record}")),
                    &items
                        .name(fields.item_id.max(0) as usize)
                        .unwrap_or_default(),
                )
            }
            false => resolve_strings(
                strings,
                record,
                message.code,
                message.has_argument,
                &message.argument,
            ),
        }
    };

    let text = announced.text.clone();

    announced.would_have_exited =
        message.has_argument && message.kind != FORMATTED && record == EXIT_RECORD;

    let memo_prefix = message.has_argument
        && message.kind != FORMATTED
        && !special
        && record != EXIT_RECORD
        && text.starts_with(NOTICE_PREFIX);

    if memo_prefix {
        announced.memo = Some(text[1..].to_string());
    }

    let nonfield_prefix = !field && memo_prefix;

    if nonfield_prefix {
        announced
            .banners
            .push((text[1..].to_string(), notice::LIFE_MS));
    }

    let routing_text = if nonfield_prefix { &text[1..] } else { &text };

    for life_ms in notice_lives(record, message.kind, message.has_argument, routing_text) {
        announced.banners.push((text.clone(), life_ms));
    }

    if field {
        for color in log_colors(record, message.kind, message.has_argument, &text) {
            announced.posted.push((text.clone(), color));
        }
    }

    announced
}

pub const NOTICE_SOUND: bool = true;

pub fn notice_lives(record: usize, kind: u8, has_argument: bool, text: &str) -> Vec<u32> {
    let mut out = Vec::new();

    if (!has_argument && !matches!(kind, PLAIN | MODAL))
        || (has_argument && !matches!(kind, PLAIN | FORMATTED | RELAYED | MODAL))
    {
        return out;
    }

    if !has_argument {
        out.push(match MODAL_ONLY.contains(&record) {
            true => notice::LIFE_LONG_MS,
            false => notice::LIFE_MS,
        });

        return out;
    }

    if kind == FORMATTED {
        out.push(notice::LIFE_MS);

        return out;
    }

    if record == EXIT_RECORD {
        return out;
    }

    if HANDLED_ELSEWHERE.contains(&record) {
        out.push(notice::LIFE_MS);

        return out;
    }

    if text.starts_with(NOTICE_PREFIX) {
        return out;
    }

    if MODAL_ONLY.contains(&record) {
        out.push(notice::LIFE_LONG_MS);
    } else if record == KHEPRA_END_RECORD || record == NOTICE_RECORD {
        out.push(notice::LIFE_MS);
    }

    out.push(notice::LIFE_MS);

    if kind == RELAYED {
        out.push(notice::LIFE_MS);
    }

    out
}

pub const KHEPRA_END_RECORD: usize = 0x562;

pub fn ends_khepra(message: &protocol::server::SystemMessage) -> bool {
    message.has_argument
        && matches!(message.kind, PLAIN | RELAYED | MODAL)
        && message.strdef() == KHEPRA_END_RECORD
}

pub const RELAYED: u8 = 3;

pub const FORMATTED: u8 = 2;

pub fn log_colors(record: usize, kind: u8, has_argument: bool, text: &str) -> Vec<u32> {
    let mut out = Vec::new();

    if !has_argument && kind != PLAIN && kind != MODAL {
        return out;
    }

    if has_argument {
        if HANDLED_ELSEWHERE.contains(&record) || record == EXIT_RECORD {
            return out;
        }

        if text.starts_with(NOTICE_PREFIX) {
            return out;
        }

        if record == NOTICE_RECORD {
            out.push(NOTICE_COLOR);
        }
    } else if MODAL_ONLY.contains(&record) {
        return out;
    }

    if kind == MODAL {
        out.push(SYSTEM_COLOR);
    }

    out
}

pub const EXIT_RECORD: usize = 0x40C;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Announced {
    pub world_changed: bool,
    pub record: usize,
    pub text: String,
    pub posted: Vec<(String, u32)>,
    pub banners: Vec<(String, u32)>,
    pub memo: Option<String>,
    pub would_have_exited: bool,
}

pub fn drain_toasts(panels: &mut Panels, toasts: &mut Vec<String>, color: u32) {
    for line in std::mem::take(toasts) {
        system(panels, &line, color);
    }
}

pub fn frame(panels: &mut Panels) {
    let Some(index) = index_of(panels, CHAT_INPUT) else {
        return;
    };

    if !panels.screens[index].placed.ui.panel.visible {
        return;
    }

    let Some(alpha) = panels.chat.frame_alpha() else {
        return;
    };

    let background = &mut panels.screens[index].placed.ui.panel.background;

    let byte = (alpha.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;

    background.color = byte << 24 | background.color & 0x00FF_FFFF;
}

pub fn enter(
    panels: &mut Panels,
    client_id: u16,
    speaker: &str,
    level: i16,
    reserved: &Reserved,
    now_ms: u32,
) -> Enter {
    enter_with_move_alpha(
        panels,
        client_id,
        speaker,
        level,
        f32::NAN,
        reserved,
        now_ms,
    )
}

pub fn enter_with_move_alpha(
    panels: &mut Panels,
    client_id: u16,
    speaker: &str,
    level: i16,
    move_alpha: f32,
    reserved: &Reserved,
    now_ms: u32,
) -> Enter {
    let raw = edit_text(panels);

    let text = raw.trim_matches([' ', '\t', '\r', '\n']).to_string();

    if text.is_empty() {
        clear(panels);

        return Enter::Nothing;
    }

    let outcome = route_with_move_alpha(
        panels, client_id, speaker, level, move_alpha, reserved, now_ms, &text,
    );

    clear(panels);

    outcome
}

pub fn route(
    panels: &mut Panels,
    client_id: u16,
    speaker: &str,
    level: i16,
    reserved: &Reserved,
    now_ms: u32,
    text: &str,
) -> Enter {
    route_with_move_alpha(
        panels,
        client_id,
        speaker,
        level,
        f32::NAN,
        reserved,
        now_ms,
        text,
    )
}

fn queue_packet(panels: &mut Panels, packet: Vec<u8>) -> Vec<u8> {
    let mut preview = packet.clone();

    if let Some(limit) =
        formats::net::client_config::normalize_chat(&mut preview, &panels.client_config)
    {
        let text = panels
            .strings
            .translate("ss_1664")
            .replacen("%s", &limit.to_string(), 1);

        system(panels, &text, SYSTEM_COLOR);
    }

    panels.chat.outbox.push(packet.clone());

    packet
}

fn command_translation(strings: &Strings, record: usize) -> String {
    let word = strings.translate(&format!("ss_{record}"));

    if record == 1951 && (word.is_empty() || word == "ss_1951") {
        "helmohide".into()
    } else {
        word
    }
}

fn command_word_matches(record: usize, translated: &str, word: &str) -> bool {
    !translated.is_empty()
        && if record == 1951 {
            translated.eq_ignore_ascii_case(word)
        } else {
            translated == word
        }
}

fn command_record(chat: &Chat, strings: &Strings, word: &str) -> Option<usize> {
    [234, 1951, 518, 519, 1099, 1184, 1185]
        .into_iter()
        .chain(COMMAND_RECORDS)
        .chain([GUILD_BOARD_RECORD])
        .find(|record| command_word_matches(*record, &command_translation(strings, *record), word))
        .or_else(|| {
            if is_command(strings, word) {
                return None;
            }

            chat.command_aliases
                .iter()
                .rev()
                .find(|(record, translated)| command_word_matches(*record, translated, word))
                .map(|(record, _)| *record)
        })
}

fn current_command_word(strings: &Strings, record: Option<usize>, original: &str) -> String {
    record
        .map(|record| command_translation(strings, record))
        .filter(|word| !word.is_empty() && !Strings::is_key(word))
        .unwrap_or_else(|| original.to_owned())
}

pub fn route_with_move_alpha(
    panels: &mut Panels,
    client_id: u16,
    speaker: &str,
    level: i16,
    move_alpha: f32,
    reserved: &Reserved,
    now_ms: u32,
    text: &str,
) -> Enter {
    let ascii = formats::text::codepage::encode(text, Codepage::Windows1252);

    if ascii.len() >= MAX_LINE {
        return Enter::Nothing;
    }

    if panels.chat.flood(now_ms) {
        return Enter::Nothing;
    }

    panels.chat.remember(text);

    if let Some(row) = local_command(&panels.strings, text) {
        return Enter::Local(row);
    }

    if command_record(&panels.chat, &panels.strings, text) == Some(GUILD_BOARD_RECORD) {
        return Enter::Local(&LOCAL_COMMANDS[0]);
    }

    if let Some(rest) = text.strip_prefix('/') {
        let word: String = rest.split(' ').next().unwrap_or_default().to_string();

        if word.chars().count() > 14 {
            return Enter::Nothing;
        }

        let record = command_record(&panels.chat, &panels.strings, &word);

        if record == Some(234) || is_relocate_command(&panels.strings, &word) {
            if !panels.is_open(CONTENT_STORE) {
                let target = rest.split_whitespace().nth(1).map(ToOwned::to_owned);

                return Enter::Relocate(target);
            }

            return Enter::Command(current_command_word(&panels.strings, record, &word));
        }

        panels.chat.whisper = word.clone();

        panels.chat.remember_target(&word);

        let alias = panels.strings.translate("ss_1951");

        if record == Some(1951)
            || word.eq_ignore_ascii_case(if alias.is_empty() || alias == "ss_1951" {
                "helmohide"
            } else {
                &alias
            })
        {
            let packet = queue_packet(panels, command_packet(client_id, "helmohidecmd"));

            return Enter::Sent(Sent {
                mode: NORMAL,
                text: "helmohidecmd".into(),
                packet,
                balloon: false,
            });
        }

        let costume_command = [(518, "costumehide"), (519, "mountcostumehide")]
            .into_iter()
            .find(|(id, _)| record == Some(*id));

        if let Some((_, command)) = costume_command {
            let packet = costume_command_packet(client_id, command == "mountcostumehide");

            let packet = queue_packet(panels, packet);

            return Enter::Sent(Sent {
                mode: NORMAL,
                text: command.into(),
                packet,
                balloon: false,
            });
        }

        if record == Some(1099) {
            let body = rest.get(word.len()..).unwrap_or_default();

            let filtered = censor(body.strip_prefix(' ').unwrap_or(body), reserved);

            let packet = channel_packet(client_id, WHISPER, "relo", &filtered);

            let packet = queue_packet(panels, packet);

            return Enter::Sent(Sent {
                mode: WHISPER,
                text: filtered,
                packet,
                balloon: false,
            });
        }

        let pending = match record {
            Some(1184) => Some(PendingCommand::Kingdom),
            Some(1185) => Some(PendingCommand::King),
            _ => None,
        };

        if let Some(command) = pending {
            if !pending_command_allowed(move_alpha) {
                return Enter::Nothing;
            }

            let body = rest.get(word.len()..).unwrap_or_default();

            let body = body.strip_prefix(' ').unwrap_or(body);

            let filtered = censor(body, reserved);

            let frame: [u8; CHANNEL_LEN] =
                channel_packet(client_id, WHISPER, command.wire(), &filtered)
                    .try_into()
                    .expect("channel_packet is exactly the arm-7 stage length");

            return Enter::PendingCommand(frame);
        }

        if word == "tab" {
            let body = rest.get(word.len()..).unwrap_or_default();

            let body = censor(body.strip_prefix(' ').unwrap_or(body), reserved);

            if !pending_command_allowed(move_alpha) || body == speaker {
                return Enter::Nothing;
            }

            let sent = whisper(panels, client_id, speaker, reserved, &word, rest);

            panels.chat.tab_request = Some((speaker.to_string(), body));

            return sent;
        }

        if (record.is_some_and(|record| COMMAND_RECORDS.contains(&record))
            || is_command(&panels.strings, &word))
            && record != Some(61)
            && !is_reply_command(&panels.strings, &word)
        {
            return Enter::Command(current_command_word(&panels.strings, record, &word));
        }

        return whisper(panels, client_id, speaker, reserved, &word, rest);
    }

    let mode = mode_of(&panels.chat, &panels.strings, text);

    let body = strip_prefix(&panels.chat, mode, text);

    let filtered = censor(&body, reserved);

    if mode == KINGDOM && !panels.chat.kingdom_ok(level, now_ms) {
        return Enter::Nothing;
    }

    if mode == KINGDOM {
        panels.chat.kingdom_at = now_ms;
    }

    let packet = match mode {
        NORMAL => command_packet(client_id, &filtered),
        _ => channel_packet(client_id, mode, &panels.chat.whisper.clone(), &filtered),
    };

    let packet = queue_packet(panels, packet);

    let echo = format!("[{speaker}]> {filtered}");

    let color = MODES[mode].color;

    let named = (mode == NORMAL).then_some(speaker);

    append(panels, &echo, color, named);

    let balloon = mode == NORMAL && !text.starts_with("+set");

    if balloon {
        panels.chat.balloon = balloon_of(&filtered).map(|(body, life)| (client_id, body, life));
    }

    Enter::Sent(Sent {
        mode,
        text: filtered,
        packet,
        balloon,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalWhat {
    Help,
    Effects { toggle_grass: bool },
    Fps,
    Effect,
    ShowExp,
    GuildMark,
    CherryBlossom,
    GuildBoard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub word: &'static str,
    pub what: LocalWhat,
}

pub const GUILD_BOARD_RECORD: usize = 191;

pub const LOCAL_COMMANDS: [Local; 9] = [
    Local {
        word: "",
        what: LocalWhat::GuildBoard,
    },
    Local {
        word: "/help",
        what: LocalWhat::Help,
    },
    Local {
        word: "effects2",
        what: LocalWhat::Effects { toggle_grass: true },
    },
    Local {
        word: "effects",
        what: LocalWhat::Effects {
            toggle_grass: false,
        },
    },
    Local {
        word: "fps",
        what: LocalWhat::Fps,
    },
    Local {
        word: "effect",
        what: LocalWhat::Effect,
    },
    Local {
        word: "exp",
        what: LocalWhat::ShowExp,
    },
    Local {
        word: "gmrk",
        what: LocalWhat::GuildMark,
    },
    Local {
        word: "/\u{ba}\u{a2}\u{b2}\u{c9}\u{b2}\u{f4}\u{b1}\u{e2}",
        what: LocalWhat::CherryBlossom,
    },
];

pub const GUILD_MARK_ON: &str = "Guild Mark : ON";

pub const GUILD_MARK_OFF: &str = "Guild Mark : OFF";

pub const OVERLAY_COLOR: u32 = 0xFFFF_AAAA;

pub fn local_command(strings: &Strings, text: &str) -> Option<&'static Local> {
    LOCAL_COMMANDS.iter().find(|row| match row.what {
        LocalWhat::GuildBoard => {
            let record = strings.translate(&format!("ss_{GUILD_BOARD_RECORD}"));

            !record.is_empty() && record == text
        }
        _ => row.word == text,
    })
}

fn whisper(
    panels: &mut Panels,
    client_id: u16,
    speaker: &str,
    reserved: &Reserved,
    word: &str,
    rest: &str,
) -> Enter {
    let body = rest.get(word.len()..).unwrap_or_default();

    let body = body.strip_prefix(' ').unwrap_or(body);

    let filtered = censor(body, reserved);

    let reply = command_record(&panels.chat, &panels.strings, word) == Some(61)
        || is_reply_command(&panels.strings, word);

    let target = if reply { "reply" } else { word };

    let packet = channel_packet(client_id, WHISPER, target, &filtered);

    let packet = queue_packet(panels, packet);

    let echo = if reply {
        format!("[{speaker}] [reply]> {filtered}")
    } else {
        format!("[{speaker}] : {word}> {filtered}")
    };

    append(panels, &echo, MODES[WHISPER].color, None);

    Enter::Sent(Sent {
        mode: WHISPER,
        text: filtered,
        packet,
        balloon: false,
    })
}

fn strip_prefix(chat: &Chat, mode: usize, text: &str) -> String {
    let _ = chat;

    let prefix = match mode {
        WHISPER | SHOUT => return text.to_string(),
        other => MODES[other].prefix,
    };

    text.strip_prefix(prefix).unwrap_or(text).to_string()
}

fn censor(text: &str, reserved: &Reserved) -> String {
    let mut bytes = formats::text::codepage::encode(text, Codepage::Windows1252);

    reserved.substitute(&mut bytes, Codepage::Windows1252);

    formats::text::codepage::decode(&bytes, Codepage::Windows1252)
}

pub fn button_down(panels: &mut Panels, content: PanelId, name: &str, strings: &Strings) {
    if content != CONTENT_CHAT_MENU {
        return;
    }

    if name == "whisper" {
        clear(panels);

        set_text(panels, "/w ");
    } else if let Some(mode) = MODES.iter().position(|m| m.button == name) {
        set_mode(panels, mode, false, strings);
    }
}

pub fn command(
    panels: &mut Panels,
    content: PanelId,
    name: &str,
    client_id: u16,
    strings: &Strings,
) -> Vec<Action> {
    if content == CONTENT_CHAT_MENU {
        if let Some(mode) = MODES.iter().position(|m| m.button == name) {
            set_mode(panels, mode, true, strings);
        }

        return Vec::new();
    }

    if content != CONTENT_CHAT {
        return Vec::new();
    }

    if let Some((_, command)) = TABS.iter().find(|(tab, _)| *tab == name) {
        match command {
            Some(text) => panels.chat.outbox.push(command_packet(client_id, text)),
            None => {
                panels.chat.tab1 = !panels.chat.tab1;

                set_tab_checked(panels, "TabBtn1", !panels.chat.tab1);

                let label = strings.translate("ss_452");

                let state = strings.translate(if panels.chat.tab1 { "ss_446" } else { "ss_447" });

                append(panels, &format!("{label}{state}"), TAB1_COLOR, None);
            }
        }

        return Vec::new();
    }

    match name {
        "BtnMenu" => {
            set_menu(panels, true);
        }
        "Br" => brightness(panels),
        "Size" => size(panels),
        _ => {}
    }

    Vec::new()
}

pub const TAB1_COLOR: u32 = 0xFFCC_AAFF;

fn brightness(panels: &mut Panels) {
    let next = match panels.chat.bright_step + 1 {
        n if n > BRIGHTNESS.len() - 1 => 0,
        n => n,
    };

    panels.chat.bright_step = next;

    let alpha = BRIGHTNESS[next];

    if let Some(screen) = screen_mut(panels, CHAT_LOG)
        && let Some(element) = screen.element_mut("backimg", ElementType::Image)
    {
        set_layer_alpha(element, alpha);
    }
}

fn size(panels: &mut Panels) {
    let next = match panels.chat.size_step + 1 {
        n if !(0..SIZE_STEPS).contains(&n) => 0,
        n => n,
    };

    panels.chat.size_step = next;

    let Some(screen) = screen_mut(panels, CHAT_LOG) else {
        return;
    };

    let Some(align) = screen
        .element("Align", ElementType::Image)
        .map(|e| e.rect())
    else {
        return;
    };

    let top = align[1] - (align[3] - align[1]) * next;

    for (name, kind) in [
        ("ChatList", ElementType::ListView),
        ("backimg", ElementType::Image),
    ] {
        if let Some(element) = screen.element_mut(name, kind) {
            let bottom = element.y + element.height;

            element.y = top;

            element.height = bottom - top;
        }
    }
}

pub fn size_hotkey(panels: &mut Panels) {
    size(panels);

    if panels.chat.size_step == 0
        && let Some(screen) = screen_mut(panels, CHAT_LOG)
    {
        scroll_to_end(screen);
    }
}

#[cfg(test)]
mod language_tests;

#[cfg(test)]
mod announcement_tests;
