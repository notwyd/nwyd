use super::pick::{self, Ray};
use formats::data::item_list::ItemList;
use formats::net::account::Item;
use formats::net::gate_command;
use formats::net::ground_item;
use formats::world::gate_stamp;
use glam::Vec3;
use render::camera::Matrix4;

pub const HALF_WIDTH: f32 = 1.2;

pub const PICK_EXTENT: f32 = 2.0;

pub const PICK_HEIGHT: f32 = 2.0;

pub const PICK_HEIGHT_TALL: f32 = 6.0;

pub const PICK_OFFSET_SLOT_MACHINE: f32 = 1.3;

pub const ZERO_EXTENT: f64 = 0.01;

pub const QUARTER_TURN: f32 = std::f32::consts::FRAC_PI_2;

pub const SWING_MS: u32 = 1000;

pub const PITCH: f32 = -std::f32::consts::FRAC_PI_2;

pub const DELEGATED: std::ops::RangeInclusive<i16> = 0xC49..=0xC4D;

pub const DESPAWN_HALF_BOX: f32 = 33.0;

pub const BUSH: i16 = 0x167;

pub const SUNKEN_ENTITY: u16 = 0x2746;

pub const SUNKEN_FLOOR: f32 = -500.0;

pub mod mesh {

    pub const CASTLE_GATE: i32 = 0x79;

    pub const KEYED_GATE: i32 = 0x88;

    pub const KEYED_GATE_LEAF: i32 = 0x89;

    pub const STEEL_WINDOW: i32 = 0x8D;

    pub const GUILD_BATTLE: i32 = 0x94;

    pub const GUILD_BATTLE_ALT: i32 = 0x96;

    pub const FIRST_GATE: i32 = 0x1E0;

    pub const SLATES: std::ops::RangeInclusive<i32> = 0x3DD..=0x3E0;

    pub const GATE_OF_TIME: i32 = 0x6D5;

    pub const SLOT_MACHINE: i32 = 0xAE0;
}

pub mod state {

    pub const OPEN: i32 = 1;

    pub const CLOSED: i32 = 2;

    pub const CLOSED_PICKABLE: i32 = 3;

    pub const OPENING: i32 = 4;

    pub const CLOSING: i32 = 5;

