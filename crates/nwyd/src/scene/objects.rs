use super::registries::Registries;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::mesh::mesh_list::{self, MeshList};
use formats::mesh::msa::{self, StaticMesh};
use formats::texture::Texture;
use formats::texture::texture_list::{FormatFlag, TextureList};
use formats::world::objects::{self, ObjRecord, ObjectClass};
use glam::Vec3;
use render::camera::{self, Matrix4};
use render::device::Gpu;
use render::mesh::{
    Cull, DrawParams, EFFECT_ADDITIVE, MeshArena, MeshDraw, MeshPass, MeshRenderer, MeshTexture,
    ObjectUniforms, effect_states,
};
use render::texture::AlphaPolicy;
use render::world::Material;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MESH_LIST: &[u8] = br"mesh\MeshList.txt";

pub const MESH_TEXTURE_LIST: &[u8] = br"mesh\MeshTextureList.bin";

pub const EFFECT_TEXTURE_LIST: &[u8] = br"Effect\EffectTextureList.bin";

pub const SKY_MESH_ID: u32 = 1;

pub const SKY_TEXTURE_BASE: i32 = 0x43;

pub const SKY_SCALE: f32 = 0.5;

pub const SKY_SCALE_SPECIAL_ZONE: f32 = 0.9;

pub const SKY_HEIGHT: f32 = -5.0;

pub const SKY_YAW: f32 = camera::PI;

pub const SKY_VERTEX_COLOR: u32 = 0x0080_8080;

pub const DEVICE_WORLD: Matrix4 = Matrix4::IDENTITY;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateLitDraw {
    pub mesh_id: i32,
    pub world: Matrix4,
    pub material: Material,
    pub fog: bool,
}

