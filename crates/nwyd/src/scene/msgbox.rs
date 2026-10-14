use crate::fonts::Chains;
use crate::scene::login_panels::PlacedPanel;
use crate::ui::PanelUi;
use anyhow::Result;
use assets::GameDir;
use formats::ui::pane::ElementType;
use ui::Strings;

pub const SLOTS: usize = 5;

pub const KIND_SELECT_CHAR: i32 = 8;

pub const KIND_SERVER_SELECT: i32 = 7;

pub const KIND: i32 = KIND_SELECT_CHAR;

pub const FLAGS: u32 = 0x2000_0001;

pub const CLOSE_ALL: u32 = 0x1000_0000;

pub const CLOSE_SAME_CONTEXT: u32 = 0x2000_0000;

pub const CLOSE_SAME_TEXT: u32 = 0x4000_0000;

pub const PANE_MASK: u32 = 0x0F00_0000;

pub const PANE_SYS_LOADING: u32 = 0x0100_0000;

pub const PANE_EDITBOX: u32 = 0x0200_0000;

pub const PANE_PROGRESS: u32 = 0x0300_0000;

pub const BUTTON_MASK: u32 = 0xF;

pub const BUTTONS_OK: u32 = 0;

pub const BUTTONS_YES_NO: u32 = 1;

pub const CLEAR_BUTTON_DEFAULTS: u32 = 0xF00;

pub const HIDE_BUTTONS: u32 = CLEAR_BUTTON_DEFAULTS;

pub const DEFAULT_CAPTION_STRDEF: usize = 237;

pub const NUI_ID: u32 = 1;

pub const Z_ORDER: i32 = 200;

pub const ELEMENT_MSG: &str = "msg";

pub const ELEMENT_TITLE: &str = "title";

pub const ELEMENT_OK: &str = "ok";

pub const ELEMENT_CANCEL: &str = "cancel";

pub const ELEMENT_EDIT: &str = "edit";

pub const ALTERNATE_PANES: [&[u8]; 3] = [
    br"ui/Panel/Msgbox/msgbox_editbox.pane",
    br"ui/Panel/Msgbox/msgbox_sysLoading.pane",
    br"ui/Panel/Msgbox/msgbox_progress.pane",
];

pub fn script(flags: u32) -> Option<&'static [u8]> {
    match flags & PANE_MASK {
        0x0000_0000 => Some(br"ui/Panel/Msgbox/msgbox.pane"),
        0x0100_0000 => Some(br"ui/Panel/Msgbox/msgbox_sysLoading.pane"),
        0x0200_0000 => Some(br"ui/Panel/Msgbox/msgbox_editbox.pane"),
        0x0300_0000 => Some(br"ui/Panel/Msgbox/msgbox_progress.pane"),
        _ => None,
    }
}

pub fn promote(flags: u32) -> u32 {
    if flags == PANE_SYS_LOADING {
        PANE_SYS_LOADING | CLOSE_ALL
    } else {
        flags
    }
}

pub fn centre(size: (i32, i32), rect: [i32; 4], screen: (i32, i32)) -> (i32, i32) {
    let (w, h) = size;

    let [left, top, right, bottom] = rect;

    let mut x = (right - left) / 2 - w / 2 + left;

    let mut y = (bottom - top) / 2 - h / 2 + top;

    if screen.0 < w + x {
        x = screen.0 - w;
    }

    if screen.1 < h + y {
        y = screen.1 - h;
    }

    (x.max(0), y.max(0))
}

pub fn centre_ok(panel: (i32, i32), ok_width: i32) -> i32 {
    let (left, right) = panel;

    ((right - ok_width) - left) / 2
}

pub const CONTEXT_EXIT: u32 = 0x0C;

pub const CONTEXT_OTHER_SERVER: u32 = 0x0D;

pub const CONTEXT_NEW_ACCOUNT: u32 = 0x0E;

pub const CONTEXT_BACK: u32 = 0x10;

pub const CONTEXT_DELETE: u32 = 0x11;

pub const CONTEXT_21F: u32 = 0x12;

pub const CONTEXT_FAA: u32 = 0x15;

pub const ANSWER_YES: i32 = 1;

pub const ANSWER_NO: i32 = 2;

