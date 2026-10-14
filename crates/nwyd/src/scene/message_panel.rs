use super::field::{FieldScene, S_MODE};
use super::{
    kickout::MessagePanel,
    login_static::{self, Layout, LoadedTexture},
    stext::SceneText,
};
use crate::frame::SceneRenderer;
use crate::frame::{PlannedQuad, TextureKey};
use formats::texture::{
    texture_list,
    texture_set_list::{self, TextureSetList},
};
use render::device::Gpu;
use render::sprite::{Quad, UvRule};
use std::collections::BTreeMap;

#[derive(Debug)]

pub struct Overlay {
    pub state: MessagePanel,
    sets: TextureSetList,
    textures: BTreeMap<u32, LoadedTexture>,
    text: SceneText,
    sounds: Vec<i64>,
}

impl Overlay {
    pub fn relocalize(&mut self, _game: &assets::GameDir, _strings: &ui::Strings) {
        let keys: Vec<_> = self.text.iter().map(|(key, _)| *key).collect();

        for key in keys {
            self.text.forget(key);
        }
    }

    #[cfg(test)]

    pub(crate) fn for_test() -> Self {
        Self {
            state: MessagePanel::new(),
            sets: Default::default(),
            textures: Default::default(),
            text: SceneText::new(&Default::default(), 800),
            sounds: Vec::new(),
        }
    }

    pub(super) fn resize(&mut self, chains: &crate::fonts::Chains, width: u32) {
        self.text = SceneText::new(chains, width);
    }

    pub fn build(game: &assets::GameDir, width: u32) -> anyhow::Result<Self> {
        let sets = texture_set_list::parse(&game.read(login_static::TEXTURE_SET_LIST)?)?;

        let list = texture_list::parse(&game.read(login_static::TEXTURE_LIST)?)?;

        let mut textures = BTreeMap::new();

        for set in SETS {
            if let Some(item) = sets.item(set, 0)
                && let Ok(slot) = u32::try_from(item.texture_index)
                && !textures.contains_key(&slot)
                && let Some(texture) = login_static::load_texture(game, &list, slot)?
            {
                textures.insert(slot, texture);
            }
        }

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        Ok(Self {
            state: MessagePanel::new(),
            sets,
            textures,
            text: SceneText::new(&chains, width),
            sounds: Vec::new(),
        })
    }

    pub fn post(&mut self, record: usize, life_ms: u32, now_ms: u32, sound: bool) {
        self.state.post(record, life_ms, now_ms);

        if sound {
            self.sounds.push(super::notice::SOUND_SLOT);
        }
    }

    pub fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    pub fn upload(&self, gpu: &Gpu, renderer: &mut SceneRenderer) {
        for (slot, loaded) in &self.textures {
            renderer.upload_once(
                gpu,
                TextureKey::Scene(*slot),
                &loaded.texture,
                loaded.alpha,
                "shared message strip",
            );
        }
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
        draws(
            &self.state,
            layout,
            &self.sets,
            &self.textures,
            &mut self.text,
            strings,
            captions,
        )
    }
}

pub const SETS: [usize; 3] = [502, 503, 504];

pub const TEXT_KEY: u32 = 0x0400_0000;

pub fn consumes_mouse(
    panel: &MessagePanel,
    layout: &Layout,
    event: ui::input::Mouse,
    x: i32,
    y: i32,
) -> bool {
    use ui::input::Mouse;

    if !panel.visible
        || !matches!(
            event,
            Mouse::Move | Mouse::LeftDown | Mouse::LeftUp | Mouse::RightDown
        )
    {
        return false;
    }

    let parent = layout.control(160.0, 80.0, 480.0, 24.0);

    [-11.0, 480.0].into_iter().any(|local_x| {
        let child = layout.control(local_x, 0.0, 11.0, 24.0);

        let (cx, cy) = (parent.x.trunc() + child.x, parent.y.trunc() + child.y);

        x as f32 >= cx && y as f32 >= cy && (x as f32) < cx + child.w && (y as f32) < cy + child.h
    })
}

pub fn post(scene: &mut FieldScene, record: usize, life_ms: u32, now_ms: u32) {
    scene.message_panel.post(record, life_ms, now_ms);

    scene.world.play_sound(super::notice::SOUND_SLOT);
}

