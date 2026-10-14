use super::pick::{self, Ray};
use formats::data::item_list::ItemList;
use formats::mesh::appearance::ground_item_mesh;
use formats::net::account::Item;
use formats::net::ground_item;
use formats::world::path::HeightField;
use glam::Vec3;

pub const LIFT: f32 = 0.1;

pub const CENTRE: f32 = 0.5;

pub const QUARTER_TURN: f32 = std::f32::consts::FRAC_PI_2;

pub const DEFAULT_PITCH: f32 = std::f32::consts::FRAC_PI_2;

pub const TIPPED_PITCH: f32 = -std::f32::consts::FRAC_PI_2;

pub const USE_KIND: u8 = formats::data::item_effect::effect::USE_KIND;

pub const GATE_EFFECT: u8 = 0x22;

pub const QUEST_FLAG_MESH: i16 = 0x647;

pub const DESPAWN_EXEMPT: [i32; 2] = [0x195, 0x647];

pub const DESPAWN_EXEMPT_RANGE: std::ops::RangeInclusive<i32> = 0xAA3..=0xAA7;

pub const DESPAWN_HALF_BOX: f32 = 18.0;

pub const FLAG_MESHES: std::ops::RangeInclusive<i16> = 0x2E1..=0x2E3;

pub const FLAG_COLOURS: [u32; 3] = [0xFFFF_4400, 0xFF33_66FF, 0xFF00_FF00];

pub const BIAS_FIXUP_MESH: i16 = 0x1B;

pub const BIAS_OVERRIDE_ITEM: i16 = 0xA9;

pub const BIAS_OVERRIDE: i32 = -4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub pitch: f32,
    pub lift: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Pose {
            pitch: DEFAULT_PITCH,
            lift: 0.0,
        }
    }
}

#[allow(clippy::if_same_then_else)]
pub fn pose(id: i16, items: &ItemList, use_kind: i32, drawn_mesh: i32) -> Pose {
    let mut out = Pose::default();

    if matches!(use_kind, 1 | 2 | 4 | 5) {
        out.pitch = 0.0;
    }

    let mesh = items
        .get(usize::try_from(id).unwrap_or(usize::MAX))
        .map_or(0, |e| e.mesh);

    if mesh == 0x2D || id == 0x1A3 || id == 0x1A4 || id == 0x2EB {
        out.pitch = 0.0;
    } else if (0x6A5..=0x6B0).contains(&id) {
        out.pitch = TIPPED_PITCH;
    } else if id == 0x6BF || (0x6C5..=0x6C8).contains(&id) {
        out.pitch = 0.0;
    } else if id == 0x6D8 || (0x6E0..=0x6E3).contains(&id) {
        out = Pose {
            pitch: 0.0,
            lift: 0.1,
        };
    } else if (0x6D0..=0x6D7).contains(&id) {
        out = Pose {
            pitch: 0.0,
            lift: 0.3,
        };
    } else if (0x8FC..=0x919).contains(&id) {
        out = Pose {
            pitch: 0.0,
            lift: 0.3,
        };
    } else if (0x91A..=0x991).contains(&id) || (0xB90..=0xBB7).contains(&id) {
        out.pitch = 0.0;
    } else if (0x13F6..=0x140D).contains(&id) {
        out.pitch = TIPPED_PITCH;
    } else if id == 0xD73 || id == 0xD74 || id == 0x140F {
        out = Pose {
            pitch: 0.0,
            lift: -0.13,
        };
    } else if (0xC7D..=0xC7F).contains(&id) {
        out.pitch = TIPPED_PITCH;
    } else if (0xDAC..=0xDAE).contains(&id)
        || (0xC44..=0xDDE).contains(&id)
        || (0xC49..=0xC4D).contains(&id)
        || (0xF8A..=0xF9F).contains(&id)
    {
        out.pitch = 0.0;
    } else if id == 0xFAD {
        out.pitch = TIPPED_PITCH;
    } else if (0x100A..=0x100D).contains(&id) {
        out.pitch = 0.0;
    } else if id == 0x14DA {
        out = Pose {
            pitch: 0.0,
            lift: -0.15,
        };
    } else if id == 0x140E {
        out = Pose {
            pitch: 0.0,
            lift: -0.1,
        };
    } else if id == 0xD77 || id == 0xD78 {
        out.lift = -0.1;
    } else if id == 0x1411 {
        out.pitch = TIPPED_PITCH;
    }

    if drawn_mesh == i32::from(QUEST_FLAG_MESH) || drawn_mesh == DESPAWN_EXEMPT[0] {
        out.pitch = 0.0;
    }

    out
}

