use super::objects::{Frame, ObjectScene, ui_mesh};
use super::registries::Registries;
use assets::GameDir;
use formats::mesh::{appearance as item_appearance, glow};
use formats::net::account::{Equipment, Item};
use glam::Vec3;
use render::Matrix4;
use render::mesh::{MeshPass, normal_matrix};
use render::world::{Fog, FrameUniforms, Lighting, Material, PointLights};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

mod actor;
mod appearance;

const CACHE_LIMIT: usize = 32;

pub struct Preview {
    pub resources: ObjectScene,
    pub identity: u64,
    pub frame: Frame,
    pub uniforms: FrameUniforms,
    pub range: Range<usize>,
    loaded: BTreeMap<(u32, i32, i32, u16), bool>,
    actors: BTreeMap<actor::Appearance, Option<actor::Model>>,
    mounts: Option<Vec<formats::data::mount::MountVisual>>,
    equipment: Option<Equipment>,
}

impl Preview {
    pub fn new(registries: Arc<Registries>) -> Self {
        Self {
            resources: ObjectScene::empty(registries),
            identity: next_identity(),
            frame: Frame {
                objects: Vec::new(),
                draws: Vec::new(),
                light_shafts: Vec::new(),
            },
            uniforms: uniforms((1, 1)),
            range: 0..0,
            loaded: BTreeMap::new(),
            actors: BTreeMap::new(),
            mounts: None,
            equipment: None,
        }
    }

    pub fn prepare(
        &mut self,
        game: &GameDir,
        item: Item,
        rect: [f32; 4],
        viewport: (u32, u32),
        now_ms: u32,
    ) -> bool {
        self.frame.objects.clear();
        self.frame.draws.clear();
        self.range = 0..0;

        if viewport.0 == 0 || viewport.1 == 0 || !valid_rect(rect) {
            return false;
        }

        let bounds = if appearance::handles(item, &self.resources.registries.items) {
            self.prepare_actor(game, item, now_ms)
        } else {
            self.prepare_static(game, item, viewport, now_ms)
        };
        let angle = (now_ms % 12_000) as f32 * std::f32::consts::TAU / 12_000.0;
        let Some(world) = bounds.and_then(|bounds| placement(bounds, rect, angle)) else {
            self.frame.objects.clear();
            self.frame.draws.clear();
            self.range = 0..0;
            return false;
        };

        let normals = normal_matrix(&world).shader_constants();

        for object in &mut self.frame.objects {
            object.world = world.shader_constants();
            object.normal_matrix = normals;
        }

        self.uniforms = uniforms(viewport);

        !self.range.is_empty()
    }

    pub fn set_equipment(&mut self, equipment: Equipment) {
        self.equipment = Some(equipment);
    }

    fn reserve_model(&mut self) {
        if self.loaded.len() + self.actors.len() >= CACHE_LIMIT {
            self.resources = ObjectScene::empty(Arc::clone(&self.resources.registries));
            self.loaded.clear();
            self.actors.clear();
            self.identity = next_identity();
        }
    }

    fn prepare_actor(
        &mut self,
        game: &GameDir,
        item: Item,
        now_ms: u32,
    ) -> Option<render::cull::Aabb> {
        let mounts = self.mounts.get_or_insert_with(|| {
            game.read(b"MountDataV.bin")
                .ok()
                .and_then(|bytes| formats::data::mount::parse_visual(&bytes).ok())
                .unwrap_or_default()
        });
        let target = appearance::select(
            item,
            self.equipment.as_ref(),
            &self.resources.registries.items,
            mounts,
        )?;

        if !self.actors.contains_key(&target) {
            self.reserve_model();
            let model = actor::load(game, &mut self.resources, target);
            self.actors.insert(target, model);
        }

        let model = self.actors.get(&target)?.as_ref()?;
        self.range = model.append(&mut self.frame, now_ms);
        Some(model.bounds)
    }

