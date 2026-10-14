use super::{
    field::{FieldScene, S_MODE},
    item_move::{self, Coord},
    stext,
};
use crate::frame::{PlannedQuad, TextureKey};
use formats::{
    data::item_effect::Effects,
    net::account::Item,
    scene::scene_ui::{ControlType, SceneUi},
};
use render::sprite::{Quad, UvRule};
use ui::input::Mouse;

pub const PANEL: u32 = 0x2201;

pub const FIRST_CELL: u32 = 0x2202;

pub const CANCEL: u32 = 0x2267;

pub const SEND: u32 = 0x2269;

pub const CLEAR: u32 = 0x226a;

pub const FIRST_PRESET: u32 = 0x226b;

pub const TEXT_BASE: u32 = 0x0420_0000;

const PRESETS: [&[u8; 100]; 5] = [
    b"1111111111100000000110100001011101001011100000000110000000011010000101100111100110000000011111111111",
    b"0000000000011000011010010010011000110001100000000110000000010100000010001000010000010010000000110000",
    b"0000110000000011000000011110001111111111011111111000111111000011111100011100111001100001101000000001",
    b"0001111000001111110001100001100110000110000000110000001110000000110000000000000000001100000000110000",
    b"0000110000000111100000011110000001111000000111100000011110000000110000000000000000001100000000110000",
];

#[derive(Debug)]

pub struct State {
    pub visible: bool,
    pub source: Option<Coord>,
    pub selected: [bool; 100],
    pub enabled: [bool; 108],
    pressed: [bool; 108],
    hovered: Option<usize>,
    click_stamps: [u32; 108],
}

impl Default for State {
    fn default() -> Self {
        Self {
            visible: false,
            source: None,
            selected: [false; 100],
            enabled: [true; 108],
            pressed: [false; 108],
            hovered: None,
            click_stamps: [0; 108],
        }
    }
}

impl State {
    pub fn open(&mut self, slot: i32) {
        self.visible = true;

        self.source = Some(Coord { container: 1, slot });
    }

    pub fn cancel(&mut self) {
        self.visible = false;

        self.source = None;
    }

    pub fn preset(&mut self, index: usize) {
        if let Some(pattern) = PRESETS.get(index) {
            for (selected, &value) in self.selected.iter_mut().zip(pattern.iter()) {
                *selected = value == b'1';
            }
        }
    }

    pub fn bits(&self) -> [u8; 13] {
        let mut out = [0u8; 13];

        for (i, &on) in self.selected.iter().enumerate() {
            if on {
                out[i / 8] |= 1 << (i % 8);
            }
        }

        out
    }

    fn frame(&self, index: usize) -> usize {
        if !self.enabled[index] {
            4
        } else if index < 100 && self.selected[index] {
            3
        } else if self.hovered == Some(index) {
            if self.pressed[index] { 2 } else { 1 }
        } else {
            0
        }
    }
}

pub fn rectangle(ui: &SceneUi) -> Option<[i32; 4]> {
    let r = ui
        .records
        .iter()
        .find(|r| r.header.name_id == PANEL && r.control == ControlType::Panel)?;

    let [_, _, w, h] = r.rect()?;

    Some([(800 - w) / 2, (600 - h) / 2, w, h])
}

pub fn drain_open(scene: &mut FieldScene) {
    if let Some(slot) = scene
        .panels
        .as_mut()
        .and_then(|p| p.firecracker_slot.take())
    {
        if rectangle(&scene.ui).is_some() {
            scene.firecracker.open(slot);
        } else {
            tracing::warn!("Premium Firecracker has no RC2201 layout owner");
        }
    }
}

pub fn packet(client: u16, slot: i32, kind: i32, at: (u16, u16), bits: [u8; 13]) -> Vec<u8> {
    let mut out = vec![0u8; 52];

    out[4..6].copy_from_slice(&0x3c9u16.to_le_bytes());

    out[6..8].copy_from_slice(&client.to_le_bytes());

    out[12..16].copy_from_slice(&1i32.to_le_bytes());

    out[16..20].copy_from_slice(&slot.to_le_bytes());

    if kind == 15 {
        out[24..28].copy_from_slice(&14i32.to_le_bytes());
    }

    out[28..30].copy_from_slice(&at.0.to_le_bytes());

    out[30..32].copy_from_slice(&at.1.to_le_bytes());

    out[34..47].copy_from_slice(&bits);

    out
}

