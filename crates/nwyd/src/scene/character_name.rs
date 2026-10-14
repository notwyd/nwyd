use super::{
    create,
    login_static::{self, Layout, LoadedTexture},
    stext::{self, SceneText},
};
use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
use anyhow::{Context, Result, ensure};
use assets::GameDir;
use formats::{
    scene::scene_ui,
    text::{Codepage, codepage},
    texture::{texture_list, texture_set_list},
};
use render::{
    device::Gpu,
    sprite::{Quad, UvRule},
};
use ui::{
    input::Mouse,
    text_entry::{self, Key, Mods, Raised},
    widget::{EditState, Widget},
};

pub const TEXT_BASE: u32 = 0x0410_0000;

#[derive(Debug)]

pub struct Dialog {
    pub visible: bool,
    pub pending: bool,
    pub prompt_required: bool,
    pub requested_at_ms: u32,
    pub edit: Box<EditState>,
    pub rect: [i32; 4],
    controls: Vec<(u32, [i32; 4], usize)>,
    background: (u32, texture_set_list::Item, LoadedTexture),
    text: SceneText,
    clipboard: text_entry::Local,
    pressed: [bool; 2],
    click_stamps: [u32; 2],
    click_sounds: Vec<super::audio::UnconditionalSound>,
    caret_frame: u8,
    focused: bool,
    legacy_focus_generation: u64,
    escape_releases: bool,
    position: Option<(f32, f32)>,
    drag_cursor: Option<(i32, i32)>,
    drag_capture: bool,
}

impl Dialog {
    pub fn relocalize(&mut self, _game: &assets::GameDir, _strings: &ui::Strings) {
        let keys: Vec<_> = self.text.iter().map(|(key, _)| *key).collect();

        for key in keys {
            self.text.forget(key);
        }
    }

    #[cfg(test)]

    pub(crate) fn for_test() -> Self {
        let Widget::Edit(edit) = Widget::new(formats::ui::pane::ElementType::EditBox) else {
            unreachable!()
        };

        Self {
            visible: false,
            pending: false,
            prompt_required: false,
            requested_at_ms: 0,
            edit,
            rect: [0, 0, 300, 160],
            controls: Vec::new(),
            background: super::login_panels::language_tests::background(),
            text: SceneText::new(&Default::default(), 800),
            clipboard: Default::default(),
            pressed: [false; 2],
            click_stamps: [0; 2],
            click_sounds: Vec::new(),
            caret_frame: 0,
            focused: false,
            legacy_focus_generation: 0,
            escape_releases: true,
            position: None,
            drag_cursor: None,
            drag_capture: false,
        }
    }

    pub(super) fn resize(&mut self, chains: &crate::fonts::Chains, old: &Layout, next: &Layout) {
        self.text = SceneText::new(chains, next.width);

        if let Some((x, y)) = &mut self.position {
            *x *= next.sx / old.sx;

            *y *= next.sy / old.sy;
        }

        self.cancel_pointer_input();
    }

    pub(super) fn cancel_pointer_input(&mut self) {
        self.pressed = [false; 2];

        self.drag_cursor = None;

        self.drag_capture = false;
    }