pub fn tex_bias(id: i16, items: &ItemList) -> i32 {
    let Some(entry) = items.get(usize::try_from(id).unwrap_or(usize::MAX)) else {
        return 0;
    };

    let sub = entry.mesh_sub;

    let mut bias = i32::from(sub);

    if entry.mesh == BIAS_FIXUP_MESH {
        if (6..=9).contains(&sub) {
            bias -= 1;
        } else if (0xB..=0x11).contains(&sub) {
            bias -= 2;
        } else if sub == 0x13 {
            bias = 0x10;
        } else if sub == 0x15 || sub == 0x16 {
            bias -= 4;
        }
    }

    bias
}

pub fn emissive(light: [f32; 4], hovered: bool, use_kind: i32, id: i16) -> [f32; 4] {
    let lit = |scale: f32, base: f32| {
        [
            light[0] * scale + base,
            light[1] * scale + base,
            light[2] * scale + base,
            0.0,
        ]
    };

    if hovered && !(0xC49..=0xC4D).contains(&id) {
        return [1.0, 1.0, 1.0, 0.0];
    }

    if id == 0x6BF {
        return lit(0.4, 0.6);
    }

    match matches!(use_kind, 1..=3) {
        true => lit(0.2, 0.7),
        false => lit(0.3, 0.3),
    }
}

pub fn material(
    light: [f32; 4],
    hovered: bool,
    use_kind: i32,
    id: i16,
    refine: i32,
) -> render::world::Material {
    let mut out = render::world::Material {
        diffuse: light,
        ambient: [1.0, 1.0, 1.0, 1.0],
        specular: light,
        emissive: emissive(light, hovered, use_kind, id),
        power: 0.0,
    };

    dim_for_refine(&mut out, refine);

    out
}

pub fn dim_for_refine(material: &mut render::world::Material, refine: i32) {
    if refine <= 0 {
        return;
    }

    material.diffuse[0] *= 0.5;

    material.diffuse[1] /= 2.4;

    material.diffuse[2] /= 2.8;

    material.specular = material.diffuse;
}

pub fn pick_box(id: i16, drawn_mesh: i32) -> Option<(f32, f32)> {
    if drawn_mesh == DESPAWN_EXEMPT[0] {
        return None;
    }

    let mut w = 0.6f32;

    let mut h = 0.7f32;

    if drawn_mesh == i32::from(QUEST_FLAG_MESH) {
        w = 1.0;
    }

    if (0xC49..=0xC4D).contains(&id) {
        w = 3.0;

        h = 3.0;
    }

    Some((w, h))
}

