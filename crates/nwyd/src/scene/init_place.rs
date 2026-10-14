use super::placed::Arrival;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::data::init_item::{self, Entry};
use formats::net::account::Item;
use formats::net::ground_item;

pub const TABLE: &[u8] = b"InitItem.bin";

pub const RADIUS: i32 = 0x21;

pub const RADIUS_LATCHED: i32 = 0x14;

pub const LATCH_AT: usize = 53;

pub const SLOTS: usize = init_item::MAX_ENTRIES;

pub const ID_MIN: i16 = 1;

pub const ID_MAX: i16 = 0x38A3;

pub const CANNON_MESH: i16 = 0x647;

pub const SEEDED_STATE: u8 = 2;

pub const STAMP_FILL: u8 = 0x10;

pub const IN_BOX: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementKind {
    Gate,
    Cannon,
    Prop,
}

pub fn load(game: &GameDir) -> Result<Vec<Entry>> {
    let raw = game.read(TABLE).context("reading InitItem.bin")?;

    init_item::parse(&raw).context("parsing InitItem.bin")
}

pub fn radius(index: usize) -> i32 {
    match index < LATCH_AT {
        true => RADIUS,
        false => RADIUS_LATCHED,
    }
}

pub fn entity_of(index: usize) -> u16 {
    (init_item::ENTITY_ID_BASE as u16).wrapping_add(index as u16)
}

pub fn reaches(entry: &Entry, player: (i32, i32), radius: i32) -> bool {
    let dx = (i32::from(entry.x) - player.0).abs();

    let dz = (i32::from(entry.y) - player.1).abs();

    dx < radius - 1 && dz < radius - 1
}

pub fn item_of(entry: &Entry) -> Item {
    Item {
        index: entry.item_id as u16,
        appearance: 0,
        reserved: 0,
        effects: Default::default(),
    }
}

pub fn synthesised(entry: &Entry, index: usize) -> ground_item::Drop {
    ground_item::Drop {
        x: entry.x as u16,
        z: entry.y as u16,
        entity: entity_of(index),
        item: item_of(entry),
        direction: entry.direction as u8,
        state: SEEDED_STATE,
        gate_tail: STAMP_FILL,
        sound: 0,
    }
}

pub fn classify_placement(
    entry: &Entry,
    items: &formats::data::item_list::ItemList,
    effects: &formats::data::item_effect::Effects<'_>,
) -> Option<PlacementKind> {
    if entry.item_id < ID_MIN || entry.item_id > ID_MAX {
        return None;
    }

    let item = item_of(entry);

    if super::placed::is_gate_item(&item, effects) {
        return Some(PlacementKind::Gate);
    }

    let mesh = items
        .get(usize::from(entry.item_id as u16))
        .map_or(0, |e| e.mesh);

    match mesh == CANNON_MESH {
        true => Some(PlacementKind::Cannon),
        false => Some(PlacementKind::Prop),
    }
}

pub fn due(entries: &[Entry], player: (i32, i32), taken: impl Fn(u16) -> bool) -> Vec<usize> {
    let mut out = Vec::new();

    for (index, entry) in entries.iter().enumerate().take(SLOTS) {
        if !reaches(entry, player, radius(index)) {
            continue;
        }

        if taken(entity_of(index)) {
            continue;
        }

        if entry.item_id < ID_MIN || entry.item_id > ID_MAX {
            continue;
        }

        out.push(index);
    }

    out
}

pub fn place_one(
    scene: &mut super::field::FieldScene,
    game: &GameDir,
    entry: &Entry,
    index: usize,
) -> Result<Arrival> {
    let placement = {
        let effects = scene.statics.effects();

        classify_placement(entry, &scene.statics.items, &effects)
    };

    let packet = synthesised(entry, index);

    let arrival = super::placed::build(scene, game, &packet, IN_BOX)?;

    if placement == Some(PlacementKind::Cannon) {
        if let Some(drop) = scene
            .drops
            .items
            .iter_mut()
            .find(|d| d.entity == packet.entity)
        {
            drop.yaw = f32::from(packet.direction) * super::drops::QUARTER_TURN;
        }
    }

    Ok(arrival)
}

pub fn pass(scene: &mut super::field::FieldScene, game: &GameDir) -> Result<Vec<(usize, Arrival)>> {
    let Some(index) = scene.player else {
        return Ok(Vec::new());
    };

    let body = &scene.world.actors.actors[index];

    let player = (body.position.x as i32, body.position.z as i32);

    let entries = std::mem::take(&mut scene.init_items);

    let wanted = due(&entries, player, |entity| {
        scene.gates.get(entity).is_some() || scene.drops.get(entity).is_some()
    });

    let mut out = Vec::new();

    for at in wanted {
        match place_one(scene, game, &entries[at], at) {
            Ok(arrival) => out.push((at, arrival)),
            Err(e) => tracing::info!("NWYD: InitItem row {at} was not placed: {e:#}"),
        }
    }

    scene.init_items = entries;

    Ok(out)
}

pub fn describe(index: usize, arrival: &Arrival) -> String {
    let what = match arrival {
        Arrival::Gate(p) => super::placed::describe(p),
        Arrival::Drop(p) => super::drops::describe(p),
    };

    format!("{what} [InitItem row {index}]")
}