pub const PK_ENTRIES: [[i32; 4]; 8] = [
    [2255, 1535, 2263, 1538],
    [150, 3788, 156, 3793],
    [150, 3711, 156, 3772],
    [154, 3773, 157, 3780],
    [1075, 1708, 1078, 1713],
    [1171, 4076, 1177, 4079],
    [2362, 4041, 2369, 4044],
    [2472, 1672, 2483, 1675],
];

pub fn pk_entry_contains(x: f32, z: f32) -> bool {
    let (x, z) = (x as i32, z as i32);

    PK_ENTRIES
        .iter()
        .any(|r| x >= r[0] && x < r[2] && z >= r[1] && z < r[3])
}

pub fn tick_pk_entry(scene: &mut FieldScene, now_ms: u32) {
    let Some(player) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    let inside = pk_entry_contains(player.position.x, player.position.z);

    if inside && !scene.pk_warning_inside {
        post(scene, 42, 2000, now_ms);
    }

    scene.pk_warning_inside = inside;
}

pub fn countdown(scene: &mut FieldScene, record: usize, seconds: u32, now_ms: u32) {
    let template = scene
        .message_strings
        .strdef
        .as_ref()
        .map_or("", |s| s.get(record));

    let text = template
        .replacen("%d", &seconds.to_string(), 1)
        .replace("%%", "%");

    scene
        .message_panel
        .post_formatted(record, text, 2000, now_ms);

    scene.world.play_sound(super::notice::SOUND_SLOT);
}

pub fn frame_draws(scene: &mut FieldScene) -> Vec<PlannedQuad> {
    draws(
        &scene.message_panel,
        &scene.layout,
        &scene.sets,
        &scene.chrome,
        &mut scene.text,
        &scene.message_strings,
        scene.captions,
    )
}

pub(crate) fn resolved_text(panel: &MessagePanel, strings: &ui::Strings) -> String {
    panel.text.clone().unwrap_or_else(|| {
        panel
            .record
            .and_then(|r| strings.strdef.as_ref().map(|s| s.get(r).to_string()))
            .unwrap_or_default()
    })
}

fn draws(
    panel: &MessagePanel,
    layout: &Layout,
    sets: &TextureSetList,
    chrome: &BTreeMap<u32, LoadedTexture>,
    cache: &mut SceneText,
    strings: &ui::Strings,
    captions: bool,
) -> Vec<PlannedQuad> {
    if !panel.visible {
        cache.forget(TEXT_KEY);

        return Vec::new();
    }

    let mut out = Vec::with_capacity(5);

    for (set, x, w) in [(502, 160.0, 480.0), (503, 149.0, 11.0), (504, 640.0, 11.0)] {
        let Some(item) = sets.item(set, 0) else {
            continue;
        };

        let Ok(slot) = u32::try_from(item.texture_index) else {
            continue;
        };

        if !chrome.contains_key(&slot) {
            continue;
        }

        let rect = layout.control(x, 80.0, w, 24.0);

        out.push(PlannedQuad {
            mode: S_MODE,
            texture: TextureKey::Scene(slot),
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
                color: 0xAAFF_FFFF,
                rotation: 0.0,
            },
            uv: UvRule::TexelCentre,
        });
    }

    if !captions {
        return out;
    }

    let text = super::stext::display_text(&resolved_text(panel, strings));

    let rect = layout.control(190.0, 81.0, 428.0, 22.0);

    if cache.resolve_with_width_limit(TEXT_KEY, &text, 0xFFFF_FFFF, rect.w.ceil() as u32)
        && let Some(line) = cache.get(TEXT_KEY)
    {
        let at = (
            rect.x + (rect.w - (line.rect[2] - line.rect[0]) as f32) * 0.5,
            rect.y + (rect.h - super::stext::LINE_BOX * layout.sy) * 0.5,
        );

        out.push(PlannedQuad {
            mode: render::preset::RenderMode::Ui2dPoint,
            texture: TextureKey::SceneText(TEXT_KEY),
            quad: super::stext::shadow_quad(line, at),
            uv: UvRule::TexelCentre,
        });

        out.push(PlannedQuad {
            mode: S_MODE,
            texture: TextureKey::SceneText(TEXT_KEY),
            quad: super::stext::quad(line, at),
            uv: UvRule::TexelCentre,
        });
    }

    out
}
