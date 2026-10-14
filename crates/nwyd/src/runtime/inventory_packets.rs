use super::dispatch::BatchEffects;
use crate::scene::{Scene, backpack, inventory, item_move, warehouse};
use formats::net::{
    backpack::{Arrival, Clear},
    inventory::Bag,
    warehouse::Move,
};

pub enum Packet {
    Moved { dst: (u8, u8), src: (u8, u8) },
    Inventory { entity: u16, bag: Box<Bag> },
    WarehouseGold(Move),
    Arrived(Arrival),
    Cleared(Clear),
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn redress_player(&mut self) -> Option<anyhow::Result<bool>>;

    fn refresh_screens(&mut self);

    fn load_panel_icons(&mut self);

    fn redraw(&mut self);
}

fn redress(host: &mut impl Host, effects: &mut BatchEffects, opcode: u16) {
    match host.redress_player() {
        Some(Ok(true)) => effects.request_upload(),
        Some(Ok(false)) => {
            tracing::info!(
                "NWYD: {opcode:#05x} — the player's appearance declined; keeping the standing body"
            )
        }
        Some(Err(error)) => {
            tracing::info!("NWYD: {opcode:#05x} — re-dressing failed: {error:#}")
        }
        None => {}
    }
}

pub fn apply(host: &mut impl Host, packet: Packet, effects: &mut BatchEffects) {
    match packet {
        Packet::Moved { dst, src } => match host.scene().item_moved(dst, src) {
            Some(item_move::Swapped::Moved { .. }) => {
                host.load_panel_icons();

                redress(host, effects, 0x376);

                host.refresh_screens();

                host.redraw();
            }
            Some(item_move::Swapped::Refused) => {
                tracing::info!("NWYD: 0x376 named a slot outside its container — {src:?} ⇄ {dst:?}")
            }
            None => tracing::info!("NWYD: 0x376 arrived with no world up"),
        },
        Packet::Inventory { entity, bag } => match host.scene().inventory(entity, &bag) {
            Some(applied) => {
                tracing::info!("{}", inventory::describe(&applied));

                if let Some(frame) = applied.wire() {
                    tracing::info!(
                        "NWYD: 0x185 produced a {}-byte frame and must not — D-473",
                        frame.len()
                    );
                }

                if applied.local {
                    host.load_panel_icons();

                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x185 for entity {entity} arrived with no world up")
            }
        },
        Packet::WarehouseGold(moved) => match host.scene().warehouse_gold(&moved) {
            Some(applied) => {
                tracing::info!("{}", warehouse::describe(&applied));

                host.refresh_screens();
            }
            None => {
                tracing::info!(
                    "NWYD: {:#06X} warehouse move arrived with no world up",
                    moved.change.opcode()
                )
            }
        },
        Packet::Arrived(arrival) => match host.scene().item_arrived(&arrival) {
            Some(applied) => {
                tracing::info!("{}", backpack::describe_arrival(&applied));

                host.refresh_screens();

                if matches!(applied, backpack::Arrived::Slot { .. }) {
                    host.load_panel_icons();
                }

                if matches!(
                    applied,
                    backpack::Arrived::Slot { .. } | backpack::Arrived::Gold { .. }
                ) {
                    host.redraw();
                }
            }
            None => tracing::info!("NWYD: 0x171 arrived with no world up"),
        },
        Packet::Cleared(clear) => match host.scene().item_cleared(&clear) {
            Some(applied) => {
                tracing::info!("{}", backpack::describe_clear(&applied));

                redress(host, effects, 0x175);

                if let Scene::Field(scene) = host.scene() {
                    backpack::notify_cleared(scene);
                }

                host.refresh_screens();

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x175 arrived with no world up"),
        },
    }
}