pub fn flag_colour(mesh: i16) -> Option<u32> {
    FLAG_MESHES
        .contains(&mesh)
        .then(|| FLAG_COLOURS[usize::try_from(mesh - *FLAG_MESHES.start()).unwrap_or(0)])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineOverlay {
    pub texture: usize,
    pub blend: render::mesh::Stage1Blend,
}

pub fn overlay_for_class(class: i16, refine: i32) -> Option<RefineOverlay> {
    use render::mesh::Stage1Blend;

    let refine = refine.clamp(0, 12) as usize;

    if refine == 0 && !(4..=8).contains(&class) {
        return None;
    }

    let base = match class {
        4 => 179,
        5 | 9 => 153,
        6 | 10 => 166,
        7 | 11 => 246,
        8 | 12 => 260,
        116..=125 => [275, 288, 301, 314, 327, 340, 353, 366, 425, 392][(class - 116) as usize] - 1,
        _ => 233,
    };

    Some(RefineOverlay {
        texture: base + refine,
        blend: if class == 4 {
            Stage1Blend::Modulate
        } else {
            Stage1Blend::AddSmooth
        },
    })
}

pub fn refine_overlay(item: &Item, items: &ItemList) -> Option<RefineOverlay> {
    use formats::mesh::{appearance, glow};

    if item.index == 0 || item.index > 0x38A4 {
        return None;
    }

    let entry = items.get(item.index as usize)?;

    if FLAG_MESHES.contains(&entry.mesh) {
        return None;
    }

    let refine = appearance::refine_level(item, items).min(12) as i32;

    let tint = glow::tint(item);

    let mut class = if (116..=125).contains(&tint) {
        i16::from(tint)
    } else {
        entry.tier
    };

    if (entry.grade == 51 || class != 0) && class < 5 && refine >= 10 {
        class = i16::from(appearance::refine_rank(item, items)) + 4;
    }

    overlay_for_class(class, refine)
}

pub fn sound(id: i16, use_kind: i32) -> u32 {
    if use_kind == 2 {
        return 0x2C;
    }

    match id {
        0x19C | 0x19D | 0x102D | 0x1A3 | 0x1A4 => 0x30,
        0x2EB => 0x132,
        _ => 0x2D,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drop {
    pub entity: u16,
    pub item: Item,
    pub cell: (u16, u16),
    pub position: Vec3,
    pub yaw: f32,
    pub mesh: Option<i32>,
    pub use_kind: i32,
    pub pose: Pose,
    pub tex_bias: i32,
    pub refine: i32,
    pub overlay: Option<RefineOverlay>,
    pub flag_colour: Option<u32>,
    pub hovered: bool,
    pub cannon: Option<super::cannon::State>,
}

impl Drop {
    pub fn drawable(&self) -> bool {
        self.mesh.is_some() && self.item.index != 0
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drops {
    pub items: Vec<Drop>,
}

impl Drops {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn get(&self, entity: u16) -> Option<&Drop> {
        self.items.iter().find(|d| d.entity == entity)
    }

    pub fn place(
        &mut self,
        packet: &ground_item::Drop,
        items: &ItemList,
        effects: &formats::data::item_effect::Effects<'_>,
        field: &HeightField,
    ) -> (usize, bool) {
        let y = field.ground(i32::from(packet.x), i32::from(packet.z)) + LIFT;

        self.place_at(packet, items, effects, y)
    }

    pub fn place_at(
        &mut self,
        packet: &ground_item::Drop,
        items: &ItemList,
        effects: &formats::data::item_effect::Effects<'_>,
        y: f32,
    ) -> (usize, bool) {
        let id = packet.item.index as i16;

        let template = items.get(usize::try_from(id).unwrap_or(usize::MAX));

        let quest_flag = template.map(|e| e.mesh) == Some(QUEST_FLAG_MESH);

        let previous = self.get(packet.entity);

        let flag_colour = items
            .get(usize::try_from(id).unwrap_or(usize::MAX))
            .and_then(|e| flag_colour(e.mesh));

        let mesh = (0..=0x38A4)
            .contains(&id)
            .then(|| ground_item_mesh(i32::from(id), items));

        let facing = match quest_flag && previous.is_none() {
            true => 1,
            false => packet.direction,
        };

        let use_kind = effects.total_scaled(&packet.item, USE_KIND);

        let drawn = mesh.unwrap_or(-1);

        let overlay = refine_overlay(&packet.item, items);

        let bias = match id == BIAS_OVERRIDE_ITEM && overlay.is_none() {
            true => BIAS_OVERRIDE,
            false => tex_bias(id, items),
        };

        let (x, z) = (f32::from(packet.x), f32::from(packet.z));

        let mut cannon = match previous {
            Some(drop) => drop.cannon.clone(),
            None => quest_flag.then(Default::default),
        };

        if let Some(state) = &mut cannon {
            state.place(mesh);
        }

        let drop = Drop {
            entity: packet.entity,
            item: packet.item,
            cell: (packet.x, packet.z),
            position: Vec3::new(x + CENTRE, y, z + CENTRE),
            yaw: f32::from(facing) * QUARTER_TURN,
            mesh,
            use_kind,
            pose: if flag_colour.is_some() {
                Pose {
                    pitch: 0.0,
                    lift: 0.0,
                }
            } else {
                pose(id, items, use_kind, drawn)
            },
            tex_bias: bias,
            refine: i32::try_from(formats::mesh::appearance::refine_level(&packet.item, items))
                .unwrap_or(0)
                .min(REFINE_CLAMP),
            overlay,
            flag_colour,
            hovered: false,
            cannon,
        };

        match self.items.iter().position(|d| d.entity == packet.entity) {
            Some(at) => {
                self.items[at] = drop;

                (at, false)
            }
            None => {
                self.items.push(drop);

                (self.items.len() - 1, true)
            }
        }
    }

    pub fn remove(&mut self, entity: u16) -> bool {
        match self.items.iter().position(|d| d.entity == entity) {
            Some(at) => {
                self.items.remove(at);

                true
            }
            None => false,
        }
    }

    pub fn despawn(&mut self, centre: (i32, i32)) -> Vec<u16> {
        let (lo_x, hi_x) = (
            centre.0 as f32 - DESPAWN_HALF_BOX,
            centre.0 as f32 + DESPAWN_HALF_BOX,
        );

        let (lo_z, hi_z) = (
            centre.1 as f32 - DESPAWN_HALF_BOX,
            centre.1 as f32 + DESPAWN_HALF_BOX,
        );

        let mut gone = Vec::new();

        self.items.retain(|d| {
            let outside = d.position.x < lo_x
                || hi_x < d.position.x
                || d.position.z < lo_z
                || hi_z < d.position.z;

            let exempt = d
                .mesh
                .is_some_and(|m| DESPAWN_EXEMPT.contains(&m) || DESPAWN_EXEMPT_RANGE.contains(&m));

            if outside && !exempt {
                gone.push(d.entity);

                return false;
            }

            true
        });

        gone
    }

    pub fn mesh_ids(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .items
            .iter()
            .filter_map(|d| d.mesh)
            .filter_map(|m| u32::try_from(m).ok())
            .collect();

        out.sort_unstable();

        out.dedup();

        out
    }
}

pub const REFINE_CLAMP: i32 = 0xC;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropDraw {
    pub mesh_id: u32,
    pub world: render::camera::Matrix4,
    pub material: render::world::Material,
    pub overlay: Option<RefineOverlay>,
    pub tex_bias: i32,
    pub flag_colour: Option<u32>,
    pub gate_stage1_source: Option<u32>,
}

pub fn draws(drops: &Drops, light: impl Fn(Vec3) -> [f32; 4]) -> Vec<DropDraw> {
    drops
        .items
        .iter()
        .rev()
        .filter(|d| d.drawable())
        .filter_map(|d| {
            let mesh_id = u32::try_from(d.mesh?).ok()?;

            let at = d.position
                + Vec3::new(0.0, d.pose.lift, 0.0)
                + d.cannon.as_ref().map_or(Vec3::ZERO, |state| state.recoil);

            Some(DropDraw {
                mesh_id,
                overlay: d.overlay,
                tex_bias: d.tex_bias,
                flag_colour: d.flag_colour,
                gate_stage1_source: None,
                world: super::objects::world_at(at, d.yaw, d.pose.pitch, 0.0),
                material: material(
                    light(d.position),
                    d.hovered,
                    d.use_kind,
                    d.item.index as i16,
                    d.refine,
                ),
            })
        })
        .collect()
}

pub fn hits(ray: &Ray, drop: &Drop) -> bool {
    if drop.item.index == 0 {
        return false;
    }

    let Some((hw, h)) = pick_box(drop.item.index as i16, drop.mesh.unwrap_or(-1)) else {
        return false;
    };

    let (x, y, z) = (drop.position.x, drop.position.y, drop.position.z);

    pick::hits_quad(
        ray,
        Vec3::new(x - hw, y + h, z),
        Vec3::new(x - hw, y, z),
        Vec3::new(x + hw, y + h, z),
        Vec3::new(x + hw, y, z),
    ) || pick::hits_quad(
        ray,
        Vec3::new(x, y + h, z - hw),
        Vec3::new(x, y, z - hw),
        Vec3::new(x, y + h, z + hw),
        Vec3::new(x, y, z + hw),
    )
}

pub fn hover(drops: &mut Drops, ray: &Ray, player_present: bool) -> Option<u16> {
    let mut claim = None;

    for drop in drops.items.iter_mut().rev() {
        drop.hovered = hits(ray, drop);

        if !drop.hovered {
            continue;
        }

        if !player_present || claim.is_none() {
            claim = Some(drop.entity);
        }
    }

    claim
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub entity: u16,
    pub id: i16,
    pub cell: (u16, u16),
    pub fresh: bool,
    pub mesh: Option<i32>,
    pub sound: Option<u32>,
}

pub fn apply(
    scene: &mut super::field::FieldScene,
    game: &assets::GameDir,
    packet: &ground_item::Drop,
) -> anyhow::Result<Placed> {
    use anyhow::Context as _;

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let effects = scene.statics.effects();

    let (at, fresh) = scene
        .drops
        .place(packet, &scene.statics.items, &effects, &field);

    let drop = &scene.drops.items[at];

    super::drop_presentation::bind(&mut scene.world.effects, game, &drop.item, &effects);

    let (id, mesh, use_kind) = (packet.item.index as i16, drop.mesh, drop.use_kind);

    if let Some(overlay) = drop.overlay {
        scene
            .world
            .objects
            .load_effect_textures(game, &[overlay.texture]);
    }

    if drop.flag_colour.is_some() {
        scene.world.objects.load_effect_textures(game, &[204]);
    }

    if let Some(mesh) = mesh.and_then(|m| u32::try_from(m).ok()) {
        scene
            .world
            .objects
            .load_by_id(game, &[mesh])
            .with_context(|| format!("loading the ground mesh {mesh} for item {id}"))?;

        scene
            .world
            .objects
            .load_texture_bias(game, mesh, drop.tex_bias);
    }

    let sound = packet.plays_sound().then(|| sound(id, use_kind));

    Ok(Placed {
        entity: packet.entity,
        id,
        cell: (packet.x, packet.z),
        fresh,
        mesh,
        sound,
    })
}

pub fn describe(placed: &Placed) -> String {
    let what = match placed.fresh {
        true => "dropped",
        false => "re-placed",
    };

    let mesh = match placed.mesh {
        Some(mesh) => format!("mesh {mesh}"),
        None => "no mesh".to_string(),
    };

    format!(
        "NWYD: 0x26E — item {} {what} at ({}, {}) as entity {} ({mesh})",
        placed.id, placed.cell.0, placed.cell.1, placed.entity
    )
}
