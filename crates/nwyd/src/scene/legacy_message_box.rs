use super::{
    login_static::{self, Layout, LoadedTexture},
    stext::{self, SceneText},
};
use crate::frame::{PlannedQuad, SceneRenderer, TextureKey};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::texture::{texture_list, texture_set_list};
use render::{
    device::Gpu,
    sprite::{Quad, UvRule},
};
use ui::input::Mouse;

pub const TEXT_BASE: u32 = 0x0440_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Question,
    Acknowledgement,
}

impl Owner {
    fn index(self) -> usize {
        match self {
            Self::Question => 0,
            Self::Acknowledgement => 1,
        }
    }
}

fn button_index(owner: Owner, no: bool) -> usize {
    match owner {
        Owner::Question => usize::from(no),
        Owner::Acknowledgement => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub visible: bool,
    pub context: u32,
    pub lines: [String; 3],
}

impl Default for State {
    fn default() -> Self {
        Self {
            visible: false,
            context: u32::MAX,
            lines: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    pub owner: Owner,
    pub context: u32,
    pub no: bool,
}

#[derive(Debug)]
pub struct Dialogs {
    pub states: [State; 2],
    background: (u32, texture_set_list::Item, LoadedTexture),
    text: SceneText,
    pressed: [bool; 3],
    click_stamps: [u32; 3],
    click_sounds: Vec<super::audio::UnconditionalSound>,
    focus: Option<Owner>,
    keyboard_focus_allowed: bool,
    focus_generation: u64,
    positions: [Option<(f32, f32)>; 2],
    dragging: Option<(Owner, i32, i32)>,
    completion: [Option<super::field::Transition>; 2],
    pending: Option<super::field::Transition>,
}

impl Dialogs {
    pub fn relocalize(&mut self, _game: &assets::GameDir, _strings: &ui::Strings) {
        let keys: Vec<_> = self.text.iter().map(|(key, _)| *key).collect();

        for key in keys {
            self.text.forget(key);
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            states: Default::default(),
            background: super::login_panels::language_tests::background(),
            text: SceneText::new(&Default::default(), 800),
            pressed: [false; 3],
            click_stamps: [0; 3],
            click_sounds: Vec::new(),
            focus: None,
            keyboard_focus_allowed: true,
            focus_generation: 0,
            positions: [None; 2],
            dragging: None,
            completion: [None; 2],
            pending: None,
        }
    }

    pub(super) fn cancel_pointer_input(&mut self) {
        self.pressed = [false; 3];

        self.dragging = None;
    }

    pub(crate) fn resize(&mut self, chains: &crate::fonts::Chains, old: &Layout, next: &Layout) {
        self.text = SceneText::new(chains, next.width);

        for (x, y) in self.positions.iter_mut().flatten() {
            *x *= next.sx / old.sx;

            *y *= next.sy / old.sy;
        }

        self.dragging = None;
    }

    pub fn build(game: &GameDir, width: u32) -> Result<Self> {
        let sets = texture_set_list::parse(&game.read(login_static::TEXTURE_SET_LIST)?)?;

        let item = *sets
            .item(501, 0)
            .context("legacy message-box backdrop501")?;

        let slot = u32::try_from(item.texture_index)?;

        let list = texture_list::parse(&game.read(login_static::TEXTURE_LIST)?)?;

        let loaded =
            login_static::load_texture(game, &list, slot)?.context("legacy message-box texture")?;

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        let config = game
            .read(b"Config.bin")
            .ok()
            .and_then(|bytes| formats::config::config_bin::parse(&bytes).ok())
            .unwrap_or_default();

        Ok(Self {
            states: Default::default(),
            background: (slot, item, loaded),
            text: SceneText::new(&chains, width),
            pressed: [false; 3],
            click_stamps: [0; 3],
            click_sounds: Vec::new(),
            focus: None,
            focus_generation: 0,
            keyboard_focus_allowed: config.raw[formats::config::config_bin::slot::CONTROL_SCHEME]
                != 1,
            positions: [None; 2],
            dragging: None,
            completion: [None; 2],
            pending: None,
        })
    }

    pub fn active(&self) -> Option<Owner> {
        if self.states[0].visible {
            Some(Owner::Question)
        } else if self.states[1].visible {
            Some(Owner::Acknowledgement)
        } else {
            None
        }
    }

    pub fn show(&mut self, owner: Owner, first: &str, context: u32, second: Option<&str>) {
        self.set_text(owner, first, context, second);

        self.states[owner.index()].visible = true;

        self.focus(owner);
    }

    pub fn set_text(&mut self, owner: Owner, first: &str, context: u32, second: Option<&str>) {
        let first = first.split('\0').next().unwrap_or_default();

        let lines = if let Some(second) = second {
            [first.to_owned(), second.to_owned(), String::new()]
        } else if first.chars().count() >= 35 {
            let split = first
                .char_indices()
                .nth(33)
                .map(|(index, _)| index)
                .unwrap_or(first.len());

            [
                first[..split].to_owned(),
                first[split..].to_owned(),
                String::new(),
            ]
        } else {
            [first.to_owned(), String::new(), String::new()]
        };

        let lines = lines.map(|line| super::stext::display_text(&line));

        let visible = self.states[owner.index()].visible;

        self.states[owner.index()] = State {
            visible,
            context,
            lines,
        };

        self.completion[owner.index()] = None;
    }

    pub fn answer(&mut self, owner: Owner, no: bool) -> Option<Answer> {
        let state = &mut self.states[owner.index()];

        if !state.visible {
            return None;
        }

        let context = state.context;

        state.visible = false;

        self.release_focus();

        self.dragging = None;

        if let Some(transition) = self.completion[0] {
            self.pending = Some(transition);
        }

        Some(Answer { owner, context, no })
    }

    pub fn hide(&mut self, owner: Owner) {
        self.states[owner.index()].visible = false;

        self.release_focus();

        self.dragging = None;
    }

    pub fn close_after_answer(&mut self, owner: Owner) {
        self.completion[owner.index()] = Some(super::field::Transition::Quit);
    }

    pub fn take_transition(&mut self) -> Option<super::field::Transition> {
        self.pending.take()
    }

    pub fn show_slashes(&mut self, owner: Owner, text: &str, context: u32) {
        self.set_slashes(owner, text, context);

        self.states[owner.index()].visible = true;

        self.focus(owner);
    }

    pub fn set_slashes(&mut self, owner: Owner, text: &str, context: u32) {
        let mut parts = text.split('\0').next().unwrap_or_default().split('/');

        let lines =
            std::array::from_fn(|_| super::stext::display_text(parts.next().unwrap_or_default()));

        let visible = self.states[owner.index()].visible;

        self.states[owner.index()] = State {
            visible,
            context,
            lines,
        };

        self.completion[owner.index()] = None;
    }

    pub fn selection_console(&mut self, line: &protocol::server::ChatLine) -> bool {
        if line.opcode != 0x101 || line.speaker != 0 {
            return false;
        }

        let Some(body) = line.text.strip_prefix('^') else {
            return false;
        };

        self.show(Owner::Acknowledgement, body, 0, None);

        true
    }

    pub fn character(&mut self, ch: char) -> Option<Answer> {
        let owner = self.focus?;

        match ch {
            '\r' | 'Y' | 'y' => self.answer(owner, false),
            'N' | 'n' => self.answer(owner, true),
            _ => None,
        }
    }

    pub fn button_rect(&self, owner: Owner, no: bool, layout: &Layout) -> Option<[f32; 4]> {
        if owner == Owner::Acknowledgement && no {
            return None;
        }

        let (x, y, w, h) = if owner == Owner::Acknowledgement {
            (70.0, 76.0, 88.0, 23.0)
        } else if no {
            (137.0, 132.0, 103.0, 21.0)
        } else {
            (17.0, 132.0, 103.0, 21.0)
        };

        let (ox, oy) = self.origin(owner, layout);

        let r = layout.control(x, y, w, h);

        Some([ox + r.x, oy + r.y, r.w, r.h])
    }

    pub fn mouse(
        &mut self,
        event: Mouse,
        x: i32,
        y: i32,
        layout: &Layout,
        now_ms: u32,
    ) -> Option<Answer> {
        self.mouse_with_capture(event, (x, y), layout, now_ms, false)
    }

    pub(super) fn mouse_with_capture(
        &mut self,
        event: Mouse,
        (x, y): (i32, i32),
        layout: &Layout,
        now_ms: u32,
        external_capture: bool,
    ) -> Option<Answer> {
        let owner = self.active()?;

        if event == Mouse::Move
            && let Some((moving, px, py)) = self.dragging
        {
            let (ox, oy) = self.origin(moving, layout);

            self.positions[moving.index()] = Some((
                (ox + (x - px) as f32)
                    .max(0.0)
                    .min((layout.width as f32 - 256.0 * layout.sx).max(0.0)),
                (oy + (y - py) as f32)
                    .max(0.0)
                    .min((layout.height as f32 - 152.0 * layout.sy).max(0.0)),
            ));

            self.dragging = Some((moving, x, y));

            return None;
        }

        let hit = [false, true].into_iter().find(|no| {
            self.button_rect(owner, *no, layout).is_some_and(|r| {
                x as f32 >= r[0]
                    && y as f32 >= r[1]
                    && (x as f32) < r[0] + r[2]
                    && (y as f32) < r[1] + r[3]
            })
        });

        match event {
            Mouse::LeftDown => {
                if let Some(no) = hit {
                    self.pressed[button_index(owner, no)] = true;
                }

                let (ox, oy) = self.origin(owner, layout);

                if x as f32 >= ox
                    && y as f32 >= oy
                    && (x as f32) < ox + 256.0 * layout.sx
                    && (y as f32) < oy + 152.0 * layout.sy
                {
                    self.focus(owner);

                    if !external_capture && (y as f32) < oy + 24.0 {
                        self.dragging = Some((owner, x, y));
                    }
                }
            }
            Mouse::LeftUp => {
                self.dragging = None;

                for no in [true, false] {
                    if self.button_rect(owner, no, layout).is_none() {
                        continue;
                    }

                    let index = button_index(owner, no);

                    if hit != Some(no) {
                        self.pressed[index] = false;

                        continue;
                    }

                    if self.pressed[index] && self.click_stamps[index].wrapping_add(500) < now_ms {
                        self.click_sounds
                            .push(super::audio::UnconditionalSound::new(53));

                        let answer = self.answer(owner, no);

                        self.click_stamps[index] = now_ms;

                        self.pressed[index] = false;

                        return answer;
                    }
                }
            }
            _ => {}
        }

        None
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
            "legacy message box",
        );
    }

    pub fn sync(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        renderer.sync_scene_text(gpu, &self.text);
    }

    pub fn frame_draws(
        &mut self,
        layout: &Layout,
        strings: &ui::Strings,
        captions: bool,
    ) -> Vec<PlannedQuad> {
        let mut out = Vec::new();

        for owner in [Owner::Acknowledgement, Owner::Question] {
            let (ox, oy) = self.origin(owner, layout);

            let index = owner.index();

            let base = TEXT_BASE + index as u32 * 16;

            if !self.states[index].visible {
                for key in base..base + 6 {
                    self.text.forget(key);
                }

                continue;
            }

            let (slot, item, _) = &self.background;

            out.push(draw(
                TextureKey::Scene(*slot),
                [ox, oy, 256.0 * layout.sx, 172.0 * layout.sy],
                [item.x as f32, item.y as f32, item.w as f32, item.h as f32],
                0x7777_7777,
            ));

            let mut labels = vec![(base, strings.translate("ss_237"), [20.0, 9.0, 216.0, 22.0])];

            for (line, text) in self.states[index].lines.iter().enumerate() {
                labels.push((
                    base + 1 + line as u32,
                    text.clone(),
                    [20.0, 36.0 + line as f32 * 16.0, 216.0, 16.0],
                ));
            }

            for no in [false, true] {
                let Some(r) = self.button_rect(owner, no, layout) else {
                    continue;
                };

                out.push(draw(
                    TextureKey::White,
                    r,
                    [0.0, 0.0, 1.0, 1.0],
                    if self.pressed[button_index(owner, no)] {
                        0xDD60_6060
                    } else {
                        0xDD40_4040
                    },
                ));

                let text = strings.translate(if no { "ss_239" } else { "ss_238" });

                let key = base + 4 + u32::from(no);

                if captions {
                    self.label(&mut out, key, &text, r, layout, true);
                }
            }

            if captions {
                for (key, text, r) in labels {
                    let r = layout.control(r[0], r[1], r[2], r[3]);

                    self.label(
                        &mut out,
                        key,
                        &text,
                        [ox + r.x, oy + r.y, r.w, r.h],
                        layout,
                        false,
                    );
                }
            }
        }

        out
    }

    fn origin(&self, owner: Owner, layout: &Layout) -> (f32, f32) {
        self.positions[owner.index()].unwrap_or_else(|| origin(layout))
    }

    fn focus(&mut self, owner: Owner) {
        if self.keyboard_focus_allowed {
            self.focus = Some(owner);

            self.focus_generation = self.focus_generation.wrapping_add(1);
        }
    }

    fn release_focus(&mut self) {
        self.focus = None;

        if self.keyboard_focus_allowed {
            self.focus_generation = self.focus_generation.wrapping_add(1);
        }
    }

    pub(super) fn focus_generation(&self) -> u64 {
        self.focus_generation
    }

    pub(super) fn yield_keyboard_to_edit(&mut self) {
        self.focus = None;
    }

    fn label(
        &mut self,
        out: &mut Vec<PlannedQuad>,
        key: u32,
        text: &str,
        r: [f32; 4],
        layout: &Layout,
        shadow: bool,
    ) {
        if self.text.resolve(key, text, 0xFFFF_FFFF)
            && let Some(line) = self.text.get(key)
        {
            let width = (line.rect[2] - line.rect[0]) as f32;

            let at = (
                r[0] + ((r[2] - width) * 0.5).max(0.0),
                r[1] + (r[3] - stext::LINE_BOX * layout.sy) * 0.5,
            );

            let clip = Some([r[0], r[1], r[0] + r[2], r[1] + r[3]]);

            if shadow {
                let mut quad = stext::shadow_quad(line, at);

                quad.clip = clip;

                out.push(PlannedQuad {
                    mode: render::preset::RenderMode::Ui2dPoint,
                    texture: TextureKey::SceneText(key),
                    quad,
                    uv: UvRule::TexelCentre,
                });
            }

            let mut quad = stext::quad(line, at);

            quad.clip = clip;

            out.push(PlannedQuad {
                mode: super::field::S_MODE,
                texture: TextureKey::SceneText(key),
                quad,
                uv: UvRule::TexelCentre,
            });
        }
    }
}

fn origin(layout: &Layout) -> (f32, f32) {
    (
        (layout.width as f32 - 256.0 * layout.sx) * 0.5,
        (layout.height as f32 - 152.0 * layout.sy) * 0.5,
    )
}

fn draw(texture: TextureKey, r: [f32; 4], s: [f32; 4], color: u32) -> PlannedQuad {
    PlannedQuad {
        mode: super::field::S_MODE,
        texture,
        quad: Quad {
            normalized_corners: None,
            clip: None,
            dst_x: r[0],
            dst_y: r[1],
            dst_w: r[2],
            dst_h: r[3],
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

#[cfg(test)]
mod language_tests;