    fn prepare_static(
        &mut self,
        game: &GameDir,
        item: Item,
        viewport: (u32, u32),
        now_ms: u32,
    ) -> Option<render::cull::Aabb> {
        let Some(recipe) = recipe(item, &self.resources.registries.items) else {
            return None;
        };

        let key = (
            recipe.mesh,
            recipe.texture_bias,
            recipe.refine,
            recipe.style,
        );

        if !self.loaded.contains_key(&key) {
            self.reserve_model();

            let ready = self.resources.load_ui_mesh(game, recipe).is_ok();
            self.loaded.insert(key, ready);
        }

        if !self.loaded.get(&key).copied().unwrap_or(false) {
            return None;
        }

        let Some(mesh) = self
            .resources
            .by_id_meshes
            .get(&recipe.mesh)
            .and_then(|&index| self.resources.meshes.get(index))
        else {
            return None;
        };

        let bounds = mesh.bounds;

        self.range = self.resources.append_ui_mesh(
            &mut self.frame,
            recipe,
            viewport,
            now_ms,
            ui_mesh::DrawState {
                pass: MeshPass::Opaque,
                lighting: true,
            },
        );

        for draw in &mut self.frame.draws {
            if self
                .resources
                .textures
                .get(draw.texture)
                .is_some_and(|texture| {
                    use formats::texture::texture_list::FormatFlag;

                    matches!(
                        texture.flag,
                        FormatFlag::Alpha | FormatFlag::AlphaLower | FormatFlag::ColorKey
                    )
                })
            {
                let mut object = self.frame.objects[draw.object as usize];
                object.alpha[0] = render::mesh::ALPHAREF_MESH_ALPHA_32BPP as f32 / 255.0;
                draw.object = self.frame.objects.len() as u32;
                self.frame.objects.push(object);
                draw.pass = MeshPass::Cutout;
                draw.cull = render::mesh::Cull::Ccw;
            }
        }

        Some(bounds)
    }
}

fn next_identity() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);

    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn recipe(item: Item, items: &formats::data::item_list::ItemList) -> Option<ui_mesh::Recipe> {
    if item.is_empty() {
        return None;
    }

    let entry = items.get(usize::from(item.index))?;
    let visual = if item.appearance != 0 {
        item.appearance
    } else {
        item.index
    };
    let visual_entry = items.get(usize::from(visual))?;
    let mesh = u32::try_from(item_appearance::ground_item_mesh(i32::from(visual), items)).ok()?;

    if mesh == 0 {
        return None;
    }

    let mut refine = item_appearance::refine_level(&item, items).min(12) as i32;
    let tint = glow::tint(&item);
    let mut style = if (116..=125).contains(&tint) {
        i16::from(tint)
    } else {
        entry.tier
    };

    if (entry.grade == 51 || style != 0) && style < 5 && refine >= 10 {
        style = i16::from(item_appearance::refine_rank(&item, items)) + 4;
    }

    if super::drops::FLAG_MESHES.contains(&visual_entry.mesh) {
        refine = 0;
        style = 0;
    }

    Some(ui_mesh::Recipe {
        mesh,
        position: (0, 0),
        angle: 0.0,
        scale: 1.0,
        color: 0xFFFF_FFFF,
        refine,
        texture_bias: super::drops::tex_bias(visual as i16, items),
        style: u16::try_from(style).unwrap_or(0),
    })
}

fn valid_rect([x, y, width, height]: [f32; 4]) -> bool {
    [x, y, width, height].iter().all(|n| n.is_finite()) && width > 0.0 && height > 0.0
}

fn placement(bounds: render::cull::Aabb, rect: [f32; 4], angle: f32) -> Option<Matrix4> {
    let min = Vec3::from(bounds.min);
    let max = Vec3::from(bounds.max);
    let extent = max - min;

    if !valid_rect(rect)
        || !min.is_finite()
        || !max.is_finite()
        || extent.min_element() < 0.0
        || !angle.is_finite()
    {
        return None;
    }

    let radius = extent.length() * 0.5;

    if !radius.is_finite() || radius <= f32::EPSILON {
        return None;
    }

    let [x, y, width, height] = rect;
    let center = (min + max) * 0.5;
    let scale = width.min(height) * 0.45 / radius;
    let rotation = Matrix4::rotation_yaw_pitch_roll(angle, -std::f32::consts::FRAC_PI_2, 0.0);
    let world = Matrix4::multiply(
        &Matrix4::translation(-center.x, -center.y, -center.z),
        &rotation,
    );
    let world = Matrix4::multiply(&world, &Matrix4::scaling(scale, scale, scale));

    Some(Matrix4::multiply(
        &world,
        &Matrix4::translation(x + width * 0.5, -y - height * 0.5, 0.0),
    ))
}

fn uniforms(viewport: (u32, u32)) -> FrameUniforms {
    let width = viewport.0.max(1) as f32;
    let height = viewport.1.max(1) as f32;
    let projection = Matrix4([
        2.0 / width,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / height,
        0.0,
        0.0,
        0.0,
        0.0,
        0.5 / width.max(height),
        0.0,
        -1.0,
        1.0,
        0.5,
        1.0,
    ]);

    FrameUniforms::new(
        Matrix4::IDENTITY,
        Matrix4::IDENTITY,
        Matrix4::IDENTITY,
        projection,
        &Lighting {
            light0: [0.8, 0.8, 0.8, 1.0],
            light1: [0.35, 0.35, 0.35, 1.0],
            material: Material::STATIC_OBJECT,
            global_ambient: 0xFF60_6060,
            point_lights: PointLights::default(),
        },
        &Fog {
            enabled: false,
            color: 0,
            start: 0.0,
            end: 1.0,
        },
    )
}

#[cfg(test)]
mod tests;
