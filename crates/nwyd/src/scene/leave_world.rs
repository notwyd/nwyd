use super::{field::FieldScene, guild, guild_panel};
use formats::net::account::{Block, Equipment, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub slot: usize,
    pub status: Status,
    pub equipment: Equipment,
    pub guild: Option<guild_panel::ConnectionReport>,
}

pub fn apply(scene: &mut FieldScene, client_id: u16, block: &mut Block) -> Option<Applied> {
    if client_id != scene.entered.client_id {
        return None;
    }

    let slot = scene.entered.slot;

    let status = scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))?
        .status;

    let equipment = scene.entered.equipment;

    let account = block.slots.get_mut(slot)?;

    tracing::debug!(
        target : "nwyd::body_state", event = "leave_world_116", entity = client_id, slot,
        live_body = scene.entered.equipment.body().index, retained_body = scene.entered
        .account_body, account_body_before = account.equipment.body().index,
        account_body_after = equipment.body().index, body_restored = equipment.body()
        .index != scene.entered.equipment.body().index, equipment = ? equipment.items
        .map(| item | item.index), "Body state written back to character selection"
    );

    let guild = guild::report_offline(scene);

    account.status = status;

    account.equipment = equipment;

    Some(Applied {
        slot,
        status,
        equipment,
        guild,
    })
}

pub fn retire_experience(scene: &FieldScene, block: &mut Block) -> Option<usize> {
    let actor = scene.world.actors.actors.get(scene.player?)?;

    let slot = block
        .slots
        .iter()
        .position(|slot| slot.name() == actor.name)?;

    block.slots[slot].exp = scene.entered.exp;

    Some(slot)
}
