use super::{field::FieldScene, stext};
use crate::frame::{PlannedQuad, TextureKey};
use formats::scene::scene_ui::ControlType;
use render::{
    preset::RenderMode,
    sprite::{Quad, UvRule},
};

pub(super) fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    let mut out = Vec::new();

    if !scene.arena.visible {
        return out;
    }

    let Some(parent) = scene
        .ui
        .records
        .iter()
        .find(|r| r.header.name_id == 0x16200 && r.control == ControlType::Panel)
    else {
        return out;
    };

    let Some([x, y, w, h]) = parent.rect() else {
        return out;
    };

    let rect = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

    if let Some(set) = parent.control_id().and_then(|id| usize::try_from(id).ok())
        && let Some(item) = scene.sets.item(set, 0)
        && let Ok(slot) = u32::try_from(item.texture_index)
    {
        out.push(PlannedQuad {
            mode: super::field::S_MODE,
            texture: TextureKey::Scene(slot),
            uv: UvRule::TexelCentre,
            quad: Quad {
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
                color: 0xffffffff,
                rotation: 0.0,
            },
        });
    }

    if !scene.captions {
        return out;
    }

    for index in (0..4).rev() {
        let key = 0x16202 + index as u32;

        let Some(record) = scene.ui.records.iter().find(|r| {
            r.header.name_id == key
                && r.header.parent_id == 0x16200
                && r.control == ControlType::Text
        }) else {
            continue;
        };

        let Some([x, y, w, h]) = record.rect() else {
            continue;
        };

        let control = scene.layout.control(x as f32, y as f32, w as f32, h as f32);

        let value = stext::display_text(&scene.arena.scores[index]);

        let colour = record.fields[5] as u32;

        if !scene.text.resolve(key, &value, colour) {
            continue;
        }

        let Some(line) = scene.text.get(key) else {
            continue;
        };

        let width = (line.rect[2] - line.rect[0]) as f32;

        let at = (
            rect.x + control.x + (control.w - width) * 0.5,
            rect.y + control.y + (control.h - stext::LINE_BOX * scene.layout.sy) * 0.5,
        );

        out.push(PlannedQuad {
            mode: RenderMode::SControl2d,
            texture: TextureKey::SceneText(key),
            uv: UvRule::TexelCentre,
            quad: stext::quad(line, at),
        });
    }

    out
}
