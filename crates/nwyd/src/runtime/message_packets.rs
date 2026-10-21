use crate::scene::Scene;
use formats::net::{long_message::LongMessage, npc_line::NpcLine};
use protocol::server::{ChatLine, SystemMessage};

pub enum Packet {
    Message(Box<SystemMessage>),
    Chat(Box<ChatLine>),
    LongMessage(Box<LongMessage>),
    NpcLine { entity: u16, line: NpcLine },
}

pub trait Host {
    fn scene(&mut self) -> &mut Scene;

    fn launcher_login(&self) -> bool;

    fn hide_item_announcements(&self) -> bool;

    fn hide_chat_item_announcement(&self, line: &ChatLine) -> bool;

    fn channel(&self) -> i32;

    fn log_message(&mut self, record: usize);

    fn retain_memo(&mut self, body: &str);

    fn sync_ime(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    match packet {
        Packet::Message(system) => {
            let launcher = host.launcher_login();
            let hide_item_announcements = host.hide_item_announcements();

            host.log_message(system.strdef());

            if !system.argument.is_empty() {
                tracing::info!(
                    "NWYD: (code {}, argument {:?})",
                    system.code,
                    system.argument
                );
            }

            if let Some(announced) =
                host.scene()
                    .announce_for_login(&system, launcher, hide_item_announcements)
            {
                if host.scene().legacy_modal() {
                    host.sync_ime();

                    host.redraw();
                }

                if !matches!(host.scene(), Scene::Field(_))
                    && let Some(body) = &announced.memo
                {
                    host.retain_memo(body);
                }

                if announced.would_have_exited {
                    tracing::info!(
                        "NWYD: strdef {} would have closed the client with _exit(0) — \
                             refused",
                        announced.record,
                    );
                }

                if announced.world_changed
                    || !announced.posted.is_empty()
                    || !announced.banners.is_empty()
                    || announced.memo.is_some()
                    || announced.would_have_exited
                {
                    host.redraw();
                }
            }
        }
        Packet::Chat(line) => {
            let channel = host.channel();
            let hide_item_announcement = host.hide_chat_item_announcement(&line);

            match host
                .scene()
                .hear_on_channel(&line, channel, hide_item_announcement)
            {
                Some(crate::scene::chat::Heard::Dropped(why)) => {
                    tracing::info!("NWYD: {:#05x} chat not shown — {why}", line.opcode)
                }
                Some(
                    crate::scene::chat::Heard::Posted { .. } | crate::scene::chat::Heard::Memo,
                ) => {}
                Some(crate::scene::chat::Heard::Modal) => host.sync_ime(),
                None => {
                    tracing::info!(
                        "NWYD: {:#05x} chat arrived with no chat log up",
                        line.opcode,
                    )
                }
            }

            host.redraw();
        }
        Packet::LongMessage(message) => match host.scene().long_message(&message) {
            Some(applied) if applied.changed => {
                tracing::info!(
                    "NWYD: 0x7B1 — long-message panel shown in mode {}",
                    message.mode
                );

                host.redraw();
            }
            Some(_) => {
                tracing::info!(
                    "NWYD: 0x7B1 — retained event lines or unavailable field panel (mode {})",
                    message.mode
                )
            }
            None => tracing::info!("NWYD: 0x7B1 arrived with no world up"),
        },
        Packet::NpcLine { entity, line } => match host.scene().npc_line(entity, &line) {
            Some(applied) => {
                tracing::info!("{}", crate::scene::npc_line::describe(&applied));

                if applied.posted.is_some() || applied.balloon.is_some() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x107 for entity {entity} arrived with no world up")
            }
        },
    }
}