    pub const CLOSING_PICKABLE: i32 = 6;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    #[default]
    TwoLeaves,
    One,
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickBox {
    pub minus_x: f32,
    pub plus_x: f32,
    pub minus_z: f32,
    pub plus_z: f32,
}

impl PickBox {
    pub const fn uniform(v: f32) -> PickBox {
        PickBox {
            minus_x: v,
            plus_x: v,
            minus_z: v,
            plus_z: v,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Table {
    pub mesh_a: i32,
    pub mesh_b: i32,
    pub half_width: f32,
    pub pick: PickBox,
    pub style: Style,
    pub glow: Option<u32>,
}

pub const SLATE_COLOURS: [u32; 4] = [0xFF00_4488, 0xFF66_6600, 0xFF00_8800, 0xFF88_0000];

pub const GLOW_KIND: i32 = 2;

pub const GLOW_SIZE: f32 = 1.8;

pub const GLOW_ALPHA: f32 = 0.3;

pub const GLOW_ALPHA_BASE: f32 = 0.2;

pub fn table(mesh: i32) -> Table {
    let mut t = Table {
        mesh_a: mesh,
        mesh_b: mesh,
        half_width: HALF_WIDTH,
        pick: PickBox {
            minus_x: 0.0,
            plus_x: PICK_EXTENT,
            minus_z: 0.0,
            plus_z: PICK_EXTENT,
        },
        style: Style::TwoLeaves,
        glow: None,
    };

    if mesh == mesh::KEYED_GATE {
        t.mesh_a = mesh::KEYED_GATE_LEAF;

        t.half_width = 1.1;

        t.pick = PickBox {
            minus_x: 1.3,
            plus_x: 2.4,
            minus_z: 1.3,
            plus_z: 2.4,
        };
    } else if mesh == mesh::FIRST_GATE {
        t.mesh_a = mesh::FIRST_GATE;

        t.half_width = 1.14;

        t.pick = PickBox {
            minus_x: 1.3,
            plus_x: 2.4,
            minus_z: 1.3,
            plus_z: 2.4,
        };
    } else if mesh == mesh::STEEL_WINDOW {
        t.pick = PickBox::uniform(1.0);
    } else if mesh::SLATES.contains(&mesh) {
        t.pick = PickBox::uniform(1.0);

        t.glow = Some(SLATE_COLOURS[(mesh - *mesh::SLATES.start()) as usize]);
    } else if mesh == mesh::GUILD_BATTLE {
        t.pick = PickBox::uniform(0.0);
    } else if mesh == mesh::GATE_OF_TIME {
        t.half_width = 1.65;
    } else if mesh == mesh::SLOT_MACHINE {
        t.half_width = 1.0;

        t.pick = PickBox::uniform(10.0);
    } else {
        t.pick = PickBox {
            minus_x: 1.0,
            plus_x: 3.0,
            minus_z: 1.0,
            plus_z: 3.0,
        };
    }

    t.style = style(t.mesh_a, t.mesh_b);

    t
}

pub fn style(mesh_a: i32, mesh_b: i32) -> Style {
    let mut style = if mesh_a == mesh::GUILD_BATTLE || mesh_a == mesh::GUILD_BATTLE_ALT {
        Style::One
    } else {
        Style::TwoLeaves
    };

    if mesh_a == mesh::STEEL_WINDOW {
        style = Style::Two;
    }

    if mesh::SLATES.contains(&mesh_b) {
        style = Style::One;
    }

    if mesh_b == mesh::SLOT_MACHINE {
        style = Style::One;
    }

    style
}

pub fn base_offset(item_id: i16, mesh_b: i32) -> f32 {
    let mut base = if mesh_b == mesh::KEYED_GATE {
        0.2
    } else if mesh_b == mesh::GATE_OF_TIME {
        0.75
    } else {
        0.0
    };

    if item_id == 0x305 || item_id == 800 {
        base = 0.61;
    }

    base
}

pub fn command(current: i32, asked: i32) -> Option<i32> {
    let refused = (current == state::OPEN && asked == state::OPENING)
        || (current == state::CLOSED && asked == state::CLOSING)
        || (current == state::CLOSED_PICKABLE && asked == state::CLOSING_PICKABLE);

    (!refused).then_some(asked)
}

pub const WINDOW_SOUND: u32 = 0x39;

pub fn command_sound(current: i32, asked: i32, mesh_b: i32) -> Option<u32> {
    if command(current, asked).is_none() || mesh_b != mesh::STEEL_WINDOW {
        return None;
    }

    (asked == state::OPENING || asked == state::CLOSING).then_some(WINDOW_SOUND)
}

pub fn advance(current: i32, elapsed: u32) -> (i32, Option<f32>) {
    let ramp = elapsed as f32 * 0.001 * std::f32::consts::PI * 0.5;

    match current {
        state::CLOSED | state::CLOSED_PICKABLE => (current, Some(0.0)),
        state::OPEN => (current, Some(QUARTER_TURN)),
        state::OPENING => {
            let next = if elapsed > SWING_MS {
                state::OPEN
            } else {
                current
            };

            (next, Some(ramp))
        }
        state::CLOSING | state::CLOSING_PICKABLE => {
            let next = match (elapsed > SWING_MS, current) {
                (true, state::CLOSING) => state::CLOSED,
                (true, _) => state::CLOSED_PICKABLE,
                (false, _) => current,
            };

            (next, Some(QUARTER_TURN - ramp))
        }
        _ => (current, None),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    pub entity: u16,
    pub item_id: i16,
    pub item: Item,
    pub cell: (u16, u16),
    pub position: Vec3,
    pub yaw: f32,
    pub direction: u8,
    pub table: Table,
    pub state: i32,
    pub since_ms: u32,
    pub swing: f32,
    pub in_box: bool,
    pub hovered: bool,
}

impl Gate {
    pub fn delegated(&self) -> bool {
        DELEGATED.contains(&self.item_id)
    }
}

pub fn quadrant(yaw: f32) -> i32 {
    (f64::from(yaw) / f64::from(QUARTER_TURN)) as i32
}

pub fn hits(ray: &Ray, gate: &Gate) -> bool {
    let mesh_b = gate.table.mesh_b;

    if mesh::SLATES.contains(&mesh_b) || mesh_b == mesh::GATE_OF_TIME {
        return false;
    }

    if f64::from(gate.table.pick.plus_x).abs() < ZERO_EXTENT {
        return false;
    }

    if gate.state != state::CLOSED_PICKABLE && mesh_b != mesh::SLOT_MACHINE {
        return false;
    }

    let height = if mesh_b == mesh::GATE_OF_TIME {
        PICK_HEIGHT_TALL
    } else {
        PICK_HEIGHT
    };

    let offset = if mesh_b == mesh::SLOT_MACHINE {
        PICK_OFFSET_SLOT_MACHINE
    } else {
        0.0
    };

    let (x, y, z) = (gate.position.x, gate.position.y, gate.position.z);

    let p = gate.table.pick;

    match quadrant(gate.yaw) {
        0 | 2 => pick::hits_quad(
            ray,
            Vec3::new(x - p.minus_x, y + height, z),
            Vec3::new(x - p.minus_x, y, z),
            Vec3::new(x + p.plus_x, y + height, z),
            Vec3::new(x + p.plus_x, y, z),
        ),
        _ => pick::hits_quad(
            ray,
            Vec3::new(x + offset, y + height, z - p.minus_z),
            Vec3::new(x + offset, y, z - p.minus_z),
            Vec3::new(x + offset, y + height, z + p.plus_z),
            Vec3::new(x + offset, y, z + p.plus_z),
        ),
    }
}

pub fn leaf_world(at: Vec3, yaw: f32, dx: f32, spin: f32) -> Matrix4 {
    let outer = Matrix4::multiply(
        &Matrix4::rotation_yaw_pitch_roll(yaw, 0.0, 0.0),
        &Matrix4::translation(at.x, at.y, at.z),
    );

    let inner = Matrix4::multiply(
        &Matrix4::rotation_yaw_pitch_roll(spin, PITCH, 0.0),
        &Matrix4::translation(dx, 0.0, 0.0),
    );

    Matrix4::multiply(&inner, &outer)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub mesh: i32,
    pub world: Matrix4,
}

pub fn poses(gate: &Gate) -> Vec<Pose> {
    if gate.item_id == BUSH || gate.delegated() {
        return Vec::new();
    }

    let t = &gate.table;

    let (x, y, z) = (gate.position.x, gate.position.y, gate.position.z);

    match t.style {
        Style::TwoLeaves => {
            let base = base_offset(gate.item_id, t.mesh_b);

            let at = Vec3::new(x, y, z);

            vec![
                Pose {
                    mesh: t.mesh_a,
                    world: leaf_world(
                        at,
                        gate.yaw,
                        base + t.half_width,
                        std::f32::consts::PI - gate.swing,
                    ),
                },
                Pose {
                    mesh: t.mesh_b,
                    world: leaf_world(
                        at,
                        gate.yaw,
                        base - (t.half_width + t.half_width),
                        gate.swing,
                    ),
                },
            ]
        }
        Style::One if t.mesh_b == mesh::SLOT_MACHINE => Vec::new(),
        Style::One => {
            let at = if mesh::SLATES.contains(&t.mesh_b) {
                Vec3::new(x - 1.2, y - gate.swing - 0.9, z)
            } else {
                Vec3::new(x, y - gate.swing, z + 1.2)
            };

            vec![Pose {
                mesh: t.mesh_a,
                world: one_world(at, gate.yaw, t.half_width),
            }]
        }
        Style::Two => {
            let at = Vec3::new(x + 0.3, y + gate.swing, z + 1.2);

            vec![Pose {
                mesh: t.mesh_a,
                world: one_world(at, gate.yaw, t.half_width),
            }]
        }
    }
}

fn one_world(at: Vec3, yaw: f32, half_width: f32) -> Matrix4 {
    leaf_world(at, yaw, half_width, std::f32::consts::PI)
}

pub fn glow_alpha(swing: f32) -> f32 {
    GLOW_ALPHA_BASE - swing
}

pub fn material(light: [f32; 4], hovered: bool) -> render::world::Material {
    let (rgb, glow) = match hovered {
        true => ([light[0], light[1], light[2]], 1.0),
        false => ([0.7, 0.7, 0.7], 0.3),
    };

    let both = [rgb[0], rgb[1], rgb[2], light[3]];

    render::world::Material {
        diffuse: both,
        ambient: [0.0, 0.0, 0.0, 0.0],
        specular: both,
        emissive: [glow, glow, glow, 0.0],
        power: 0.0,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gates {
    pub items: Vec<Gate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Left {
    pub entity: u16,
    pub item_id: i16,
    pub destroyed: bool,
}

impl Gates {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn get(&self, entity: u16) -> Option<&Gate> {
        self.items.iter().find(|g| g.entity == entity)
    }

    pub fn place(
        &mut self,
        packet: &ground_item::Drop,
        items: &ItemList,
        ground: impl Fn(f32, f32) -> f32,
        now_ms: u32,
        in_box: bool,
    ) -> (usize, bool) {
        let item_id = packet.item.index as i16;

        let mesh = items
            .get(usize::try_from(item_id).unwrap_or(usize::MAX))
            .map(|e| i32::from(e.mesh))
            .unwrap_or(0);

        let table = table(mesh);

        let (x, z) = (f32::from(packet.x) + 0.5, f32::from(packet.z) + 0.5);

        let seeded = command(state::CLOSED, i32::from(packet.state));

        let gate = Gate {
            entity: packet.entity,
            item_id,
            item: packet.item,
            cell: (packet.x, packet.z),
            position: Vec3::new(x, ground(x, z), z),
            yaw: f32::from(packet.direction) * QUARTER_TURN,
            direction: packet.direction,
            table,
            state: seeded.unwrap_or(state::CLOSED),
            since_ms: match seeded {
                Some(_) => now_ms,
                None => 0,
            },
            swing: 0.0,
            in_box,
            hovered: false,
        };

        match self.items.iter().position(|g| g.entity == packet.entity) {
            Some(at) => {
                self.items[at] = gate;

                (at, false)
            }
            None => {
                self.items.push(gate);

                (self.items.len() - 1, true)
            }
        }
    }

    pub fn remove(&mut self, entity: u16) -> bool {
        match self.items.iter().position(|g| g.entity == entity) {
            Some(at) => {
                self.items.remove(at);

                true
            }
            None => false,
        }
    }

    pub fn tick(&mut self, now_ms: u32, centre: Option<(i32, i32)>) -> Vec<Left> {
        let mut left = Vec::new();

        if let Some((cx, cz)) = centre {
            let (lo_x, hi_x) = (cx as f32 - DESPAWN_HALF_BOX, cx as f32 + DESPAWN_HALF_BOX);

            let (lo_z, hi_z) = (cz as f32 - DESPAWN_HALF_BOX, cz as f32 + DESPAWN_HALF_BOX);

            for gate in &mut self.items {
                let inside = lo_x <= gate.position.x
                    && gate.position.x <= hi_x
                    && lo_z <= gate.position.z
                    && gate.position.z <= hi_z;

                if inside {
                    continue;
                }

                let report = gate.in_box;

                gate.state = state::CLOSED;

                gate.in_box = false;

                if report || gate.item_id == BUSH {
                    left.push(Left {
                        entity: gate.entity,
                        item_id: gate.item_id,
                        destroyed: gate.item_id == BUSH,
                    });
                }
            }

            self.items.retain(|g| !(g.item_id == BUSH && !g.in_box));
        }

        for gate in &mut self.items {
            if gate.delegated() {
                continue;
            }

            let (next, angle) = advance(gate.state, now_ms.wrapping_sub(gate.since_ms));

            gate.state = next;

            if let Some(angle) = angle {
                gate.swing = angle;
            }
        }

        left
    }

    pub fn mesh_ids(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .items
            .iter()
            .flat_map(|g| poses(g).into_iter().map(|p| p.mesh).collect::<Vec<_>>())
            .filter_map(|m| u32::try_from(m).ok())
            .filter(|m| *m != 0)
            .collect();

        out.sort_unstable();

        out.dedup();

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateDraw {
    pub mesh_id: u32,
    pub world: Matrix4,
    pub material: render::world::Material,
    pub stage1_source: u32,
}

impl GateDraw {
    pub fn as_object(&self) -> super::drops::DropDraw {
        super::drops::DropDraw {
            mesh_id: self.mesh_id,
            world: self.world,
            material: self.material,
            overlay: None,
            tex_bias: 0,
            flag_colour: None,
            gate_stage1_source: Some(self.stage1_source),
        }
    }
}

pub fn draws(gates: &Gates, light: impl Fn(Vec3) -> [f32; 4]) -> Vec<GateDraw> {
    let mut out = Vec::new();

    for gate in gates.items.iter().rev() {
        let material = material(light(gate.position), gate.hovered);

        for pose in poses(gate) {
            let Ok(mesh_id) = u32::try_from(pose.mesh) else {
                continue;
            };

            if mesh_id == 0 {
                continue;
            }

            out.push(GateDraw {
                mesh_id,
                world: pose.world,
                material,
                stage1_source: gate.table.mesh_a as u32,
            });
        }
    }

    out
}

pub const CLAIM_RANGE: f32 = 2.0;

pub fn hover(
    gates: &mut Gates,
    ray: &Ray,
    player: Option<Vec3>,
    claim: Option<u16>,
) -> Option<u16> {
    let mut claim = claim;

    let mut held: Option<Vec3> = None;

    for gate in gates.items.iter_mut().rev() {
        gate.hovered = hits(ray, gate);

        if !gate.hovered {
            continue;
        }

        let Some(body) = player else {
            claim = Some(gate.entity);

            held = Some(gate.position);

            continue;
        };

        if gate.table.mesh_b == mesh::SLOT_MACHINE {
            claim = Some(gate.entity);

            held = Some(gate.position);

            continue;
        }

        if let (Some(_), Some(at)) = (claim, held) {
            let mine = (gate.position.z - body.z).abs();

            let theirs = (at.z - body.z).abs();

            if mine > theirs {
                continue;
            }
        }

        if (gate.position.x - body.x).abs() <= CLAIM_RANGE
            && (gate.position.z - body.z).abs() <= CLAIM_RANGE
        {
            claim = Some(gate.entity);

            held = Some(gate.position);
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
    pub state: i32,
    pub meshes: Vec<i32>,
    pub stamped: usize,
    pub sound: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arrival {
    Drop(super::drops::Placed),
    Gate(Placed),
}

pub fn is_gate_item(item: &Item, effects: &formats::data::item_effect::Effects<'_>) -> bool {
    effects.total_scaled(item, super::drops::GATE_EFFECT) >= 1
}

pub fn apply(
    scene: &mut super::field::FieldScene,
    game: &assets::GameDir,
    packet: &ground_item::Drop,
) -> anyhow::Result<Arrival> {
    build(scene, game, packet, PACKET_IN_BOX)
}

pub const PACKET_IN_BOX: bool = true;

pub fn build(
    scene: &mut super::field::FieldScene,
    game: &assets::GameDir,
    packet: &ground_item::Drop,
    in_box: bool,
) -> anyhow::Result<Arrival> {
    use anyhow::Context as _;

    let effects = scene.statics.effects();

    if !is_gate_item(&packet.item, &effects) {
        scene.gates.remove(packet.entity);

        return super::drops::apply(scene, game, packet).map(Arrival::Drop);
    }

    let item_id = packet.item.index as i16;

    let y = scene
        .world
        .gate_height(game, f32::from(packet.x) + 0.5, f32::from(packet.z) + 0.5)?
        .unwrap_or(0.0);

    let ground = |_: f32, _: f32| y;

    let now_ms = scene.time.now_ms;

    if DELEGATED.contains(&item_id) {
        scene.gates.remove(packet.entity);

        let (at, fresh) = scene
            .drops
            .place_at(packet, &scene.statics.items, &effects, y);

        let drop = &scene.drops.items[at];

        super::drop_presentation::bind(&mut scene.world.effects, game, &drop.item, &effects);

        let (mesh, use_kind) = (drop.mesh, drop.use_kind);

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
                .with_context(|| format!("loading the ground mesh {mesh} for item {item_id}"))?;

            scene
                .world
                .objects
                .load_texture_bias(game, mesh, drop.tex_bias);
        }

        let sound = packet
            .plays_sound()
            .then(|| super::drops::sound(item_id, use_kind));

        let yaw = scene.drops.items[at].yaw;

        let kind = effects.total_scaled(&packet.item, super::drops::GATE_EFFECT);

        stamp_placement(scene, yaw, kind, packet);

        return Ok(Arrival::Drop(super::drops::Placed {
            entity: packet.entity,
            id: item_id,
            cell: (packet.x, packet.z),
            fresh,
            mesh,
            sound,
        }));
    }

    scene.drops.remove(packet.entity);

    let (at, fresh) = scene
        .gates
        .place(packet, &scene.statics.items, ground, now_ms, in_box);

    let kind = effects.total_scaled(&packet.item, super::drops::GATE_EFFECT);

    let yaw = scene.gates.items[at].yaw;

    let stamped = stamp_placement(scene, yaw, kind, packet);

    let gate = &scene.gates.items[at];

    let (state, mesh_b) = (gate.state, gate.table.mesh_b);

    super::drop_presentation::bind(
        &mut scene.world.effects,
        game,
        &gate.item,
        &scene.statics.effects(),
    );

    if gate.table.glow.is_some() {
        scene.world.effects.bind_slot(game, 2);
    }

    let meshes: Vec<i32> = poses(gate).into_iter().map(|p| p.mesh).collect();

    let load: Vec<u32> = meshes
        .iter()
        .filter_map(|m| u32::try_from(*m).ok())
        .filter(|m| *m != 0)
        .collect();

    if !load.is_empty() {
        scene
            .world
            .objects
            .load_by_id(game, &load)
            .with_context(|| format!("loading the gate meshes {load:?} for item {item_id}"))?;
    }

    if scene
        .world
        .objects
        .uses_gate_texture_overlay(gate.table.mesh_a as u32)
    {
        scene.world.objects.load_effect_textures(game, &[6]);
    }

    let sound = command_sound(state::CLOSED, i32::from(packet.state), mesh_b);

    Ok(Arrival::Gate(Placed {
        entity: packet.entity,
        id: item_id,
        cell: (packet.x, packet.z),
        fresh,
        state,
        meshes,
        stamped,
        sound,
    }))
}

pub fn stamp_placement(
    scene: &mut super::field::FieldScene,
    yaw: f32,
    kind: i32,
    packet: &ground_item::Drop,
) -> usize {
    let facing = quadrant(yaw);

    let stamp = gate_stamp::Stamp {
        kind,
        mode: 1,
        state: i32::from(packet.state),
        at: (i32::from(packet.x), i32::from(packet.z)),
        facing,
        fill: packet.gate_tail as i8,
    };

    let zone = scene.world.actors.zone;

    gate_stamp::stamp(&mut scene.world.actors.window, zone, &stamp)
}

pub fn restamp(scene: &mut super::field::FieldScene) -> usize {
    let effects = scene.statics.effects();

    let due: Vec<gate_stamp::Stamp> = scene
        .gates
        .items
        .iter()
        .filter(|g| !g.in_box && g.item_id > 0)
        .map(|g| gate_stamp::Stamp {
            kind: effects.total_scaled(&g.item, super::drops::GATE_EFFECT),
            mode: 2,
            state: 2,
            at: (g.position.x as i32, g.position.z as i32),
            facing: quadrant(g.yaw),
            fill: gate_stamp::TICK_FILL,
        })
        .collect();

    let zone = scene.world.actors.zone;

    let window = &mut scene.world.actors.window;

    due.iter().map(|s| gate_stamp::stamp(window, zone, s)).sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commanded {
    pub entity: u16,
    pub id: i16,
    pub asked: i32,
    pub stored: Option<i32>,
    pub settled: i32,
    pub stamped: usize,
    pub sound: Option<u32>,
}

pub fn fold(state: i32) -> i32 {
    match state > 3 {
        true => state - 3,
        false => state,
    }
}

pub fn commanded(
    scene: &mut super::field::FieldScene,
    packet: &gate_command::Command,
) -> Option<Commanded> {
    let entity = packet.entity as u16;

    let at = scene.gates.items.iter().position(|g| g.entity == entity)?;

    let effects = scene.statics.effects();

    if !is_gate_item(&scene.gates.items[at].item, &effects) {
        return None;
    }

    let gate = &scene.gates.items[at];

    let synthesised = Item {
        index: gate.item.index,
        ..Item::default()
    };

    let kind = effects.total_scaled(&synthesised, super::drops::GATE_EFFECT);

    let settled = fold(gate.state);

    let asked = packet.commanded();

    let stamp = gate_stamp::Stamp {
        kind,
        mode: settled,
        state: i32::from(packet.state),
        at: (gate.position.x as i32, gate.position.z as i32),
        facing: quadrant(gate.yaw),
        fill: packet.fill,
    };

    let (id, current, mesh_b) = (gate.item_id, gate.state, gate.table.mesh_b);

    let sound = command_sound(current, asked, mesh_b);

    scene.gates.items[at].in_box = true;

    let zone = scene.world.actors.zone;

    let stamped = gate_stamp::stamp(&mut scene.world.actors.window, zone, &stamp);

    let stored = command(current, asked);

    if let Some(state) = stored {
        let gate = &mut scene.gates.items[at];

        gate.state = state;

        gate.since_ms = scene.time.now_ms;
    }

    Some(Commanded {
        entity,
        id,
        asked,
        stored,
        settled,
        stamped,
        sound,
    })
}

pub fn describe_commanded(commanded: &Commanded) -> String {
    let what = match commanded.stored {
        Some(state) => format!("is in state {state}"),
        None => "was already there, so movement refused".to_string(),
    };

    format!(
        "NWYD: 0x374 — gate {} (entity {}) {what} ({} cells stamped)",
        commanded.id, commanded.entity, commanded.stamped,
    )
}

pub fn describe(placed: &Placed) -> String {
    let what = match placed.fresh {
        true => "placed",
        false => "re-placed",
    };

    let meshes = match placed.meshes.is_empty() {
        true => "no mesh".to_string(),
        false => format!("mesh {:?}", placed.meshes),
    };

    format!(
        "NWYD: 0x26E — gate {} {what} at ({}, {}) as entity {} in state {} ({meshes}, {} cells)",
        placed.id, placed.cell.0, placed.cell.1, placed.entity, placed.state, placed.stamped,
    )
}

pub fn describe_left(left: &Left) -> String {
    match left.destroyed {
        true => {
            format!(
                "NWYD: bush {} (entity {}) left the despawn box",
                left.item_id, left.entity
            )
        }
        false => {
            let (id, entity) = (left.item_id, left.entity);

            format!("NWYD: gate {id} (entity {entity}) left the despawn box and shut")
        }
    }
}