pub fn send(scene: &mut FieldScene) -> bool {
    if rectangle(&scene.ui).is_none() {
        return false;
    }

    let Some(at) = scene
        .firecracker
        .source
        .filter(|a| a.container == 1 && a.slot >= 0)
    else {
        return false;
    };

    let Some(body) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return false;
    };

    let tile = (body.position.x as i32 as u16, body.position.z as i32 as u16);

    let Some(p) = scene.panels.as_mut() else {
        return false;
    };

    if p.cooldowns.running(0, scene.time.now_ms) {
        return false;
    }

    let Some(item) = item_move::item_at(&scene.entered, at).filter(|i| !i.is_empty()) else {
        return false;
    };

    let kind = Effects::new(&p.items, &[]).total_scaled(&item, 0x26);

    scene.outbox.push(packet(
        scene.entered.client_id,
        at.slot,
        kind,
        tile,
        scene.firecracker.bits(),
    ));

    p.cooldowns.stamp(0, scene.time.now_ms);

    let count = if (1..=500).contains(&item.appearance) {
        item.appearance
    } else {
        1
    };

    if let Some(item) = scene.entered.carry.get_mut(at.slot as usize) {
        if count < 2 {
            *item = Item::default();
        } else {
            item.appearance = count - 1;
        }
    }

    super::panels::refresh_inventory_hover(p, &scene.entered, at.slot);

    super::panels::refine_item_changed(p, &scene.entered);

    scene.firecracker.source = None;

    if kind != 19 {
        p.notice
            .sounds
            .push(if (11..=13).contains(&kind) { 54 } else { 41 });
    }

    super::screens::refresh_all(p, &scene.entered);

    true
}

fn controls(ui: &SceneUi) -> Vec<(u32, [i32; 4], i32, usize, u32)> {
    let mut out = Vec::with_capacity(108);

    for i in 0..100 {
        out.push((
            FIRST_CELL + i,
            [4 + (i % 10) as i32 * 25, 2 + (i / 10) as i32 * 25, 24, 24],
            375,
            0,
            0x77777777,
        ));
    }

    for id in [
        CANCEL,
        SEND,
        CLEAR,
        FIRST_PRESET,
        FIRST_PRESET + 1,
        FIRST_PRESET + 2,
        FIRST_PRESET + 3,
        FIRST_PRESET + 4,
    ] {
        if let Some(r) = ui.records.iter().find(|r| {
            r.header.name_id == id
                && r.header.parent_id == PANEL
                && r.control == ControlType::Button
        }) && let Some(mut at) = r.rect()
        {
            at[0] += 1;

            at[1] -= 1;

            out.push((
                id,
                at,
                r.control_id().unwrap_or(-2),
                r.fields.last().copied().unwrap_or(0).max(0) as usize,
                r.fields.get(5).copied().unwrap_or(-1) as u32,
            ));
        }
    }

    out
}

pub fn mouse(scene: &mut FieldScene, event: Mouse, x: i32, y: i32) -> bool {
    if !scene.firecracker.visible {
        return false;
    }

    let Some([px, py, w, h]) = rectangle(&scene.ui) else {
        return false;
    };

    let x = x as f32 / scene.layout.sx - px as f32;

    let y = y as f32 / scene.layout.sy - py as f32;

    let controls = controls(&scene.ui);

    let hit = controls.iter().position(|(_, r, ..)| {
        x >= r[0] as f32 && y >= r[1] as f32 && x < (r[0] + r[2]) as f32 && y < (r[1] + r[3]) as f32
    });

    scene.firecracker.hovered = hit;

    match event {
        Mouse::LeftDown => {
            if let Some(i) = hit {
                scene.firecracker.pressed[i] = true;
            }
        }
        Mouse::LeftUp => {
            for i in 0..controls.len() {
                if hit != Some(i) || !scene.firecracker.enabled[i] {
                    scene.firecracker.pressed[i] = false;
                }
            }

            if let Some(i) = hit.filter(|&i| scene.firecracker.pressed[i])
                && scene.firecracker.click_stamps[i].wrapping_add(500) < scene.time.now_ms
            {
                scene
                    .unconditional_sounds
                    .push(super::audio::UnconditionalSound::new(53));

                let id = controls[i].0;

                if id < FIRST_CELL + 100 {
                    scene.firecracker.selected[i] = !scene.firecracker.selected[i];
                } else if id == CANCEL {
                    scene.firecracker.cancel();
                } else if id == CLEAR {
                    scene.firecracker.selected.fill(false);
                } else if id == SEND {
                    scene.firecracker.visible = false;

                    send(scene);
                } else {
                    scene.firecracker.preset((id - FIRST_PRESET) as usize);
                }

                scene.firecracker.click_stamps[i] = scene.time.now_ms;

                scene.firecracker.pressed[i] = false;
            }
        }
        _ => {}
    }

    x >= 0.0 && y >= 0.0 && x < w as f32 && y < h as f32
}

