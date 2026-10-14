use super::field::FieldScene;
use super::item_move;
use super::panels::{
    CONTENT_CARGO, CONTENT_CHARACTER, CONTENT_COIN_INPUT, CONTENT_INVENTORY, CONTENT_ITEM_MIX,
    CONTENT_ITEM_MIX4, CONTENT_MISSION_MIX, CONTENT_SKILL, CONTENT_SKILL_MASTER, CONTENT_STORE,
    Panels,
};
use super::screens;
use super::store;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Closed {
    pub mix: bool,
    pub refined_equipment: bool,
    pub mission_mix: bool,
    pub store: Option<store::Closed>,
    pub coin_input: bool,
    pub skill_master: bool,
    pub skill: bool,
    pub character: bool,
    pub cargo: bool,
    pub inventory: bool,
    pub inventory_unlocks: u8,
    pub owner_close_sent: bool,
}

impl Closed {
    pub fn changed(&self) -> bool {
        self.mix
            || self.refined_equipment
            || self.mission_mix
            || self.store.is_some()
            || self.coin_input
            || self.skill_master
            || self.skill
            || self.character
            || self.cargo
            || self.inventory
    }
}

fn close_panel(panels: &mut Panels, content: crate::frame::PanelId) -> bool {
    if !panels.is_open(content) {
        return false;
    }

    panels.set_open(content, false);

    true
}

fn unlock_inventory(panels: &mut Panels) -> bool {
    let Some(inventory) = panels.screen_mut(CONTENT_INVENTORY) else {
        return false;
    };

    for source in 0..item_move::INVENTORY_SLOTS {
        screens::inventory_lock(inventory, source, false);
    }

    true
}

pub fn accept_item_skill(scene: &mut FieldScene, at: item_move::Coord, now_ms: u32) -> bool {
    use formats::net::attack::{self as wire, Attack};

    let Some(player) = scene
        .player
        .filter(|&p| p < scene.world.actors.actors.len())
    else {
        return false;
    };

    if now_ms.wrapping_sub(scene.attack.last_attack_ms) <= 999 {
        return false;
    }

    let Some(skill) = scene
        .statics
        .skills
        .as_ref()
        .and_then(|table| table.get(83))
        .copied()
    else {
        tracing::warn!(skill = 83, "item skill requires SkillData.bin");

        return false;
    };

    if super::cast::mana_cost(scene, &skill, 83)
        > i32::try_from(scene.entered.status.mp).unwrap_or(i32::MAX)
    {
        super::cast::mana_toast(scene);

        return false;
    }

    let id = scene.world.actors.actors[player].entity;

    let here = scene.attack.from.unwrap_or((0, 0));

    let attack = Attack {
        from: here,
        to: here,
        attacker: id,
        action: 255,
        skill_id: 83,
        used: 1,
        ..Default::default()
    };

    let mut cast = attack.to_bytes(wire::ATTACK);

    cast[6..8].copy_from_slice(&id.to_le_bytes());

    let mut request = vec![0u8; 20];

    request[4..6].copy_from_slice(&0x2D4u16.to_le_bytes());

    request[6..8].copy_from_slice(&id.to_le_bytes());

    request[14..16].copy_from_slice(&(at.container as u16).to_le_bytes());

    request[16..18].copy_from_slice(&(at.slot as u16).to_le_bytes());

    scene.outbox.push(cast);

    scene.outbox.push(request);

    scene.attack.last_attack_ms = now_ms;

    for hand in &mut scene.world.actors.actors[player].swings {
        hand.emitters.panel = true;
    }

    true
}

pub fn close(scene: &mut FieldScene) -> Closed {
    let mut closed = Closed::default();

    let Some(panels) = scene.panels.as_mut() else {
        return closed;
    };

    if panels.is_open(CONTENT_ITEM_MIX) {
        let inventory_open = panels.is_open(CONTENT_INVENTORY);

        let inventory_present = panels.screen(CONTENT_INVENTORY).is_some();

        close_panel(panels, CONTENT_ITEM_MIX);

        closed.mix = true;

        closed.inventory_unlocks += u8::from(inventory_present);

        closed.inventory |= inventory_open;
    }

    let mix4_inventory = panels.is_open(CONTENT_INVENTORY);

    if close_panel(panels, CONTENT_ITEM_MIX4) {
        closed.refined_equipment = true;

        if mix4_inventory {
            closed.inventory_unlocks += 1;

            closed.inventory = true;
        }
    }

    if !panels.is_open(CONTENT_MISSION_MIX) {
        return closed;
    }

    let store_open = panels.is_open(CONTENT_STORE);

    let store_inventory = store_open && panels.is_open(CONTENT_INVENTORY);

    let store_cargo = store_open && panels.is_open(CONTENT_CARGO);

    let _ = panels;

    if store_open {
        let store = store::close(scene);

        closed.inventory |= store_inventory;

        closed.cargo |= store_cargo;

        closed.owner_close_sent = store.wire();

        if closed.owner_close_sent {
            scene.outbox.push(store::close_frame());
        }

        closed.store = Some(store);
    }

    let Some(panels) = scene.panels.as_mut() else {
        return closed;
    };

    closed.coin_input = close_panel(panels, CONTENT_COIN_INPUT);

    closed.skill_master = close_panel(panels, CONTENT_SKILL_MASTER);

    closed.skill = close_panel(panels, CONTENT_SKILL);

    closed.character = close_panel(panels, CONTENT_CHARACTER);

    if close_panel(panels, CONTENT_CARGO) {
        closed.cargo = true;

        closed.inventory |= close_panel(panels, CONTENT_INVENTORY);
    }

    closed.inventory |= close_panel(panels, CONTENT_INVENTORY);

    closed.inventory_unlocks += u8::from(unlock_inventory(panels));

    scene.mission = None;

    closed.mission_mix = close_panel(panels, CONTENT_MISSION_MIX);

    if closed.mission_mix {
        closed.inventory_unlocks += u8::from(panels.screen(CONTENT_INVENTORY).is_some());
    }

    closed
}

pub fn describe(closed: &Closed) -> String {
    format!(
        "NWYD: 0x3A7 crafting teardown — mix {}, refined_equipment {}, mission {}, store {}, owner-close {}",
        closed.mix,
        closed.refined_equipment,
        closed.mission_mix,
        closed.store.is_some(),
        closed.owner_close_sent,
    )
}