    pub fn build(game: &GameDir, width: u32) -> Result<Self> {
        let ui = scene_ui::parse(&game.read(br"UI\SelCharScene2.bin")?)?;

        let record = |id| {
            ui.records
                .iter()
                .find(|r| r.header.name_id == id)
                .with_context(|| format!("missing character-name RC{id:X}"))
        };

        use scene_ui::ControlType::{Button, Edit, Panel, Text};

        for (id, kind, parent) in [
            (0x620, Panel, 0),
            (0x274, Button, 0x620),
            (0x275, Button, 0x620),
            (0x624, Text, 0x620),
            (0x621, Edit, 0x620),
            (0x277, Panel, 0x620),
        ] {
            let r = record(id)?;

            ensure!(
                r.control == kind && r.header.parent_id == parent,
                "invalid character-name RC{id:X} type/parent"
            );
        }

        let rect = record(0x620)?.rect().context("RC620 rectangle")?;

        let mut controls = Vec::new();

        for id in [0x274, 0x275, 0x624, 0x621] {
            let r = record(id)?;

            let at = if id == 0x621 {
                r.fields.get(1..5).and_then(|v| v.try_into().ok())
            } else {
                r.rect()
            }
            .context("character-name control rectangle")?;

            let label = if id == 0x621 {
                0
            } else {
                *r.fields.last().context("control label")? as usize
            };

            controls.push((id, at, label));
        }

        let set = record(0x277)?
            .control_id()
            .context("dialog background atlas")?
            .unsigned_abs() as usize;

        let sets = texture_set_list::parse(&game.read(login_static::TEXTURE_SET_LIST)?)?;

        let item = *sets.item(set, 0).context("character-name background set")?;

        let slot = u32::try_from(item.texture_index)?;

        let list = texture_list::parse(&game.read(login_static::TEXTURE_LIST)?)?;

        let loaded = login_static::load_texture(game, &list, slot)?
            .context("character-name background texture")?;

        let Widget::Edit(mut edit) = Widget::new(formats::ui::pane::ElementType::EditBox) else {
            unreachable!()
        };

        edit.range = record(0x621)?
            .fields
            .get(10)
            .copied()
            .context("name edit maximum")?
            .max(0);

        edit.text = record(0x621)?.default_text().unwrap_or_default();

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        let config = game
            .read(b"Config.bin")
            .ok()
            .and_then(|bytes| formats::config::config_bin::parse(&bytes).ok())
            .unwrap_or_default();

        Ok(Self {
            visible: false,
            pending: false,
            prompt_required: false,
            requested_at_ms: 0,
            edit,
            rect,
            controls,
            background: (slot, item, loaded),
            text: SceneText::new(&chains, width),
            clipboard: text_entry::Local::default(),
            pressed: [false; 2],
            click_stamps: [0; 2],
            click_sounds: Vec::new(),
            caret_frame: 0,
            focused: false,
            legacy_focus_generation: 0,
            position: None,
            drag_cursor: None,
            drag_capture: false,
            escape_releases: config.raw[formats::config::config_bin::slot::CONTROL_SCHEME] == 0,
        })
    }

    pub fn open(&mut self) {
        self.visible = true;

        self.focused = true;
    }

    pub fn close(&mut self) {
        self.visible = false;

        self.focused = false;

        self.drag_capture = false;
    }

    pub fn cancel(&mut self) {
        self.close();

        self.pending = false;
    }

    pub fn has_focus(&self) -> bool {
        self.visible && self.focused
    }

    pub(super) fn owns_drag_capture(&self) -> bool {
        self.visible && self.drag_capture
    }

    pub(super) fn observe_legacy_focus(&mut self, generation: u64) {
        if generation != self.legacy_focus_generation {
            self.focused = false;
        }

        self.legacy_focus_generation = generation;
    }

    pub(super) fn begin_frame(&mut self, captions: bool) {
        if self.has_focus() && captions {
            self.caret_frame = (self.caret_frame + 1) % 20;
        }
    }

    pub fn prompt_due(&self, now_ms: u32) -> bool {
        self.prompt_required && !self.pending && now_ms.wrapping_sub(self.requested_at_ms) >= 1000
    }

    pub fn character(&mut self, ch: char) -> Vec<Raised> {
        if !self.has_focus() {
            return Vec::new();
        }

        if ch == '%' {
            return Vec::new();
        }

        if ch == '\u{1b}' {
            self.escape();

            return Vec::new();
        }

        text_entry::character(&mut self.edit, ch, &mut self.clipboard)
    }

    pub fn key(&mut self, key: Key, mods: Mods) -> Vec<Raised> {
        if !self.has_focus() {
            return Vec::new();
        }

        if key == Key::Escape {
            self.escape();

            return Vec::new();
        }

        text_entry::key(&mut self.edit, key, mods, &mut self.clipboard)
    }

    fn escape(&mut self) {
        if !self.escape_releases {
            return;
        }

        self.edit.text.clear();

        self.edit.caret = 0;

        self.edit.anchor = 0;

        self.edit.preedit = None;

        self.focused = false;
    }