pub fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    if !scene.firecracker.visible {
        return Vec::new();
    }

    let Some([px, py, w, h]) = rectangle(&scene.ui) else {
        return Vec::new();
    };

    let mut out = Vec::new();

    let mut draw = |at: [i32; 4], set: i32, state: usize, color: u32| {
        let r = scene.layout.control(
            (px + at[0]) as f32,
            (py + at[1]) as f32,
            at[2] as f32,
            at[3] as f32,
        );

        let atlas = usize::try_from(set)
            .ok()
            .and_then(|set| scene.sets.item(set, state));

        let (texture, s) = atlas
            .and_then(|a| {
                u32::try_from(a.texture_index).ok().map(|slot| {
                    (
                        TextureKey::Scene(slot),
                        [a.x as f32, a.y as f32, a.w as f32, a.h as f32],
                    )
                })
            })
            .unwrap_or((TextureKey::White, [0.0, 0.0, 1.0, 1.0]));

        out.push(PlannedQuad {
            mode: S_MODE,
            texture,
            quad: Quad {
                normalized_corners: None,
                clip: None,
                dst_x: r.x,
                dst_y: r.y,
                dst_w: r.w,
                dst_h: r.h,
                src_x: s[0],
                src_y: s[1],
                src_w: s[2],
                src_h: s[3],
                color,
                rotation: 0.0,
            },
            uv: UvRule::TexelCentre,
        });
    };

    let background = scene
        .ui
        .records
        .iter()
        .find(|r| r.header.name_id == PANEL)
        .and_then(|r| r.fields.get(5))
        .copied()
        .unwrap_or(0) as u32;

    draw([0, 0, w, h], -2, 0, background);

    let controls = controls(&scene.ui);

    for (i, (_, at, set, _, color)) in controls.iter().enumerate().rev() {
        draw(*at, *set, scene.firecracker.frame(i), *color);
    }

    let tooltip = scene
        .firecracker
        .hovered
        .and_then(|i| controls.get(i))
        .copied()
        .filter(|(_, _, set, label, _)| *set > 0 && *label > 0);

    for (id, at, set, label, _) in controls {
        if set >= 0 || label == 0 || !scene.captions {
            continue;
        }

        let key = TEXT_BASE + id;

        let value = stext::display_text(&scene.message_strings.translate(&format!("uu_{label}")));

        if scene.text.resolve(key, &value, 0xffffffff)
            && let Some(line) = scene.text.get(key)
        {
            let r = scene.layout.control(
                (px + at[0]) as f32,
                (py + at[1]) as f32,
                at[2] as f32,
                at[3] as f32,
            );

            let origin = (
                r.x + (r.w - (line.rect[2] - line.rect[0]) as f32) * 0.5,
                r.y + (r.h - stext::LINE_BOX * scene.layout.sy) * 0.5,
            );

            out.push(PlannedQuad {
                mode: render::preset::RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: stext::shadow_quad(line, origin),
                uv: UvRule::TexelCentre,
            });

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::SceneText(key),
                quad: stext::quad(line, origin),
                uv: UvRule::TexelCentre,
            });
        }
    }

    if scene.captions
        && let Some((id, at, _, label, _)) = tooltip
    {
        let key = TEXT_BASE + id;

        let value = stext::display_text(&scene.message_strings.translate(&format!("uu_{label}")));

        if scene.text.resolve(key, &value, 0xffffffff)
            && let Some(line) = scene.text.get(key)
        {
            let x = (px + at[0]) as f32 * scene.layout.sx;

            let y = (py + at[1] + at[3] + 2) as f32 * scene.layout.sy;

            let width = (line.rect[2] - line.rect[0]) as f32;

            out.push(PlannedQuad {
                mode: render::preset::RenderMode::Ui2dPoint,
                texture: TextureKey::SceneText(key),
                quad: stext::shadow_quad(
                    line,
                    (x.min(scene.layout.width as f32 - width).max(0.0), y),
                ),
                uv: UvRule::TexelCentre,
            });

            out.push(PlannedQuad {
                mode: S_MODE,
                texture: TextureKey::SceneText(key),
                quad: stext::quad(line, (x.min(scene.layout.width as f32 - width).max(0.0), y)),
                uv: UvRule::TexelCentre,
            });
        }
    }

    out
}