pub fn answered(answer: i32) -> bool {
    answer == ANSWER_YES
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Body {
    StrDef(usize),
    TwoLines(usize, usize),
    Text(String),
}

impl Body {
    pub fn text(&self, strings: &Strings) -> String {
        let record = |n: usize| strings.translate(&format!("ss_{n}"));

        match self {
            Body::StrDef(n) => record(*n),
            Body::TwoLines(a, b) => {
                format!("{}{}{}", record(*a), ui::text::FF_BREAK, record(*b))
            }
            Body::Text(text) => text.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Request {
    pub caption: Option<usize>,
    pub kind: i32,
    pub context: u32,
    pub body: Body,
    pub flags: u32,
}

impl Request {
    pub fn restart_notice() -> Request {
        Request {
            caption: None,
            kind: 0,
            context: 0,
            body: Body::Text(" Please Re-run the game for autopatch.".into()),
            flags: 0,
        }
    }

    pub fn character_select(context: u32, text: usize) -> Request {
        Request {
            caption: None,
            kind: KIND_SELECT_CHAR,
            context,
            body: Body::StrDef(text),
            flags: FLAGS,
        }
    }

    pub fn server_select(context: u32, body: Body) -> Request {
        Request {
            caption: None,
            kind: KIND_SERVER_SELECT,
            context,
            body,
            flags: FLAGS,
        }
    }

    pub fn caption_strdef(&self) -> usize {
        self.caption.unwrap_or(DEFAULT_CAPTION_STRDEF)
    }

    pub fn yes_no(&self) -> bool {
        !matches!(self.flags & BUTTON_MASK, BUTTONS_OK | 0xC)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Open {
    pub visible: bool,
    pub kind: i32,
    pub context: u32,
    pub flags: u32,
    pub caption: String,
    pub text: String,
    pub script: &'static [u8],
    pub yes_no: bool,
    timeout: Option<(u32, u32)>,
    pub progress: i32,
    pub edit_limit: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Boxes {
    slots: [Option<Open>; SLOTS],
}

impl Boxes {
    pub fn new() -> Boxes {
        Boxes::default()
    }

    pub fn raise(&mut self, request: &Request, strings: &Strings) -> Option<usize> {
        let flags = promote(request.flags);

        let context = request.context;

        let text = request.body.text(strings);

        for slot in self.slots.iter_mut() {
            let Some(open) = slot.as_ref() else {
                continue;
            };

            if !open.visible {
                continue;
            }

            let close = (flags & CLOSE_ALL != 0 && open.flags & CLOSE_ALL != 0)
                || (flags & CLOSE_SAME_CONTEXT != 0
                    && context != 0
                    && open.flags & CLOSE_SAME_CONTEXT != 0
                    && open.context == context)
                || (flags & CLOSE_SAME_TEXT != 0
                    && context != 0
                    && open.flags & CLOSE_SAME_TEXT != 0
                    && open.context == context
                    && open.text == text);

            if close {
                *slot = None;
            }
        }

        let free = self
            .slots
            .iter()
            .position(|s| s.as_ref().is_none_or(|open| !open.visible))?;

        self.slots[free] = Some(Open {
            visible: true,
            kind: request.kind,
            context,
            flags,
            caption: strings.translate(&format!("ss_{}", request.caption_strdef())),
            text,
            script: script(flags)?,
            yes_no: request.yes_no(),
            timeout: None,
            progress: 0,
            edit_limit: 1000,
        });

        Some(free)
    }

    pub fn answer(&mut self, slot: usize, yes: bool) -> Option<(i32, u32, i32)> {
        let open = self.slots.get_mut(slot)?.take()?;

        Some((
            open.kind,
            open.context,
            if yes { ANSWER_YES } else { ANSWER_NO },
        ))
    }

    pub fn close(&mut self, slot: usize) {
        if let Some(entry) = self.slots.get_mut(slot) {
            *entry = None;
        }
    }

    pub fn hide(&mut self, slot: usize) {
        if let Some(Some(open)) = self.slots.get_mut(slot) {
            open.visible = false;
        }
    }

    pub fn hide_contexts(&mut self, first: u32, second: u32) -> Vec<usize> {
        let mut hidden = Vec::new();

        for (slot, open) in self.slots.iter_mut().enumerate() {
            if let Some(open) = open
                && open.visible
                && (open.context == first || (second != 0 && open.context == second))
            {
                open.visible = false;

                open.context = 0;

                hidden.push(slot);
            }
        }

        hidden
    }

    pub fn arm_notice_timeout(&mut self, slot: usize, now_ms: u32, lifetime_ms: u32) -> bool {
        let Some(Some(open)) = self.slots.get_mut(slot) else {
            return false;
        };

        if open.kind != 0 {
            return false;
        }

        open.timeout = Some((now_ms, lifetime_ms));

        open.progress = 0;

        true
    }

    pub fn set_edit_limit(&mut self, slot: usize, limit: i32) -> bool {
        let Some(Some(open)) = self.slots.get_mut(slot) else {
            return false;
        };

        open.edit_limit = limit;

        true
    }

    pub fn expire_notices(&mut self, now_ms: u32) -> Vec<usize> {
        let mut expired = Vec::new();

        for (slot, open) in self.slots.iter_mut().enumerate() {
            if let Some(o) = open.as_mut()
                && o.visible
                && o.flags & PANE_MASK == PANE_PROGRESS
                && let Some((start, duration)) = o.timeout
                && duration > 0
            {
                o.progress =
                    ((now_ms.wrapping_sub(start) as u64 * 1000 / duration as u64).min(1000)) as i32;
            }

            if open.as_ref().is_some_and(|o| {
                o.visible
                    && o.timeout.is_some_and(|(start, duration)| {
                        duration > 100 && now_ms.wrapping_sub(start) > duration
                    })
            }) {
                *open = None;

                expired.push(slot);
            }
        }

        expired
    }

    pub fn open(&self) -> impl Iterator<Item = (usize, &Open)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|o| (i, o)))
    }

    pub fn count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn find(&self, context: u32) -> Option<usize> {
        self.open()
            .find(|(_, o)| o.context == context)
            .map(|(i, _)| i)
    }

    pub fn topmost(&self) -> Option<(usize, &Open)> {
        self.open().filter(|(_, open)| open.visible).last()
    }
}

#[derive(Debug)]

pub struct BoxPanel {
    surfaces: [BoxSurface; SLOTS],
    top: usize,
}

impl std::ops::Deref for BoxPanel {
    type Target = BoxSurface;

    fn deref(&self) -> &BoxSurface {
        &self.surfaces[self.top]
    }
}

impl std::ops::DerefMut for BoxPanel {
    fn deref_mut(&mut self) -> &mut BoxSurface {
        &mut self.surfaces[self.top]
    }
}

impl BoxPanel {
    pub(crate) fn relocalize(&mut self, game: &GameDir, strings: &Strings) {
        self.for_each_ui(&mut |ui| ui.relocalize(game, strings));

        for surface in &mut self.surfaces {
            surface.strings = strings.clone();

            for (_, _, panel) in &mut surface.templates {
                panel.relocalize(strings);
            }
        }
    }

    #[cfg(test)]

    pub(crate) fn for_test() -> Self {
        Self {
            surfaces: std::array::from_fn(|_| BoxSurface {
                placed: PlacedPanel {
                    ui: PanelUi::from_resource(
                        ui::Panel::default(),
                        ui::Resource::default(),
                        &Chains::default(),
                        1.0,
                    ),
                    origin: (0, 0),
                },
                slot: None,
                alternates: Vec::new(),
                current: script(FLAGS).unwrap(),
                virtual_screen: (800, 600),
                templates: Vec::new(),
                strings: Strings::default(),
                edit_limit: 1000,
            }),
            top: 0,
        }
    }

    pub(super) fn reposition(&mut self, layout: &super::login_static::Layout) {
        let extent = layout.virtual_extent();

        for surface in &mut self.surfaces {
            surface.virtual_screen = extent;

            for placed in std::iter::once(&mut surface.placed)
                .chain(surface.alternates.iter_mut().map(|(_, p)| p))
            {
                placed.ui.scale = layout.ui_scale();

                placed.origin = BoxSurface::place(&placed.ui, extent);
            }
        }
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut crate::ui::PanelUi)) {
        for surface in &mut self.surfaces {
            visit(&mut surface.placed.ui);

            for (_, placed) in &mut surface.alternates {
                visit(&mut placed.ui);
            }
        }
    }

    pub fn build(
        game: &GameDir,
        strings: &Strings,
        chains: &Chains,
        zoom: f32,
        virtual_screen: (u32, u32),
    ) -> Result<Self> {
        let surfaces = (0..SLOTS)
            .map(|_| BoxSurface::build(game, strings, chains, zoom, virtual_screen))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .expect("five message-box surfaces");

        Ok(Self { surfaces, top: 0 })
    }

    pub fn visible(&self) -> impl Iterator<Item = (usize, &PlacedPanel)> {
        self.surfaces
            .iter()
            .enumerate()
            .filter(|(_, s)| s.placed.ui.panel.visible)
            .map(|(slot, s)| (slot, &s.placed))
    }

    pub fn visible_mut(&mut self) -> impl Iterator<Item = (usize, &mut PlacedPanel)> {
        self.surfaces
            .iter_mut()
            .enumerate()
            .filter(|(_, s)| s.placed.ui.panel.visible)
            .map(|(slot, s)| (slot, &mut s.placed))
    }

    pub fn frame_draws(
        &mut self,
        base: u32,
        captions: bool,
        interaction: &ui::draw::Interaction,
    ) -> Vec<crate::frame::PlannedQuad> {
        use crate::{
            frame::{PlannedQuad, TextureKey},
            ui::Source,
        };

        let top = self.top;

        let mut out = Vec::new();

        for (slot, placed) in self.visible_mut() {
            let state = if slot == top {
                *interaction
            } else {
                Default::default()
            };

            for draw in placed.ui.frame(&state, placed.origin) {
                if !captions && matches!(draw.source, Source::Caption(..)) {
                    continue;
                }

                let texture = match draw.source {
                    Source::Texture(slot) => TextureKey::GuiMat(slot),
                    Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                    Source::Amul(sheet) => TextureKey::Amul(sheet),
                    Source::Caption(owner, run) => {
                        TextureKey::Caption(caption_id(base, slot), owner, run)
                    }
                    Source::NamedIcon(owner) => {
                        TextureKey::NamedIcon(caption_id(base, slot), owner)
                    }
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

    pub fn textures(
        &mut self,
        game: &GameDir,
        missing: &mut Vec<i32>,
    ) -> Result<Vec<super::login_panels::GuiTexture>> {
        let surface = &mut self.surfaces[0];

        let mut textures = Vec::new();

        for placed in std::iter::once(&mut surface.placed)
            .chain(surface.alternates.iter_mut().map(|(_, placed)| placed))
        {
            let visible = placed.ui.panel.visible;

            placed.ui.panel.visible = true;

            let slots = placed.ui.referenced_textures(&Default::default());

            placed.ui.panel.visible = visible;

            for slot in slots {
                if let Some(texture) =
                    super::login_panels::load_guimat_texture(game, &placed.ui, slot)?
                {
                    textures.push(texture);
                } else if !missing.contains(&slot) {
                    missing.push(slot);
                }
            }
        }

        Ok(textures)
    }

    pub fn follow(&mut self, boxes: &Boxes) {
        for (slot, surface) in self.surfaces.iter_mut().enumerate() {
            match boxes.slots[slot].as_ref() {
                Some(open) if open.visible => surface.fill(slot, open),
                _ => surface.hide(),
            }
        }

        self.top = self
            .surfaces
            .iter()
            .rposition(|s| s.placed.ui.panel.visible)
            .unwrap_or(0);

        for (slot, surface) in self.surfaces.iter_mut().enumerate() {
            surface.placed.ui.panel.focus = if slot == self.top && surface.placed.ui.panel.visible {
                surface.placed.ui.panel.elements.iter().position(|e| {
                    e.name == ELEMENT_EDIT && e.visible && e.enabled && e.widget.edit().is_some()
                })
            } else {
                None
            };
        }
    }

    pub fn raised(&mut self, boxes: &Boxes, slot: usize) {
        if let Some(open) = boxes.slots.get(slot).and_then(Option::as_ref)
            && let Some(surface) = self.surfaces.get_mut(slot)
        {
            surface.reset(open.script);
        }

        self.follow(boxes);

        let Some(surface) = self.surfaces.get_mut(slot) else {
            return;
        };

        surface.placed.origin = BoxSurface::place(&surface.placed.ui, surface.virtual_screen);

        if let Some(edit) = surface
            .placed
            .ui
            .panel
            .element_mut(ELEMENT_EDIT, ElementType::ImeEditBox)
            .and_then(|element| element.widget.edit_mut())
        {
            edit.text.clear();

            edit.caret = 0;

            edit.anchor = 0;

            edit.scroll = 0;

            edit.insert = true;

            edit.preedit = None;
        }
    }
}

pub fn caption_id(base: u32, slot: usize) -> u32 {
    base + ((slot as u32) << 20)
}

#[derive(Debug)]

pub struct BoxSurface {
    pub placed: PlacedPanel,
    pub slot: Option<usize>,
    pub alternates: Vec<(&'static [u8], PlacedPanel)>,
    pub current: &'static [u8],
    virtual_screen: (u32, u32),
    templates: Vec<(&'static [u8], formats::ui::pane::Pane, ui::Panel)>,
    strings: Strings,
    edit_limit: i32,
}

impl BoxSurface {
    pub fn build(
        game: &GameDir,
        strings: &Strings,
        chains: &Chains,
        zoom: f32,
        virtual_screen: (u32, u32),
    ) -> Result<BoxSurface> {
        let script = script(FLAGS).expect("FLAGS names msgbox.pane");

        let mut ui = PanelUi::build(game, script, strings, chains, zoom)?;

        ui.panel.visible = false;

        debug_assert_eq!(ui.panel.z_order, Z_ORDER, "msgbox.pane's ZOrder");

        let origin = Self::place(&ui, virtual_screen);

        let mut alternates = Vec::new();

        let mut templates = vec![(
            script,
            formats::ui::pane::load(&String::from_utf8_lossy(script), &game.read(script)?)?,
            ui.panel.clone(),
        )];

        for alternate in ALTERNATE_PANES {
            let mut ui = PanelUi::build(game, alternate, strings, chains, zoom)?;

            ui.panel.visible = false;

            let origin = Self::place(&ui, virtual_screen);

            templates.push((
                alternate,
                formats::ui::pane::load(
                    &String::from_utf8_lossy(alternate),
                    &game.read(alternate)?,
                )?,
                ui.panel.clone(),
            ));

            alternates.push((alternate, PlacedPanel { ui, origin }));
        }

        Ok(BoxSurface {
            placed: PlacedPanel { ui, origin },
            slot: None,
            alternates,
            current: script,
            virtual_screen,
            templates,
            strings: strings.clone(),
            edit_limit: 1000,
        })
    }

    fn reset(&mut self, script: &'static [u8]) {
        self.swap_to(script);

        let (_, pane, _) = self
            .templates
            .iter()
            .find(|(name, _, _)| *name == script)
            .expect("loaded box template");

        self.placed.ui.panel =
            ui::Panel::instantiate(pane, &self.placed.ui.resource, &self.strings, false);

        self.placed.ui.panel.prepare_text_layouts();
    }

    fn swap_to(&mut self, script: &'static [u8]) {
        if self.current == script {
            return;
        }

        let Some(index) = self.alternates.iter().position(|(name, _)| *name == script) else {
            return;
        };

        let (name, panel) = self.alternates.remove(index);

        let mut previous = std::mem::replace(&mut self.placed, panel);

        previous.ui.panel.visible = false;

        self.alternates.push((self.current, previous));

        self.current = name;
    }

    pub fn edit_text(&self) -> Option<String> {
        let element = self
            .placed
            .ui
            .panel
            .elements
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(ELEMENT_EDIT))?;

        element.widget.edit().map(|edit| edit.text.clone())
    }

    pub fn edit_mut(&mut self) -> Option<&mut ui::EditState> {
        if !self.placed.ui.panel.visible {
            return None;
        }

        let element = self
            .placed
            .ui
            .panel
            .element_mut(ELEMENT_EDIT, ElementType::ImeEditBox)?;

        if !element.visible || !element.enabled || !element.msg_receive {
            return None;
        }

        element.widget.edit_mut()
    }

    pub fn edit_changed(&mut self, name: &str) -> bool {
        if name != ELEMENT_EDIT {
            return false;
        }

        let limit = self.edit_limit;

        let edit = self
            .placed
            .ui
            .panel
            .element_or_create(
                ELEMENT_EDIT,
                ElementType::ImeEditBox,
                &self.placed.ui.resource,
            )
            .widget
            .edit_mut()
            .expect("typed editor factory");

        if limit >= 1000 {
            return false;
        }

        let bytes = formats::text::codepage::encode(
            &edit.text,
            formats::text::codepage::Codepage::Windows1252,
        );

        let length = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());

        if limit >= 0 && length <= limit as usize {
            return false;
        }

        let changed = edit.text.pop().is_some();

        edit.scroll = 0;

        edit.caret = edit.text.chars().count() as i32;

        edit.anchor = edit.caret;

        edit.preedit = None;

        changed
    }

    pub fn character(
        &mut self,
        ch: char,
        clipboard: &mut dyn ui::text_entry::Clipboard,
    ) -> Option<Vec<ui::text_entry::Raised>> {
        if matches!(ch, '\r' | '\n' | '\u{1b}') {
            return None;
        }

        let raised = ui::text_entry::character(self.edit_mut()?, ch, clipboard);

        if raised.contains(&ui::text_entry::Raised::Changed) {
            self.edit_changed(ELEMENT_EDIT);
        }

        Some(raised)
    }

    pub fn key(
        &mut self,
        key: ui::text_entry::Key,
        mods: ui::text_entry::Mods,
        clipboard: &mut dyn ui::text_entry::Clipboard,
    ) -> Option<Vec<ui::text_entry::Raised>> {
        if matches!(
            key,
            ui::text_entry::Key::Return | ui::text_entry::Key::Escape
        ) {
            return None;
        }

        let raised = ui::text_entry::key(self.edit_mut()?, key, mods, clipboard);

        if raised.contains(&ui::text_entry::Raised::Changed) {
            self.edit_changed(ELEMENT_EDIT);
        }

        Some(raised)
    }

    pub fn place(ui: &PanelUi, virtual_screen: (u32, u32)) -> (i32, i32) {
        let screen = (virtual_screen.0 as i32, virtual_screen.1 as i32);

        let size = (ui.panel.width(), ui.panel.height());

        centre(size, [0, 0, screen.0, screen.1], screen)
    }

    pub fn covers(&self, request: &Request) -> bool {
        let Some(wanted) = script(promote(request.flags)) else {
            return false;
        };

        wanted == self.current || self.alternates.iter().any(|(name, _)| *name == wanted)
    }

    pub fn fill(&mut self, slot: usize, open: &Open) {
        self.swap_to(open.script);

        self.edit_limit = open.edit_limit;

        let (_, _, authored) = self
            .templates
            .iter()
            .find(|(name, _, _)| *name == open.script)
            .expect("loaded box template");

        let panel = &mut self.placed.ui.panel;

        panel
            .element_or_create(ELEMENT_MSG, ElementType::Static, &self.placed.ui.resource)
            .set_display(&open.text);

        if let Some(title) = panel.element_mut(ELEMENT_TITLE, ElementType::Static) {
            title.set_display(&open.caption);
        }

        if open.flags & PANE_MASK == PANE_EDITBOX {
            panel.element_or_create(
                ELEMENT_EDIT,
                ElementType::ImeEditBox,
                &self.placed.ui.resource,
            );
        }

        let nibble = open.flags & BUTTON_MASK;

        let ok_width = authored
            .element(ELEMENT_OK, ElementType::Button)
            .map(|e| e.width);

        for name in [ELEMENT_OK, ELEMENT_CANCEL] {
            if let Some(base) = authored.element(name, ElementType::Button)
                && let Some(button) = panel.element_mut(name, ElementType::Button)
            {
                button.visible = base.visible;

                button.x = base.x;

                button.is_default = base.is_default;

                if open.flags & CLEAR_BUTTON_DEFAULTS == CLEAR_BUTTON_DEFAULTS {
                    button.is_default = false;
                }

                if name == ELEMENT_OK {
                    if matches!(nibble, 0xD | 0xE)
                        || (nibble == 0xC && open.timeout.is_some_and(|(_, d)| d != 0))
                    {
                        button.visible = false;
                    }

                    if matches!(nibble, 0 | 0xC)
                        && let Some(width) = ok_width
                    {
                        button.x = centre_ok((authored.left, authored.right), width);
                    }
                } else {
                    if matches!(nibble, 0 | 0xC | 0xE) {
                        button.visible = false;
                    }

                    if nibble == 0xD
                        && let Some(width) = ok_width
                    {
                        button.x = centre_ok((authored.left, authored.right), width);
                    }
                }
            }
        }

        if open.flags & PANE_MASK == PANE_PROGRESS {
            let gauge = panel
                .element_or_create("gauge", ElementType::GaugeBar, &self.placed.ui.resource)
                .widget
                .gauge_mut()
                .expect("typed gauge factory");

            gauge.set_range(0, 1000);

            gauge.set_pos(open.progress);
        }

        panel.visible = true;

        self.slot = Some(slot);
    }

    pub fn hide(&mut self) {
        self.placed.ui.panel.visible = false;

        self.slot = None;
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<bool> {
        let (ox, oy) = self.placed.origin;

        let local = (x - ox, y - oy);

        for (name, yes) in [(ELEMENT_OK, true), (ELEMENT_CANCEL, false)] {
            let Some(element) = self.placed.ui.panel.element(name, ElementType::Button) else {
                continue;
            };

            if !element.visible || !element.enabled || !element.msg_receive {
                continue;
            }

            let [left, top, right, bottom] = element.rect();

            if (left..right).contains(&local.0) && (top..bottom).contains(&local.1) {
                return Some(yes);
            }
        }

        None
    }
}