    pub fn mouse(
        &mut self,
        event: Mouse,
        x: i32,
        y: i32,
        layout: &Layout,
        now_ms: u32,
    ) -> Option<u32> {
        if !self.visible {
            return None;
        }

        let (ox, oy) = self.origin(layout);

        match event {
            Mouse::Move => {
                if let Some((px, py)) = self.drag_cursor {
                    self.position = Some((
                        (ox + (x - px) as f32)
                            .max(0.0)
                            .min(layout.width as f32 - self.rect[2] as f32 * layout.sx),
                        (oy + (y - py) as f32)
                            .max(0.0)
                            .min(layout.height as f32 - self.rect[3] as f32 * layout.sy),
                    ));

                    self.drag_cursor = Some((x, y));
                }
            }
            Mouse::LeftDown
                if !self.drag_capture
                    && x as f32 >= ox
                    && y as f32 >= oy
                    && (x as f32) < ox + self.rect[2] as f32 * layout.sx
                    && (y as f32) < oy + 24.0 =>
            {
                self.drag_capture = true;

                self.drag_cursor = Some((x, y));
            }
            Mouse::LeftUp => {
                self.drag_capture = false;

                self.drag_cursor = None;
            }
            _ => {}
        }

        let (ox, oy) = self.origin(layout);

        let hit = self
            .controls
            .iter()
            .find(|(_, r, _)| {
                let mut r = layout.control(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32);

                r.x += ox.trunc();

                r.y += oy.trunc();

                x as f32 >= r.x
                    && y as f32 >= r.y
                    && (x as f32) < r.x + r.w
                    && (y as f32) < r.y + r.h
            })
            .map(|r| r.0);

        match event {
            Mouse::LeftDown => {
                if let Some(id @ (0x274 | 0x275)) = hit {
                    self.pressed[(id - 0x274) as usize] = true;
                }

                if hit == Some(0x621) {
                    self.focused = true;
                }
            }
            Mouse::LeftUp => {
                for id in [0x275, 0x274] {
                    let index = (id - 0x274) as usize;

                    if hit != Some(id) {
                        self.pressed[index] = false;

                        continue;
                    }

                    if self.pressed[index] && self.click_stamps[index].wrapping_add(500) < now_ms {
                        self.click_sounds
                            .push(super::audio::UnconditionalSound::new(53));

                        return Some(id);
                    }
                }
            }
            _ => {}
        }

        None
    }

    pub(super) fn finish_mouse(&mut self, id: u32, now_ms: u32) {
        if let Some(index) = [0x274, 0x275].iter().position(|&value| value == id) {
            self.click_stamps[index] = now_ms;

            self.pressed[index] = false;
        }
    }

    pub(super) fn take_click_sounds(&mut self) -> Vec<super::audio::UnconditionalSound> {
        std::mem::take(&mut self.click_sounds)
    }

    pub fn upload(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        let (slot, _, loaded) = &self.background;

        renderer.upload_once(
            gpu,
            TextureKey::Scene(*slot),
            &loaded.texture,
            loaded.alpha,
            "character-name dialog",
        );
    }

    pub fn sync(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        renderer.sync_scene_text(gpu, &self.text);
    }

    pub fn ime_area(&self, layout: &Layout) -> [f64; 4] {
        let (_, at, _) = self
            .controls
            .iter()
            .find(|c| c.0 == 0x621)
            .expect("validated name edit");

        let r = self.child_rect(layout, *at);

        [r.x as f64, r.y as f64, r.w as f64, r.h as f64]
    }

    fn origin(&self, layout: &Layout) -> (f32, f32) {
        self.position.unwrap_or((
            self.rect[0] as f32 * layout.sx,
            self.rect[1] as f32 * layout.sy,
        ))
    }

    fn child_rect(&self, layout: &Layout, at: [i32; 4]) -> login_static::Rect {
        let mut r = layout.control(at[0] as f32, at[1] as f32, at[2] as f32, at[3] as f32);

        let (x, y) = self.origin(layout);

        r.x += x;

        r.y += y;

        r
    }