impl RotateLitDraw {
    fn states(self, vertex_color: bool) -> (MeshPass, Cull, DrawParams) {
        (
            MeshPass::Blended,
            Cull::Ccw,
            DrawParams {
                material: self.material,
                lighting: true,
                fog: self.fog,
                color_op: render::preset::ColorOp::Modulate,
                vertex_color,
                force_opaque: false,
                stage1: false,
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct BoundTexture {
    pub pending: bool,
    pub effect_table: bool,
    pub slot: usize,
    pub path: String,
    pub flag: FormatFlag,
    pub texture: Arc<Texture>,
    pub mip_policy_override: Option<render::texture::MipPolicy>,
    pub prepared: Option<Arc<render::texture::PreparedMaterial>>,
}

impl BoundTexture {
    pub fn prepared_material(&self, skin: bool) -> Option<&render::texture::PreparedMaterial> {
        self.prepared
            .as_deref()
            .filter(|material| !self.pending && material.policy() == self.mip_policy(skin))
    }

    pub fn prepare_material(&mut self, skin: bool) {
        if !self.pending && self.prepared_material(skin).is_none() {
            self.prepared = Some(Arc::new(render::texture::PreparedMaterial::from_shared(
                Arc::clone(&self.texture),
                self.mip_policy(skin),
            )));
        }
    }

    pub fn alpha_policy(&self) -> AlphaPolicy {
        AlphaPolicy::for_flag(self.flag)
    }

    pub fn mip_policy(&self, skin: bool) -> render::texture::MipPolicy {
        use render::texture::MipPolicy;

        if let Some(policy) = self.mip_policy_override {
            policy
        } else if self.alpha_policy() == AlphaPolicy::Opaque {
            MipPolicy::Opaque
        } else if !skin && !self.effect_table {
            MipPolicy::Cutout(render::mesh::ALPHAREF_MESH_ALPHA_32BPP as u8)
        } else {
            MipPolicy::Blend
        }
    }
}

#[derive(Debug)]
pub struct MeshResource {
    pub id: u32,
    pub path: String,
    pub mesh: StaticMesh,
    pub bounds: render::cull::Aabb,
    pub alloc: usize,
    pub subset_slots: Vec<Option<usize>>,
    pub subset_textures: Vec<Option<usize>>,
    pub subset_zero_pass: MeshPass,
    pub subset_passes: Vec<MeshPass>,
    pub vertex_color: bool,
}

impl MeshResource {
    pub fn mixed_subsets(&self) -> usize {
        self.subset_passes
            .iter()
            .filter(|&&p| p != self.subset_zero_pass)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    pub kind: u32,
    pub mesh: usize,
    pub position: Vec3,
    pub yaw: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub position: Vec3,
    pub world: Matrix4,
    pub bounds: render::cull::Aabb,
    pub fade: super::scenery_fade::State,
}

impl Instance {
    pub fn world(&self) -> Matrix4 {
        if self.kind == 0x736 {
            let mut mesh = super::effect_mesh::EffectMesh::new(
                1846,
                0xffffffff,
                self.yaw - std::f32::consts::FRAC_PI_2,
                0,
                0,
            );

            mesh.position = self.position;

            return mesh.world();
        }

        world_at(self.position, self.yaw, 0.0, 0.0)
    }
}

pub fn reflective_scenery(kind: u32) -> bool {
    matches!(kind, 1934 | 1976 | 1977)
}

fn static_mesh_pass(flag: FormatFlag) -> MeshPass {
    match flag {
        FormatFlag::Alpha | FormatFlag::AlphaLower | FormatFlag::ColorKey => MeshPass::Cutout,
        _ => MeshPass::Opaque,
    }
}

fn static_position(kind: u32, mut position: Vec3) -> Vec3 {
    if matches!(
        (kind, position.x as i32, position.z as i32),
        (443, 2540, 2086) | (454, 2542, 2090) | (454, 2540, 2082) | (449, 2540, 2094)
    ) {
        position.y = 0.0;
    }

    position
}

pub fn world_at(position: Vec3, yaw: f32, pitch: f32, roll: f32) -> Matrix4 {
    let r = Matrix4::rotation_yaw_pitch_roll(yaw, pitch - camera::PI / 2.0, roll);

    let m = Matrix4::multiply(&DEVICE_WORLD, &r);

    let m = Matrix4::multiply(&m, &Matrix4::scaling(1.0, 1.0, 1.0));

    Matrix4::multiply(
        &m,
        &Matrix4::translation(position.x, position.y, position.z),
    )
}

pub fn world_at_quaternion(position: Vec3, rotation: [f32; 4], scale: (f32, f32)) -> Matrix4 {
    let r = Matrix4::rotation_quaternion(rotation);

    let m = Matrix4::multiply(&r, &DEVICE_WORLD);

    let m = Matrix4::multiply(&m, &Matrix4::scaling(scale.0, scale.1, scale.0));

    Matrix4::multiply(
        &m,
        &Matrix4::translation(position.x, position.y, position.z),
    )
}

pub mod ui_mesh;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HousePart {
    pub kind: u32,
    pub mesh: usize,
    pub position: Vec3,
    pub yaw: f32,
    pub part: super::house::Part,
}

impl HousePart {
    pub fn world(&self, swing: f32) -> Matrix4 {
        let (yaw, pitch, roll) = self.part.pose(self.yaw, swing);

        world_at(
            self.position + self.part.offset_at(self.yaw),
            yaw,
            pitch,
            roll,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    pub mesh: usize,
    pub scale: f32,
    pub yaw: f32,
    pub height: f32,
    pub texture_slot: i32,
}

impl Sky {
    pub fn world(&self, eye: Vec3) -> Matrix4 {
        let r = Matrix4::rotation_yaw_pitch_roll(self.yaw, -camera::PI / 2.0, 0.0);

        let s = Matrix4::scaling(self.scale, self.scale * 0.5, self.scale);

        let t = Matrix4::translation(eye.x, self.height + 1.0, eye.z);

        let m = Matrix4::multiply(&DEVICE_WORLD, &r);

        let m = Matrix4::multiply(&m, &s);

        Matrix4::multiply(&m, &t)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassCounts(pub BTreeMap<&'static str, usize>);

impl ClassCounts {
    pub fn get(&self, class: &str) -> usize {
        self.0.get(class).copied().unwrap_or(0)
    }
}

fn class_name(class: ObjectClass) -> &'static str {
    match class {
        ObjectClass::StaticMesh => "static",
        ObjectClass::Ship => "ship",
        ObjectClass::Suppressed => "suppressed",
        ObjectClass::Grass => "grass",
        ObjectClass::Tree => "tree",
        ObjectClass::Butterfly => "butterfly",
        ObjectClass::Fish => "fish",
        ObjectClass::Prop => "prop",
        ObjectClass::Sea => "sea",
        ObjectClass::Particle => "particle",
        ObjectClass::Light => "light",
        ObjectClass::Glow => "glow",
        ObjectClass::Dust => "dust",
        ObjectClass::Drop => "drop",
        ObjectClass::House => "house",
        ObjectClass::Skipped => "skipped",
    }
}

#[derive(Debug)]
pub struct ObjectScene {
    pub records: Vec<ObjRecord>,
    pub neighbour_records: Vec<ObjRecord>,
    pub classes: ClassCounts,
    pub meshes: Vec<MeshResource>,
    pub textures: Vec<BoundTexture>,
    pub instances: Vec<Instance>,
    pub placements: Vec<Placement>,
    pub house_parts: Vec<HousePart>,
    pub house_emitters: Vec<super::house::Source>,
    pub roof_hidden: bool,
    pub house_target: Option<Vec3>,
    pub environment: i32,
    pub by_id_meshes: BTreeMap<u32, usize>,
    pub missing_by_id: Vec<u32>,
    pub arena: MeshArena,
    pub sky: Option<Sky>,
    pub sky_horizon: Option<super::sky::Horizon>,
    pub missing_meshes: Vec<u32>,
    pub missing_textures: Vec<String>,
    pub registries: Arc<Registries>,
    held_loader: Option<held_loading::Loader>,
    texture_loader: Option<super::actors::texture_loading::Loader>,
    pub texture_updates: Vec<usize>,
}

mod held_loading;
pub(crate) mod resources;

#[cfg(test)]
mod alpha_tests;

pub fn build(
    game: &GameDir,
    zone_name: &str,
    zone: (u8, u8),
    day: u32,
    registries: Arc<Registries>,
) -> Result<ObjectScene> {
    let reg = Arc::clone(&registries);

    let (list, mesh_textures, effect_textures) = (&reg.list, &reg.textures, &reg.effect_textures);

    let dat_path = format!(r"Env\{zone_name}.dat");

    let records = objects::parse(
        &game
            .read(dat_path.as_bytes())
            .with_context(|| format!("reading {dat_path}"))?,
    )
    .with_context(|| format!("parsing {dat_path}"))?;

    let mut classes = ClassCounts::default();

    for r in &records {
        *classes.0.entry(class_name(r.class())).or_default() += 1;
    }

    let mut scene = ObjectScene::empty(registries);

    scene.records = records;

    scene.classes = classes;

    let mut by_kind: BTreeMap<u32, Option<usize>> = BTreeMap::new();

    let records = std::mem::take(&mut scene.records);

    for record in &records {
        if !record.class().uses_mesh_list_kind() {
            continue;
        }

        let resolved = *by_kind.entry(record.kind).or_insert_with(|| {
            scene
                .load_mesh(game, list, mesh_textures, None, record.kind, true)
                .ok()
                .flatten()
        });

        let Some(mesh) = resolved else {
            continue;
        };

        let [x, y, z] = record.world_position(u32::from(zone.0), u32::from(zone.1));

        let position = Vec3::new(x, y, z);

        let instance = Instance {
            kind: record.kind,
            mesh,
            position,
            yaw: record.yaw,
        };

        scene.instances.push(instance);

        scene.placements.push(scene.placement_of(&instance));

        if record.class() == ObjectClass::House {
            scene.place_house(
                Some(game),
                list,
                mesh_textures,
                record.kind,
                position,
                record.yaw,
            );
        }
    }

    scene.records = records;

    scene.load_placed_effect_textures(game);

    if scene.instances.iter().any(|i| reflective_scenery(i.kind)) {
        scene.load_effect_textures(game, &[0, 67, 68, 69, 70]);
    }

    scene.missing_meshes = by_kind
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(&k, _)| k)
        .collect();

    if let Ok(Some(mesh)) = scene.load_mesh(game, list, mesh_textures, None, SKY_MESH_ID, false) {
        for slot in 67..=70 {
            scene.bind_slot(game, effect_textures, true, slot);
        }

        let slot = SKY_TEXTURE_BASE + (day & 3) as i32;

        let texture = scene.bind_slot(game, effect_textures, true, slot as usize);

        let subsets = scene.meshes[mesh].mesh.subsets.len();

        scene.meshes[mesh].subset_textures = vec![texture; subsets];

        scene.meshes[mesh].subset_zero_pass = MeshPass::Sky;

        scene.meshes[mesh].subset_passes = vec![MeshPass::Sky; subsets];

        let alloc = scene.meshes[mesh].alloc;

        scene.arena.set_diffuse(alloc, SKY_VERTEX_COLOR);

        scene.sky = Some(Sky {
            mesh,
            scale: if crate::scene::world::is_special_zone(zone.0, zone.1) {
                SKY_SCALE_SPECIAL_ZONE
            } else {
                SKY_SCALE
            },
            yaw: SKY_YAW,
            height: SKY_HEIGHT,
            texture_slot: slot,
        });

        let sky = scene.sky.unwrap();
        let textures = std::array::from_fn(|row| {
            scene
                .textures
                .iter()
                .find(|texture| {
                    texture.effect_table && texture.slot == 67 + row && !texture.pending
                })
                .map(|texture| texture.texture.as_ref())
        });

        scene.sky_horizon = Some(super::sky::Horizon::new(
            &scene.meshes[mesh].mesh,
            &sky.world(Vec3::ZERO),
            textures,
        ));
    }

    Ok(scene)
}

impl ObjectScene {
    pub fn apply_sky(&self, frame: &mut Frame, sky: super::sky::Sky, now_ms: u32, day_tail: bool) {
        let reflection = self
            .textures
            .iter()
            .position(|t| t.effect_table && t.slot == sky.destination_texture);

        for draw in &mut frame.draws {
            if frame.objects[draw.object as usize].refine[3] > 0.5 {
                draw.stage1 = reflection;

                if reflection.is_none() {
                    frame.objects[draw.object as usize].alpha[2] = 0.0;
                }
            }
        }

        let texture = |row: usize| {
            self.textures
                .iter()
                .position(|texture| texture.effect_table && texture.slot == 67 + row)
        };

        let Some(base) = texture(sky.from) else {
            return;
        };

        let destination = (day_tail && sky.fading(now_ms))
            .then(|| texture(sky.to))
            .flatten();

        for draw in frame
            .draws
            .iter_mut()
            .filter(|draw| draw.pass == MeshPass::Sky)
        {
            draw.texture = base;

            draw.stage1 = destination;

            if destination.is_some() {
                frame.objects[draw.object as usize].set_sky_blend(
                    now_ms.wrapping_sub(sky.stamp) as f32 / super::sky::FADE_MS as f32,
                );
            }
        }
    }

    pub fn load_neighbour(
        &mut self,
        game: &GameDir,
        zone_name: &str,
        zone: (u8, u8),
    ) -> Result<usize> {
        let first = self.neighbour_records.len();

        self.load_neighbour_at(game, zone_name, zone)?;

        Ok(first)
    }

    fn load_neighbour_at(
        &mut self,
        game: &GameDir,
        zone_name: &str,
        zone: (u8, u8),
    ) -> Result<usize> {
        let dat_path = format!(r"Env\{zone_name}.dat");

        let records = objects::parse(
            &game
                .read(dat_path.as_bytes())
                .with_context(|| format!("reading {dat_path}"))?,
        )
        .with_context(|| format!("parsing {dat_path}"))?;

        self.append_neighbour_records(game, records, zone)
    }

    pub(super) fn append_neighbour_records(
        &mut self,
        game: &GameDir,
        records: Vec<ObjRecord>,
        zone: (u8, u8),
    ) -> Result<usize> {
        self.append_neighbour_resources(game, records, zone, true)
    }

    pub(super) fn append_neighbour_resources(
        &mut self,
        game: &GameDir,
        records: Vec<ObjRecord>,
        zone: (u8, u8),
        allow_load: bool,
    ) -> Result<usize> {
        let reg = Arc::clone(&self.registries);

        let (list, mesh_textures) = (&reg.list, &reg.textures);

        for r in &records {
            *self.classes.0.entry(class_name(r.class())).or_default() += 1;
        }

        let mut by_kind: BTreeMap<u32, Option<usize>> = self
            .meshes
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id, Some(i)))
            .collect();

        let mut added = 0;

        for record in &records {
            if !record.class().uses_mesh_list_kind() {
                continue;
            }

            let resolved = *by_kind.entry(record.kind).or_insert_with(|| {
                if !allow_load {
                    return None;
                }

                self.load_mesh(game, list, mesh_textures, None, record.kind, true)
                    .ok()
                    .flatten()
            });

            let Some(mesh) = resolved else {
                if !self.missing_meshes.contains(&record.kind) {
                    self.missing_meshes.push(record.kind);
                }

                continue;
            };

            let [x, y, z] = record.world_position(u32::from(zone.0), u32::from(zone.1));

            let position = Vec3::new(x, y, z);

            let instance = Instance {
                kind: record.kind,
                mesh,
                position,
                yaw: record.yaw,
            };

            self.instances.push(instance);

            self.placements.push(self.placement_of(&instance));

            if record.class() == ObjectClass::House {
                self.place_house(
                    allow_load.then_some(game),
                    list,
                    mesh_textures,
                    record.kind,
                    position,
                    record.yaw,
                );
            }

            added += 1;
        }

        if allow_load && records.iter().any(|r| reflective_scenery(r.kind)) {
            self.load_effect_textures(game, &[0, 67, 68, 69, 70]);
        }

        if allow_load && records.iter().any(|r| r.kind == 0x736) {
            self.load_placed_effect_textures(game);
        }

        self.neighbour_records.extend(records);

        Ok(added)
    }

    pub fn mesh_max_z(&self, id: u32) -> Option<f32> {
        let bounds = &self.meshes[*self.by_id_meshes.get(&id)?].bounds;

        Some(bounds.max[2]).filter(|z| z.is_finite())
    }

    pub fn uses_gate_texture_overlay(&self, id: u32) -> bool {
        self.by_id_meshes
            .get(&id)
            .and_then(|i| self.meshes[*i].subset_textures.first())
            .and_then(|i| *i)
            .and_then(|i| self.textures.get(i))
            .is_some_and(|t| t.flag == FormatFlag::None_)
    }

    pub fn load_by_id(&mut self, game: &GameDir, ids: &[u32]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let reg = Arc::clone(&self.registries);

        let (list, textures, effect_textures) = (&reg.list, &reg.textures, &reg.effect_textures);

        for &id in ids {
            if self.by_id_meshes.contains_key(&id) {
                continue;
            }

            match self.load_mesh(game, list, textures, Some(effect_textures), id, true)? {
                Some(mesh) => {
                    self.by_id_meshes.insert(id, mesh);
                }
                None => self.missing_by_id.push(id),
            }
        }

        Ok(())
    }

    pub fn load_held(
        &mut self,
        game: &GameDir,
        actors: &mut super::actors::ActorScene,
    ) -> Result<()> {
        self.load_held_slice(game, &actors.actors)?;

        actors.siege_meshes = self.resident_siege_meshes();

        Ok(())
    }

    pub(crate) fn load_held_actor(
        &mut self,
        game: &GameDir,
        actors: &mut super::actors::ActorScene,
        index: usize,
    ) -> Result<()> {
        if let Some(loader) = &mut self.held_loader {
            loader.invalidate();

            actors.siege_meshes = self.resident_siege_meshes();

            return Ok(());
        }

        self.load_held_slice(game, &actors.actors[index..=index])?;

        actors.siege_meshes = self.resident_siege_meshes();

        Ok(())
    }

    fn resident_siege_meshes(&self) -> [bool; 2] {
        [
            self.by_id_meshes.contains_key(&2888),
            self.by_id_meshes.contains_key(&2889),
        ]
    }

    fn load_held_slice(&mut self, game: &GameDir, actors: &[super::actors::Actor]) -> Result<()> {
        let mut ids: Vec<_> = actors
            .iter()
            .flat_map(|actor| {
                actor
                    .held
                    .iter()
                    .map(|hand| hand.mesh_id)
                    .chain(super::siege_mesh::mesh(actor.parts[7].mesh))
            })
            .collect();

        ids.sort_unstable();

        ids.dedup();

        self.load_by_id(game, &ids)?;

        let mut slots: std::collections::BTreeSet<_> = actors
            .iter()
            .flat_map(|a| &a.held)
            .flat_map(|h| {
                h.glow
                    .into_iter()
                    .flat_map(|slot| [slot, super::weapon_glow::after_draw(h.mesh_id, slot)])
            })
            .collect();

        for hand in actors.iter().flat_map(|a| &a.held) {
            if let Some(glow) = hand
                .ordinary_glow
                .and_then(|g| self.ordinary_held_glow(hand.mesh_id, g))
            {
                slots.insert(glow.texture as usize);
            }
        }

        self.load_effect_textures(game, &slots.into_iter().collect::<Vec<_>>());

        Ok(())
    }

    fn ordinary_held_glow(
        &self,
        id: u32,
        appearance: super::weapon_glow::Ordinary,
    ) -> Option<formats::mesh::glow::Glow> {
        self.by_id_meshes.get(&id)?;

        appearance.resolve(
            self.registries
                .textures
                .get(0)
                .is_some_and(|t| t.format_flag == FormatFlag::ColorKey),
        )
    }

    pub fn load_effect_textures(&mut self, game: &GameDir, slots: &[usize]) {
        let reg = Arc::clone(&self.registries);

        for &slot in slots {
            if self
                .bind_slot(game, &reg.effect_textures, true, slot)
                .is_none()
            {
                tracing::warn!(slot, "missing effect mesh texture override");
            }
        }
    }

    fn load_placed_effect_textures(&mut self, game: &GameDir) {
        let slots: std::collections::BTreeSet<_> = self
            .instances
            .iter()
            .filter(|i| i.kind == 0x736)
            .flat_map(|i| self.meshes[i.mesh].subset_slots.iter().flatten().copied())
            .collect();

        self.load_effect_textures(game, &slots.into_iter().collect::<Vec<_>>());
    }

    pub fn load_mesh_texture_overrides(&mut self, game: &GameDir, paths: &[&str]) {
        let reg = Arc::clone(&self.registries);

        for path in paths {
            if reg
                .textures
                .find(path.as_bytes())
                .and_then(|slot| self.bind_slot(game, &reg.textures, false, slot))
                .is_none()
            {
                tracing::warn!(path, "missing named effect mesh texture override");
            }
        }
    }

    pub fn load_mesh_texture_slots(&mut self, game: &GameDir, slots: &[usize]) {
        let reg = Arc::clone(&self.registries);

        for &slot in slots {
            if self.bind_slot(game, &reg.textures, false, slot).is_none() {
                tracing::warn!(slot, "missing mesh texture override");
            }
        }
    }

    pub fn load_texture_bias(&mut self, game: &GameDir, id: u32, bias: i32) {
        if bias == 0 {
            return;
        }

        let Some(&index) = self.by_id_meshes.get(&id) else {
            return;
        };

        let mesh = &self.meshes[index];

        let effect_table = mesh_list::is_effect_dir(mesh_list::split(&mesh.path).0);

        let slots: Vec<_> = mesh
            .subset_slots
            .iter()
            .map(|slot| slot.and_then(|s| s.checked_add_signed(bias as isize)))
            .collect();

        let reg = Arc::clone(&self.registries);

        let table = if effect_table {
            &reg.effect_textures
        } else {
            &reg.textures
        };

        for (subset, slot) in slots.into_iter().enumerate() {
            if slot
                .and_then(|slot| self.bind_slot(game, table, effect_table, slot))
                .is_none()
            {
                tracing::warn!(
                    id,
                    bias,
                    subset,
                    ?slot,
                    effect_table,
                    "missing biased ground-item texture"
                );
            }
        }
    }

    fn place_house(
        &mut self,
        game: Option<&GameDir>,
        list: &MeshList,
        textures: &TextureList,
        kind: u32,
        position: Vec3,
        yaw: f32,
    ) {
        if matches!(objects::house_type(kind), Some(2..=4)) {
            self.house_emitters.push(super::house::Source {
                kind,
                position,
                yaw,
                last_ms: 0,
            });
        }

        if let Some(game) = game
            && matches!(objects::house_type(kind), Some(2 | 3))
        {
            self.load_mesh_texture_slots(game, &[1, 2, 8]);
        }

        for part in super::house::parts(kind) {
            let mesh = match self.by_id_meshes.get(&part.mesh_id).copied() {
                Some(mesh) => Some(mesh),
                None => {
                    let Some(game) = game else {
                        continue;
                    };

                    let loaded = self
                        .load_mesh(game, list, textures, None, part.mesh_id, true)
                        .ok()
                        .flatten();

                    match loaded {
                        Some(mesh) => {
                            self.by_id_meshes.insert(part.mesh_id, mesh);

                            Some(mesh)
                        }
                        None => {
                            if !self.missing_by_id.contains(&part.mesh_id) {
                                self.missing_by_id.push(part.mesh_id);
                            }

                            None
                        }
                    }
                }
            };

            let Some(mesh) = mesh else {
                continue;
            };

            self.house_parts.push(HousePart {
                kind,
                mesh,
                position,
                yaw,
                part,
            });
        }
    }

    fn load_mesh(
        &mut self,
        game: &GameDir,
        list: &MeshList,
        textures: &TextureList,
        effect_textures: Option<&TextureList>,
        id: u32,
        bind: bool,
    ) -> Result<Option<usize>> {
        let Some(path) = list.get(id) else {
            return Ok(None);
        };

        let path = path.to_string();

        let bytes = { game.read(path.as_bytes()) };

        let Ok(bytes) = bytes else {
            return Ok(None);
        };

        let mesh = { msa::parse(&bytes).with_context(|| format!("parsing {path}"))? };

        let (dir, file_name) = mesh_list::split(&path);

        let effect_dir = mesh_list::is_effect_dir(dir);

        let mut subset_slots = Vec::with_capacity(mesh.subsets.len());

        let mut subset_textures = Vec::with_capacity(mesh.subsets.len());

        let (table, effect_table) = match effect_textures {
            Some(effect)
                if effect_dir
                    && self
                        .find_texture(
                            textures,
                            dir,
                            &msa::texture_base_name(
                                file_name,
                                mesh.texture_fields
                                    .first()
                                    .map(Vec::as_slice)
                                    .unwrap_or(&[]),
                            ),
                        )
                        .is_none() =>
            {
                (effect, true)
            }
            _ => (textures, effect_dir),
        };

        for field in &mesh.texture_fields {
            let base = msa::texture_base_name(file_name, field);

            let mut slot = self.find_texture(table, dir, &base);

            if slot.is_none() {
                let own = file_name.split('.').next().unwrap_or(file_name);

                slot = self.find_texture(table, dir, own);

                if slot.is_none() {
                    self.missing_textures.push(format!("{dir}\\{base}.wys"));
                }
            }

            subset_slots.push(slot);

            subset_textures.push(
                slot.filter(|_| bind)
                    .and_then(|s| self.bind_slot(game, table, effect_table, s)),
            );
        }

        let flag_pass = |t: Option<usize>| match t {
            Some(t) => static_mesh_pass(self.textures[t].flag),
            None => MeshPass::Opaque,
        };

        let subset_zero_pass = flag_pass(subset_textures.first().copied().flatten());

        let subset_passes: Vec<MeshPass> = subset_textures.iter().map(|&t| flag_pass(t)).collect();

        let alloc = self.arena.push(&mesh);

        let mut bounds = render::cull::Aabb::EMPTY;

        for v in &mesh.vertices {
            bounds.grow(v.position);
        }

        self.meshes.push(MeshResource {
            id,
            path,
            vertex_color: mesh.has_colors,
            mesh,
            bounds,
            alloc,
            subset_slots,
            subset_textures,
            subset_zero_pass,
            subset_passes,
        });

        Ok(Some(self.meshes.len() - 1))
    }

    fn find_texture(&self, textures: &TextureList, dir: &str, base: &str) -> Option<usize> {
        let dir = if mesh_list::is_effect_dir(dir) {
            "Effect"
        } else {
            "mesh"
        };

        textures.find(format!("{dir}\\{base}.wys").as_bytes())
    }

    fn bind_slot(
        &mut self,
        game: &GameDir,
        textures: &TextureList,
        effect_table: bool,
        slot: usize,
    ) -> Option<usize> {
        if let Some(i) = self
            .textures
            .iter()
            .position(|t| t.slot == slot && t.effect_table == effect_table)
        {
            return Some(i);
        }

        let entry = textures.get(slot)?;

        let bytes = { game.read(&entry.path).ok() };

        let texture = bytes.and_then(|bytes| formats::texture::decode(&bytes).ok());

        if texture.is_none() && effect_table {
            return None;
        }

        let pending = texture.is_none();

        let texture = texture.unwrap_or_else(super::actors::texture_loading::placeholder);

        self.textures.push(BoundTexture {
            mip_policy_override: None,
            prepared: None,
            pending,
            effect_table,
            slot,
            path: entry.path_str(),
            flag: entry.format_flag,
            texture: texture.into(),
        });

        Some(self.textures.len() - 1)
    }

    pub fn upload_textures(&self, gpu: &Gpu, renderer: &MeshRenderer) -> Vec<MeshTexture> {
        self.textures
            .iter()
            .map(|t| {
                let uploaded = render::texture::upload_material(
                    gpu,
                    &t.texture,
                    t.alpha_policy(),
                    t.mip_policy(false),
                    &format!("mesh texture {}", t.path),
                );

                renderer.bind_texture(gpu, &uploaded.view)
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &self,
        eye: Vec3,
        vendor: render::quirk::VendorClass,
        held: &[crate::scene::actors::HeldDraw],
        drops: &[crate::scene::drops::DropDraw],
        glows: &[crate::scene::lights::Glow],
        rotate_lit: &[RotateLitDraw],
        now_ms: u32,
        sky_enabled: bool,
        window_light_shafts: bool,
        frustum: &render::cull::Frustum,
    ) -> Frame {
        let mut objects = Vec::with_capacity(
            self.instances.len() + self.house_parts.len() + held.len() + drops.len() + 1,
        );

        let mut draws = Vec::new();

        let mut light_shafts = Vec::new();

        if let Some(sky) = self.sky.filter(|_| sky_enabled) {
            self.push_instance(
                &mut objects,
                &mut draws,
                sky.mesh,
                sky.world(eye),
                |_| DrawParams::SKY,
                Cull::Ccw,
                vendor,
            );
        }

        for (instance, placement) in self.instances.iter().zip(&self.placements) {
            let world = placement.world;

            if !frustum.admits(&placement.bounds) {
                continue;
            }

            if instance.kind == 0x736 {
                let mesh = &self.meshes[instance.mesh];

                let alloc = &self.arena.allocs[mesh.alloc];

                let (pass, cull, params) = effect_states(3, [1.0; 4]);

                let object = objects.len() as u32;

                objects.push(ObjectUniforms::new(world, &params, 0.0));

                for (i, subset) in alloc.subsets.iter().enumerate() {
                    let Some(texture) =
                        mesh.subset_slots
                            .get(i)
                            .copied()
                            .flatten()
                            .and_then(|slot| {
                                self.textures
                                    .iter()
                                    .position(|t| t.effect_table && t.slot == slot)
                            })
                    else {
                        continue;
                    };

                    draws.push(MeshDraw {
                        pass,
                        cull,
                        object,
                        texture,
                        stage1: None,
                        base_vertex: alloc.base_vertex,
                        subset: *subset,
                    });
                }

                continue;
            }

            let opacity = if objects::house_type(instance.kind) == Some(0xB) {
                let Some(opacity) =
                    super::house::proximity_opacity(placement.position, self.house_target)
                else {
                    continue;
                };

                opacity
            } else {
                placement.fade.opacity
            };

            let vertex_color = self.meshes[instance.mesh].vertex_color;

            let first_object = objects.len();

            let first_draw = draws.len();

            self.push_instance(
                &mut objects,
                &mut draws,
                instance.mesh,
                world,
                |pass| DrawParams::static_object(vertex_color, pass != MeshPass::Cutout),
                Cull::Ccw,
                vendor,
            );

            if opacity < 1.0 {
                for object in &mut objects[first_object..] {
                    object.set_opacity(opacity);
                }

                for draw in &mut draws[first_draw..] {
                    draw.pass = MeshPass::Blended;
                }
            }

            if reflective_scenery(instance.kind) {
                for object in &mut objects[first_object..] {
                    object.set_environment_reflection();
                }
            }
        }

        let swing = super::house::swing(now_ms);

        for part in &self.house_parts {
            let opacity = if part.part.gate == super::house::Gate::PlayerNotRoofed {
                if self.roof_hidden {
                    continue;
                }

                let Some(opacity) =
                    super::house::proximity_opacity(part.position, self.house_target)
                else {
                    continue;
                };

                opacity
            } else {
                1.0
            };

            let world = part.world(swing);

            if !self.visible(part.mesh, &world, frustum) {
                continue;
            }

            if matches!(objects::house_type(part.kind), Some(2 | 3)) {
                let outdoor = matches!(self.environment, 0 | 3 | 4);

                let slot = if outdoor {
                    2
                } else if objects::house_type(part.kind) == Some(3) {
                    1
                } else {
                    8
                };

                let Some(texture) = self
                    .textures
                    .iter()
                    .position(|t| !t.effect_table && t.slot == slot)
                else {
                    continue;
                };

                let mesh = &self.meshes[part.mesh];

                let alloc = &self.arena.allocs[mesh.alloc];

                let pass = if outdoor {
                    MeshPass::Reflective
                } else {
                    MeshPass::Blended
                };

                let material = Material {
                    diffuse: [0.5; 4],
                    ambient: [0.5, 0.5, 0.5, 0.0],
                    specular: [0.5; 4],
                    emissive: [0.0; 4],
                    power: 0.0,
                };

                let params = DrawParams::ground_drop(material, mesh.vertex_color, false);

                let mut uniform = ObjectUniforms::new(world, &params, 0.0);

                if outdoor {
                    uniform.set_stage1(render::mesh::Stage1Blend::Modulate2x, 0.0);
                }

                if mesh.mesh.fvf == 0x112 {
                    [uniform.uv0, uniform.uv1] =
                        super::house::uv_scroll(if outdoor { 4 } else { 7 }, now_ms);
                }

                let object = objects.len() as u32;

                objects.push(uniform);

                for (i, subset) in alloc.subsets.iter().enumerate() {
                    let Some(base) = (if i == 0 {
                        Some(texture)
                    } else {
                        mesh.subset_textures.get(i).copied().flatten()
                    }) else {
                        continue;
                    };

                    draws.push(MeshDraw {
                        pass,
                        cull: Cull::None,
                        object,
                        texture: base,
                        stage1: outdoor.then_some(texture),
                        base_vertex: alloc.base_vertex,
                        subset: *subset,
                    });
                }

                continue;
            }

            let vertex_color = self.meshes[part.mesh].vertex_color;

            let first_object = objects.len();

            let first_draw = draws.len();

            self.push_instance(
                &mut objects,
                &mut draws,
                part.mesh,
                world,
                |pass| DrawParams::static_object(vertex_color, pass != MeshPass::Cutout),
                Cull::Ccw,
                vendor,
            );

            if opacity < 1.0 {
                for uniform in &mut objects[first_object..] {
                    uniform.set_opacity(opacity);
                }

                for draw in &mut draws[first_draw..] {
                    draw.pass = MeshPass::Blended;
                }
            }

            if self.meshes[part.mesh].mesh.fvf == 0x112 {
                for uniform in &mut objects[first_object..] {
                    [uniform.uv0, uniform.uv1] = super::house::uv_scroll(part.part.uv_anim, now_ms);
                }
            }
        }

        for weapon in held {
            let Some(&mesh) = self.by_id_meshes.get(&weapon.mesh_id) else {
                continue;
            };

            if !self.visible(mesh, &weapon.world, frustum) {
                continue;
            }

            let vertex_color = self.meshes[mesh].vertex_color;

            let first = draws.len();

            if let super::actors::ActorMeshPlacement::Siege { selected } = weapon.placement {
                self.push_instance(
                    &mut objects,
                    &mut draws,
                    mesh,
                    weapon.world,
                    |pass| {
                        let mut params =
                            DrawParams::static_object(vertex_color, pass != MeshPass::Cutout);

                        if selected {
                            params.material = render::world::Material {
                                diffuse: [1.0, 1.0, 1.0, 0.0],
                                ambient: [0.0; 4],
                                specular: [1.0, 1.0, 1.0, 0.0],
                                emissive: [1.0, 1.0, 1.0, 0.0],
                                power: 0.0,
                            };
                        }

                        params
                    },
                    Cull::Ccw,
                    vendor,
                );

                continue;
            }

            self.push_instance(
                &mut objects,
                &mut draws,
                mesh,
                weapon.world,
                |pass| {
                    if weapon.silhouette {
                        DrawParams::held_silhouette(vertex_color, pass != MeshPass::Cutout)
                    } else {
                        DrawParams::held_weapon(vertex_color, pass != MeshPass::Cutout)
                    }
                },
                Cull::Cw,
                vendor,
            );

            if weapon.silhouette {
                for draw in &mut draws[first..] {
                    draw.pass = MeshPass::Silhouette;
                }
            } else if let Some(texture) = weapon.glow.and_then(|slot| {
                self.textures
                    .iter()
                    .position(|t| t.effect_table && t.slot == slot)
            }) {
                for draw in &mut draws[first..] {
                    draw.stage1 = Some(texture);

                    let color_key = self.textures[draw.texture].flag == FormatFlag::ColorKey;

                    let uniform = &mut objects[draw.object as usize];

                    uniform.set_stage1(
                        super::weapon_glow::blend(weapon.mesh_id, color_key),
                        super::weapon_glow::scroll_for(weapon.mesh_id, now_ms),
                    );

                    if !super::weapon_glow::class_run(weapon.mesh_id) {
                        uniform.refine[0] = 2.0;
                    }

                    uniform.alpha[0] = 0.0;

                    uniform.alpha[1] = self.textures[draw.texture].alpha_policy().as_uniform();

                    draw.pass = MeshPass::Opaque;

                    draw.cull = Cull::None;
                }
            } else if let Some(glow) = weapon
                .ordinary_glow
                .and_then(|g| self.ordinary_held_glow(weapon.mesh_id, g))
            {
                if let Some(texture) = self
                    .textures
                    .iter()
                    .position(|t| t.effect_table && t.slot == glow.texture as usize)
                {
                    let scroll = if glow.uv == formats::mesh::glow::Uv::Sphere
                        && self.meshes[mesh].mesh.fvf == 0x112
                        && weapon.ordinary_glow.is_some_and(|g| g.refine > 0)
                    {
                        (now_ms % 4000) as f32 / 4000.0
                    } else {
                        0.0
                    };

                    for draw in &mut draws[first..] {
                        draw.stage1 = Some(texture);

                        objects[draw.object as usize].set_refine(glow, scroll);
                    }
                }
            }
        }

        for drop in drops {
            let Some(&mesh) = self.by_id_meshes.get(&drop.mesh_id) else {
                continue;
            };

            if !self.visible(mesh, &drop.world, frustum) {
                continue;
            }

            if let Some(colour) = drop.flag_colour {
                let resource = &self.meshes[mesh];

                let alloc = &self.arena.allocs[resource.alloc];

                let mut params = DrawParams::glow(render::world::argb_to_rgba_f32(colour));

                params.force_opaque = true;

                params.fog = true;

                let pass = MeshPass::AdditiveSolid;

                let object = objects.len() as u32;

                objects.push(ObjectUniforms::new(
                    drop.world,
                    &params,
                    pass.alpha_reference(vendor),
                ));

                for (i, subset) in alloc.subsets.iter().enumerate() {
                    let slot = if i == 0 {
                        Some(204)
                    } else {
                        resource.subset_slots.get(i).copied().flatten()
                    };

                    let Some(texture) = slot.and_then(|slot| {
                        self.textures
                            .iter()
                            .position(|t| t.effect_table && t.slot == slot)
                    }) else {
                        continue;
                    };

                    draws.push(MeshDraw {
                        pass,
                        cull: Cull::Ccw,
                        object,
                        texture,
                        stage1: None,
                        base_vertex: alloc.base_vertex,
                        subset: *subset,
                    });
                }

                continue;
            }

            let vertex_color = self.meshes[mesh].vertex_color;

            let first_draw = draws.len();

            let first_object = objects.len();

            self.push_instance_biased(
                &mut objects,
                &mut draws,
                mesh,
                drop.world,
                |pass| {
                    DrawParams::ground_drop(drop.material, vertex_color, pass != MeshPass::Cutout)
                },
                Cull::Ccw,
                vendor,
                drop.tex_bias,
            );

            let overlay = drop.overlay.map(|overlay| (overlay, true)).or_else(|| {
                drop.gate_stage1_source
                    .filter(|id| self.uses_gate_texture_overlay(*id))
                    .map(|_| {
                        (
                            super::drops::RefineOverlay {
                                texture: 6,
                                blend: render::mesh::Stage1Blend::Modulate,
                            },
                            false,
                        )
                    })
            });

            if let Some((overlay, texture, scrolling)) = overlay.and_then(|(overlay, scrolling)| {
                self.textures
                    .iter()
                    .position(|t| t.effect_table && t.slot == overlay.texture)
                    .map(|texture| (overlay, texture, scrolling))
            }) {
                let scroll = if scrolling && self.meshes[mesh].mesh.fvf == 0x112 {
                    (now_ms % 4000) as f32 / 4000.0
                } else {
                    0.0
                };

                for object in &mut objects[first_object..] {
                    object.set_stage1(overlay.blend, scroll);
                }

                for draw in &mut draws[first_draw..] {
                    draw.stage1 = Some(texture);
                }
            }
        }

        for glow in glows {
            let Some(&mesh) = self.by_id_meshes.get(&(glow.mesh_id as u32)) else {
                continue;
            };

            let world = glow.world();

            if window_light_shafts
                && let Some(shafts) =
                    super::light_shafts::from_glow(&self.meshes[mesh], &self.textures, glow, now_ms)
            {
                light_shafts.extend(shafts);

                continue;
            }

            if !self.visible(mesh, &world, frustum) {
                continue;
            }

            let (pass, cull, params) = effect_states(EFFECT_ADDITIVE, glow.tint(now_ms));

            let first = objects.len();

            self.push_instance_as(
                &mut objects,
                &mut draws,
                mesh,
                world,
                pass,
                cull,
                &params,
                vendor,
                true,
            );

            for uniform in &mut objects[first..] {
                uniform.uv0[2..].copy_from_slice(&glow.uv_offset);
            }
        }

        for effect in rotate_lit {
            let Some(&mesh) = self.by_id_meshes.get(&(effect.mesh_id as u32)) else {
                continue;
            };

            if !self.visible(mesh, &effect.world, frustum) {
                continue;
            }

            let (pass, cull, params) = effect.states(self.meshes[mesh].vertex_color);

            self.push_instance_as(
                &mut objects,
                &mut draws,
                mesh,
                effect.world,
                pass,
                cull,
                &params,
                vendor,
                false,
            );
        }

        Frame {
            objects,
            draws,
            light_shafts,
        }
    }

    pub fn append_textured_effects(
        &self,
        frame: &mut Frame,
        effects: &[super::effect_tree::TexturedMesh],
        vendor: render::quirk::VendorClass,
        frustum: &render::cull::Frustum,
    ) {
        for effect in effects {
            let Some(&mesh_index) = self.by_id_meshes.get(&effect.mesh_id) else {
                continue;
            };

            let texture = self
                .textures
                .iter()
                .position(|t| t.effect_table && t.slot == effect.texture)
                .unwrap_or(render::mesh::NULL_TEXTURE);

            if !self.visible(mesh_index, &effect.world, frustum) {
                continue;
            }

            let mesh = &self.meshes[mesh_index];

            let alloc = &self.arena.allocs[mesh.alloc];

            let (pass, cull, params) = effect.states();

            let object = frame.objects.len() as u32;

            frame.objects.push(ObjectUniforms::new(
                effect.world,
                &params,
                pass.alpha_reference(vendor),
            ));

            if texture == render::mesh::NULL_TEXTURE {
                Self::null_effect_texture(
                    &mut frame.objects[object as usize],
                    &params,
                    mesh.vertex_color,
                );
            }

            frame.objects[object as usize].uv0[2..].copy_from_slice(&effect.uv_offset);

            for subset in &alloc.subsets {
                frame.draws.push(MeshDraw {
                    pass,
                    cull,
                    object,
                    texture,
                    stage1: None,
                    base_vertex: alloc.base_vertex,
                    subset: *subset,
                });
            }
        }
    }

    pub fn append_authored_effects(
        &self,
        frame: &mut Frame,
        effects: &[super::effect_tree::AuthoredMesh],
        vendor: render::quirk::VendorClass,
        frustum: &render::cull::Frustum,
    ) {
        for effect in effects {
            let Some(&mesh) = self.by_id_meshes.get(&effect.mesh_id) else {
                continue;
            };

            if !self.visible(mesh, &effect.world, frustum) {
                continue;
            }

            let (pass, cull, params) = effect.states(self.meshes[mesh].vertex_color);

            let override_texture = if let super::effect_tree::MeshMaterial::Swing {
                texture: Some(slot),
                ..
            } = effect.material
            {
                Some(
                    self.textures
                        .iter()
                        .position(|t| !t.effect_table && t.slot == slot),
                )
            } else {
                effect.texture.map(|path| {
                    self.textures.iter().position(|t| {
                        !t.effect_table
                            && t.path
                                .replace('\\', "/")
                                .eq_ignore_ascii_case(&path.replace('\\', "/"))
                    })
                })
            };

            if let Some(texture) = override_texture {
                let Some(texture) = texture else {
                    tracing::warn!(
                        mesh_id = effect.mesh_id, material =? effect.material,
                        "authored effect texture override is missing"
                    );

                    continue;
                };

                let object = frame.objects.len() as u32;

                frame.objects.push(ObjectUniforms::new(
                    effect.world,
                    &params,
                    pass.alpha_reference(vendor),
                ));

                if let super::effect_tree::MeshMaterial::Effect { uv_offset, .. } = effect.material
                {
                    frame.objects[object as usize].uv0[2..].copy_from_slice(&uv_offset);
                }

                let alloc = &self.arena.allocs[self.meshes[mesh].alloc];

                for subset in &alloc.subsets {
                    frame.draws.push(MeshDraw {
                        pass,
                        cull,
                        object,
                        texture,
                        stage1: None,
                        base_vertex: alloc.base_vertex,
                        subset: *subset,
                    });
                }

                continue;
            }

            let start = frame.draws.len();

            let first = frame.objects.len();

            self.push_instance_as(
                &mut frame.objects,
                &mut frame.draws,
                mesh,
                effect.world,
                pass,
                cull,
                &params,
                vendor,
                matches!(
                    effect.material,
                    super::effect_tree::MeshMaterial::Effect { .. }
                ),
            );

            if let super::effect_tree::MeshMaterial::Effect { uv_offset, .. } = effect.material {
                for uniform in &mut frame.objects[first..] {
                    uniform.uv0[2..].copy_from_slice(&uv_offset);
                }
            }

            if params.stage1 {
                let (effect_table, slot) = match effect.material {
                    super::effect_tree::MeshMaterial::Arrow { stage1: Some(slot) } => (true, slot),
                    _ => (false, 6),
                };

                let texture = self
                    .textures
                    .iter()
                    .position(|t| t.effect_table == effect_table && t.slot == slot);

                if let Some(texture) = texture {
                    for draw in &mut frame.draws[start..] {
                        draw.stage1 = Some(texture);
                    }
                } else {
                    tracing::warn!(
                        mesh_id = effect.mesh_id,
                        slot,
                        effect_table,
                        "effect stage-1 texture is missing"
                    );

                    frame.draws.truncate(start);
                }
            }
        }
    }

    fn visible(&self, mesh: usize, world: &Matrix4, frustum: &render::cull::Frustum) -> bool {
        frustum.admits(&self.meshes[mesh].bounds.transformed(world))
    }

    pub fn placement_of(&self, instance: &Instance) -> Placement {
        let position = static_position(instance.kind, instance.position);

        let world = Instance {
            position,
            ..*instance
        }
        .world();

        let bounds = self.meshes[instance.mesh].bounds.transformed(&world);

        Placement {
            position,
            world,
            bounds,
            fade: Default::default(),
        }
    }

    pub fn update_motion(&mut self, now: u32) {
        for (instance, placement) in self.instances.iter().zip(&mut self.placements) {
            if !super::bike::enabled(instance.kind) {
                continue;
            }

            placement.position = super::bike::position(instance.position, instance.yaw, now);

            placement.world = world_at(placement.position, instance.yaw, 0.0, 0.0);

            placement.bounds = self.meshes[instance.mesh]
                .bounds
                .transformed(&placement.world);
        }
    }

    pub fn update_fade(
        &mut self,
        scene: u32,
        target: Option<Vec3>,
        eye: Vec3,
        now: u32,
        heights: &formats::world::path::HeightField<'_>,
    ) {
        self.house_target = target;

        for (instance, placement) in self.instances.iter().zip(&mut self.placements) {
            let blocked = scene == 30000
                && target.is_some_and(|target| {
                    super::scenery_fade::enabled(instance.kind, placement.position)
                        && heights.at(placement.position.x as i32, placement.position.z as i32)
                            > 125
                        && super::scenery_fade::obstructs(&placement.bounds, eye, target)
                });

            placement.fade.update(now, blocked);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_instance(
        &self,
        objects: &mut Vec<ObjectUniforms>,
        draws: &mut Vec<MeshDraw>,
        mesh_index: usize,
        world: Matrix4,
        params: impl Fn(MeshPass) -> DrawParams,
        cull: Cull,
        vendor: render::quirk::VendorClass,
    ) {
        self.push_instance_biased(objects, draws, mesh_index, world, params, cull, vendor, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_instance_biased(
        &self,
        objects: &mut Vec<ObjectUniforms>,
        draws: &mut Vec<MeshDraw>,
        mesh_index: usize,
        world: Matrix4,
        params: impl Fn(MeshPass) -> DrawParams,
        cull: Cull,
        vendor: render::quirk::VendorClass,
        bias: i32,
    ) {
        let mesh = &self.meshes[mesh_index];

        let alloc = &self.arena.allocs[mesh.alloc];

        let mut slots: [Option<u32>; MeshPass::ALL.len()] = [None; MeshPass::ALL.len()];

        for (i, subset) in alloc.subsets.iter().enumerate() {
            let texture = if bias == 0 {
                mesh.subset_textures.get(i).copied().flatten()
            } else {
                let effect_table = mesh_list::is_effect_dir(mesh_list::split(&mesh.path).0);

                mesh.subset_slots
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|slot| slot.checked_add_signed(bias as isize))
                    .and_then(|slot| {
                        self.textures
                            .iter()
                            .position(|t| t.effect_table == effect_table && t.slot == slot)
                    })
            };

            let Some(texture) = texture else {
                continue;
            };

            let pass = if bias != 0 {
                static_mesh_pass(self.textures[texture].flag)
            } else {
                mesh.subset_passes
                    .get(i)
                    .copied()
                    .unwrap_or(MeshPass::Opaque)
            };

            let slot = &mut slots[MeshPass::ALL.iter().position(|p| *p == pass).unwrap_or(0)];

            let object = match *slot {
                Some(object) => object,
                None => {
                    let object = objects.len() as u32;

                    objects.push(ObjectUniforms::new(
                        world,
                        &params(pass),
                        pass.alpha_reference(vendor),
                    ));

                    *slot = Some(object);

                    object
                }
            };

            draws.push(MeshDraw {
                pass,
                cull,
                object,
                texture,
                stage1: None,
                base_vertex: alloc.base_vertex,
                subset: *subset,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_instance_as(
        &self,
        objects: &mut Vec<ObjectUniforms>,
        draws: &mut Vec<MeshDraw>,
        mesh_index: usize,
        world: Matrix4,
        pass: MeshPass,
        cull: Cull,
        params: &DrawParams,
        vendor: render::quirk::VendorClass,
        allow_null: bool,
    ) {
        let mesh = &self.meshes[mesh_index];

        let alloc = &self.arena.allocs[mesh.alloc];

        let object = objects.len() as u32;

        objects.push(ObjectUniforms::new(
            world,
            params,
            pass.alpha_reference(vendor),
        ));

        let mut null_object = None;

        for (i, subset) in alloc.subsets.iter().enumerate() {
            let (texture, object) = match mesh.subset_textures.get(i).copied().flatten() {
                Some(texture) => (texture, object),
                None if allow_null => {
                    if params.material.diffuse[3] == 0.0
                        && params.color_op != render::preset::ColorOp::SelectTextureRgb
                    {
                        continue;
                    }

                    let object = *null_object.get_or_insert_with(|| {
                        let mut uniform =
                            ObjectUniforms::new(world, params, pass.alpha_reference(vendor));

                        Self::null_effect_texture(&mut uniform, params, mesh.vertex_color);

                        let slot = objects.len() as u32;

                        objects.push(uniform);

                        slot
                    });

                    (render::mesh::NULL_TEXTURE, object)
                }
                None => continue,
            };

            draws.push(MeshDraw {
                pass,
                cull,
                object,
                texture,
                stage1: None,
                base_vertex: alloc.base_vertex,
                subset: *subset,
            });
        }
    }

    fn null_effect_texture(uniform: &mut ObjectUniforms, params: &DrawParams, vertex_color: bool) {
        if params.color_op == render::preset::ColorOp::SelectTextureRgb {
            uniform.flags[3] = f32::from(vertex_color);

            uniform.material_diffuse = [1.0; 4];
        }

        uniform.set_null_texture();
    }

    pub fn drawn_mesh_ids(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .instances
            .iter()
            .map(|instance| self.meshes[instance.mesh].id)
            .filter(|&id| id != SKY_MESH_ID)
            .collect();

        v.sort_unstable();

        v.dedup();

        v
    }

    pub fn suppressed(&self) -> usize {
        self.classes.get("suppressed")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub objects: Vec<ObjectUniforms>,
    pub draws: Vec<MeshDraw>,
    pub light_shafts: Vec<render::light_shafts::LightShaft>,
}
