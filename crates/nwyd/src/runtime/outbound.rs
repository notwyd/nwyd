use crate::{clock::Clock, net::Session, scene::Scene};

pub fn tick(
    scene: &mut Scene,
    session: &mut Session,
    clock: &Clock,
) -> Option<crate::scene::field::Transition> {
    let transition = scene.tick();

    if transition.is_some() {
        flush(scene, session, clock);
    }

    transition
}

pub fn flush(scene: &mut Scene, session: &mut Session, clock: &Clock) {
    if let Some((slot, position)) = scene.take_saved_position()
        && let Some(block) = session.block.as_mut()
    {
        crate::scene::city::write_back(block, slot, position);
    }

    for mut body in scene.take_pending_outbox() {
        let now = clock.wire_tick();

        let opcode = protocol::header::opcode_of(&body).unwrap_or(0);

        let what = match opcode {
            protocol::client::MOVE => "move",
            protocol::client::WHO_IS => "entity request",
            protocol::client::ENTITY_REREQUEST => "entity re-request",
            crate::scene::item_move::OPCODE_MOVE => "item move",
            crate::scene::item_move::OPCODE_COMBINE => "item combine or use",
            crate::scene::item_move::OPCODE_BUY => "shop buy",
            crate::scene::item_move::OPCODE_SELL => "shop sell",
            crate::scene::item_move::OPCODE_DELETE => "item delete",
            _ => "chat",
        };

        apply_window_state(&mut body, active_window_iconic());

        if let Err(error) = session.send_field(&mut body, what, now) {
            tracing::info!("NWYD: {opcode:#05x} {what} not sent — {error}");
        }
    }

    if let Some(limit) = session.take_chat_limit_notice()
        && let Scene::Field(scene) = scene
        && let Some(panels) = scene.panels.as_mut()
        && let Some(template) = panels.strings.strdef.as_ref().map(|s| s.get(1664))
        && let Some(notice) = formats::net::client_config::chat_limit_notice(template, limit)
    {
        crate::scene::chat::system(panels, &notice, 0xffff_ffff);
    }
}

fn apply_window_state(body: &mut [u8], iconic: bool) {
    if !iconic || protocol::header::opcode_of(body) != Some(0x2E4) {
        return;
    }

    if let Some(bytes) = body.get_mut(12..16) {
        let slot = i32::from_le_bytes(bytes.try_into().expect("four-byte slot"));

        if slot as u32 & 0x8000_0001 == 0 {
            bytes.copy_from_slice(&(slot | 0x2000).to_le_bytes());
        }
    }
}

#[cfg(windows)]

fn active_window_iconic() -> bool {
    #[link(name = "user32")]

    unsafe extern "system" {

        fn GetActiveWindow() -> *mut std::ffi::c_void;

        fn IsIconic(window: *mut std::ffi::c_void) -> i32;

    }

    unsafe { IsIconic(GetActiveWindow()) != 0 }
}

#[cfg(not(windows))]

fn active_window_iconic() -> bool {
    false
}