    pub fn frame_draws(
        &mut self,
        layout: &Layout,
        strings: &ui::Strings,
        captions: bool,
    ) -> Vec<PlannedQuad> {
        if !self.visible {
            for id in [0x274, 0x275, 0x624, 0x621, 0x622] {
                self.text.forget(TEXT_BASE + id);
            }

            return Vec::new();
        }

        let mut out = Vec::new();

        let shown = text_entry::presentation(&self.edit);

        let r = self.child_rect(layout, [0, 0, self.rect[2], self.rect[3]]);

        let (slot, item, _) = &self.background;

        out.push(draw(
            TextureKey::Scene(*slot),
            [r.x, r.y, r.w, r.h],
            [item.x as f32, item.y as f32, item.w as f32, item.h as f32],
            0xFFFF_FFFF,
        ));

        for (id, at, label) in &self.controls {
            let r = self.child_rect(layout, *at);

            if *id != 0x624 {
                out.push(draw(
                    TextureKey::White,
                    [r.x, r.y, r.w, r.h],
                    [0.0, 0.0, 1.0, 1.0],
                    if *id == 0x621 {
                        0xCC10_1010
                    } else {
                        0xCC40_4040
                    },
                ));
            }

            if !captions {
                continue;
            }

            let value = if *id == 0x621 {
                shown.masked().into_owned()
            } else {
                stext::display_text(&strings.translate(&format!("uu_{label}")))
            };

            let key = TEXT_BASE + id;

            if !value.is_empty()
                && self.text.resolve(key, &value, 0xFFFF_FFFF)
                && let Some(line) = self.text.get(key)
            {
                let x = if *id == 0x621 {
                    r.x + 2.0
                } else {
                    r.x + (r.w - (line.rect[2] - line.rect[0]) as f32) * 0.5
                };

                let y = r.y + (r.h - stext::LINE_BOX * layout.sy) * 0.5;

                if matches!(*id, 0x274 | 0x275) {
                    out.push(PlannedQuad {
                        mode: render::preset::RenderMode::Ui2dPoint,
                        texture: TextureKey::SceneText(key),
                        quad: stext::shadow_quad(line, (x, y)),
                        uv: UvRule::TexelCentre,
                    });
                }

                out.push(PlannedQuad {
                    mode: super::field::S_MODE,
                    texture: TextureKey::SceneText(key),
                    quad: stext::quad(line, (x, y)),
                    uv: UvRule::TexelCentre,
                });
            }

            if *id == 0x621 {
                let prefix: String = value.chars().take(shown.caret.max(0) as usize).collect();

                let key = TEXT_BASE + 0x622;

                let width = if !prefix.is_empty() && self.text.resolve(key, &prefix, 0xFFFF_FFFF) {
                    self.text
                        .get(key)
                        .map_or(0.0, |l| (l.rect[2] - l.rect[0]) as f32)
                } else {
                    0.0
                };

                if self.focused
                    && self.caret_frame >= 10
                    && shown.caret_visible
                    && !shown.caret_suppressed
                {
                    out.push(draw(
                        TextureKey::White,
                        [r.x + 2.0 + width.min((r.w - 3.0).max(0.0)), r.y, 1.0, r.h],
                        [0.0, 0.0, 1.0, 1.0],
                        0xFFFF_FFFF,
                    ));
                }
            }
        }

        out
    }
}

fn draw(texture: TextureKey, d: [f32; 4], s: [f32; 4], color: u32) -> PlannedQuad {
    PlannedQuad {
        mode: super::field::S_MODE,
        texture,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: d[0],
            dst_y: d[1],
            dst_w: d[2],
            dst_h: d[3],
            src_x: s[0],
            src_y: s[1],
            src_w: s[2],
            src_h: s[3],
            color,
            rotation: 0.0,
        },
        uv: UvRule::TexelCentre,
    }
}

pub fn checked_name(
    text: &str,
    reserved: &create::Reserved,
) -> Result<Vec<u8>, create::NameReject> {
    validate_name(&mut text.to_owned(), reserved)
}

pub(super) fn validate_name(
    text: &mut String,
    reserved: &create::Reserved,
) -> Result<Vec<u8>, create::NameReject> {
    let mut bytes = codepage::encode(text, Codepage::Windows1252);

    if bytes.len() < 4 {
        return Err(create::NameReject::TooShort);
    }

    if bytes.len() > 12 {
        return Err(create::NameReject::TooLong);
    }

    if !create::charset_ok(&bytes) || !create::printable_ok(&bytes, Codepage::Windows1252) {
        return Err(create::NameReject::Invalid);
    }

    let original = bytes.clone();

    reserved.substitute(&mut bytes, Codepage::Windows1252);

    if bytes != original {
        *text = codepage::decode(&bytes, Codepage::Windows1252);
    }

    if bytes.windows(2).any(|pair| pair == [0xA1, 0xA1]) {
        return Err(create::NameReject::Invalid);
    }

    Ok(bytes)
}

pub fn packet(slot: usize, old: &[u8; 16], name: &[u8]) -> [u8; 52] {
    let mut out = [0u8; 52];

    out[4..6].copy_from_slice(&[0xAA, 0x0F]);

    out[16..20].copy_from_slice(&(slot as u32).to_le_bytes());

    let n = old.iter().position(|b| *b == 0).unwrap_or(15).min(15);

    out[20..20 + n].copy_from_slice(&old[..n]);

    let n = name.len().min(15);

    out[36..36 + n].copy_from_slice(&name[..n]);

    out
}
