use super::field::FieldScene;
use super::screens;
use super::trade::Closing;
use formats::net::inventory::Bag;

pub fn open(scene: &mut FieldScene, partner: crate::frame::PanelId, block_equipment: bool) -> bool {
    use super::panels::CONTENT_INVENTORY;

    if scene
        .panels
        .as_ref()
        .and_then(|p| p.screen(CONTENT_INVENTORY))
        .is_none()
    {
        return false;
    }

    if super::store::close(scene).wire() {
        scene.outbox.push(super::store::close_frame());
    }

    if let Some(panels) = scene.panels.as_mut() {
        panels.pair_inventory(0, false);

        panels.set_open(CONTENT_INVENTORY, true);

        panels.pair_inventory(partner, block_equipment);
    }

    super::trade::finish_panel_close(scene);

    super::store::finish_panel_close(scene);

    true
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Replaced {
    pub entity: u16,
    pub local: bool,
    pub filled: usize,
    pub gold: u32,
    pub trade: Option<Closing>,
    pub repainted: bool,
}

impl Replaced {
    pub fn wire(&self) -> Option<&Vec<u8>> {
        self.trade
            .as_ref()
            .and_then(|closing| closing.frame.as_ref())
    }
}

pub const PARTNER_CLEAR: u32 = 0x0054_7058;

pub const READY_CLEAR: u32 = 0x0054_7065;

pub const TRADE_CLOSE_CALL: u32 = 0x0054_7086;

pub fn replace(scene: &mut FieldScene, entity: u16, bag: &Bag) -> Replaced {
    super::trade::finish_panel_close(scene);

    let index = scene.world.actors.entity(entity);

    let local = index.is_some() && index == scene.player;

    let refused = Replaced {
        entity,
        local,
        filled: 0,
        gold: 0,
        trade: None,
        repainted: false,
    };

    if !local {
        return refused;
    }

    scene.entered.carry = bag.carry;

    scene.entered.gold = bag.gold;

    scene.trade.mine.partner = 0;

    scene.trade.mine.ready = 0;

    let closing = super::trade::close(scene);

    let repainted = match &mut *scene {
        FieldScene {
            panels: Some(panels),
            entered,
            ..
        } => {
            screens::inventory_broadcast(panels, entered);

            true
        }
        _ => false,
    };

    Replaced {
        filled: bag.filled(),
        gold: bag.gold,
        trade: Some(closing),
        repainted,
        ..refused
    }
}

pub fn describe(applied: &Replaced) -> String {
    if !applied.local {
        return format!(
            "NWYD: 0x185 for entity {} is not the local player",
            applied.entity
        );
    }

    let trade = match &applied.trade {
        Some(closing) if closing.was_open => " and the trade window came down",
        _ => "",
    };

    format!(
        "NWYD: 0x185 — the bag is now {} slot(s) and {} gold{trade}",
        applied.filled, applied.gold,
    )
}
