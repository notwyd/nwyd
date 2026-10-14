use super::{
    create::{self, NameReject, Reserved},
    field::FieldScene,
    item_move::Coord,
    panels::{self, CONTENT_COIN_INPUT},
    screens,
};
use formats::text::codepage::{self, Codepage};

pub fn checked_name(text: &str, reserved: &Reserved) -> Result<Vec<u8>, NameReject> {
    let mut bytes = codepage::encode(text, Codepage::Windows1252);

    if bytes.len() < 4 {
        return Err(NameReject::TooShort);
    }

    if bytes.len() > 12 {
        return Err(NameReject::TooLong);
    }

    if !create::charset_ok(&bytes) || !create::printable_ok(&bytes, Codepage::Windows1252) {
        return Err(NameReject::Invalid);
    }

    reserved.substitute(&mut bytes, Codepage::Windows1252);

    if bytes.len() > 17 || bytes.windows(2).any(|pair| pair == [0xa1, 0xa1]) {
        return Err(NameReject::Invalid);
    }

    Ok(bytes)
}

pub fn packet(client: u16, slot: i32, at: (u16, u16), name: &[u8]) -> Option<Vec<u8>> {
    if name.len() > 17 || name.contains(&0) {
        return None;
    }

    let mut out = vec![0; 52];

    out[4..6].copy_from_slice(&0x3ccu16.to_le_bytes());

    out[6..8].copy_from_slice(&client.to_le_bytes());

    out[12..16].copy_from_slice(&1i32.to_le_bytes());

    out[16..20].copy_from_slice(&slot.to_le_bytes());

    out[28..30].copy_from_slice(&at.0.to_le_bytes());

    out[30..32].copy_from_slice(&at.1.to_le_bytes());

    out[34..34 + name.len()].copy_from_slice(name);

    Some(out)
}

pub fn drain_open(scene: &mut FieldScene) {
    let Some(p) = scene.panels.as_mut() else {
        return;
    };

    let Some(slot) = p.seal_open.take() else {
        return;
    };

    let at = Coord { container: 1, slot };

    if super::item_move::item_at(&scene.entered, at).is_none_or(|i| i.is_empty())
        || p.cooldowns.running(0, scene.time.now_ms)
    {
        return;
    }

    p.seal_name_slot = Some(slot);

    if p.screen(CONTENT_COIN_INPUT).is_none() {
        return;
    }

    p.set_open(CONTENT_COIN_INPUT, true);

    p.split_stack = None;

    p.store_price = None;

    p.coin_action = 6;

    let body = p.strings.translate("ss_349");

    screens::coin_input_fill(p.screen_mut(CONTENT_COIN_INPUT).unwrap(), &body);
}

pub fn answer(scene: &mut FieldScene, yes: bool) {
    let Some(p) = scene.panels.as_mut() else {
        return;
    };

    if !p.is_open(CONTENT_COIN_INPUT) {
        return;
    }

    let Some(slot) = p.seal_name_slot else {
        return;
    };

    if !yes {
        p.set_open(CONTENT_COIN_INPUT, false);

        return;
    }

    let Some(body) = scene.player.and_then(|i| scene.world.actors.actors.get(i)) else {
        return;
    };

    if body.is_dead {
        return;
    }

    let Some(text) = p.coin_input_text().map(str::to_owned) else {
        return;
    };

    if text.is_empty() || text == "all" {
        return;
    }

    let tile = (body.position.x as i32 as u16, body.position.z as i32 as u16);

    match checked_name(&text, &p.reserved) {
        Err(reason) => {
            let text = p.strings.translate(&format!("ss_{}", reason.strdef()));

            super::notice::post(p, &text, 2000, true, scene.time.now_ms);
        }
        Ok(name) => {
            if let Some(frame) = packet(scene.entered.client_id, slot, tile, &name) {
                scene.outbox.push(frame);

                p.cooldowns.stamp(0, scene.time.now_ms);

                screens::refresh_all(p, &scene.entered);

                if let Ok(slot) = usize::try_from(slot)
                    && let Some(item) = scene.entered.carry.get_mut(slot)
                {
                    *item = Default::default();
                }

                panels::refresh_inventory_hover(p, &scene.entered, slot);

                panels::refine_item_changed(p, &scene.entered);
            }
        }
    }

    p.set_open(CONTENT_COIN_INPUT, false);
}
