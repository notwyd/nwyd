use super::*;
use crate::{
    clock::{Calendar, Clock},
    net::Transition,
    runtime::dispatch::{self, BatchEffects, DispatchHost},
    scene::{enter::Entered, field, login_static::SceneTime},
};
use assets::GameDir;
use formats::{
    config::serverlist::{Server, ServerList},
    net::enter,
};
use protocol::{ServerMessage, transport::Event};
use std::ops::ControlFlow;

struct Harness {
    scene: Scene,
    session: Session,
    retained: Retained,
    picked: Picked,
    now: u32,
    reconnects: Vec<(String, Redirect)>,
    disconnects: usize,
    notices: usize,
}

impl Harness {
    fn new(game: &GameDir) -> Self {
        let mut packet = enter::parse(&vec![0; enter::MINIMUM_LEN]).unwrap();
        packet.client_id = 37;
        packet.x = 8 * 128 + 64;
        packet.z = 13 * 128 + 64;
        let entered = Entered::from_packet(&packet, 0, None);
        let mut field = field::build(
            game,
            (800, 600),
            &SceneTime::pinned(2_000, Calendar::GOLDEN),
            &entered,
            &field::Chrome::bare(),
        )
        .unwrap();
        field.session.adopt(0, 1);
        field.redirect.arm_transfer(1_000);
        field.servers.table = ServerList {
            servers: vec![Server {
                status_url: String::new(),
                channels: vec!["first.invalid:8281".into(), "second.invalid:8281".into()],
            }],
            ..Default::default()
        };
        let scene = Scene::Field(Box::new(field));
        let mut retained = Retained::new(&scene);
        retained.name_prompt_required = true;
        Self {
            scene,
            session: Session::default(),
            retained,
            picked: Picked {
                channel: 1,
                ..Default::default()
            },
            now: 2_000,
            reconnects: Vec::new(),
            disconnects: 0,
            notices: 0,
        }
    }

    fn field(&mut self) -> &mut field::FieldScene {
        let Scene::Field(field) = &mut self.scene else {
            panic!("expected field")
        };
        field
    }
}

impl Host for Harness {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.scene,
            session: &mut self.session,
            retained: &mut self.retained,
            picked: Some(&mut self.picked),
            screens: None,
            launcher_login: false,
        }
    }

    fn game_time(&self) -> u32 {
        self.now
    }

    fn reconnect(&mut self, address: &str, ticket: &Redirect) -> Result<(), String> {
        assert_eq!(self.picked.channel, 2);
        assert_eq!(self.field().session.channel, 2);
        assert!(!self.retained.name_prompt_required);
        self.reconnects.push((address.into(), ticket.clone()));
        Ok(())
    }

    fn disconnect(&mut self) {
        self.disconnects += 1;
    }
    fn show(&mut self, _: Scene) {
        panic!("unexpected scene replacement")
    }
    fn sync_ime(&mut self) {}
    fn redraw(&mut self) {}
    fn show_message(&mut self, _: usize) {
        panic!("unexpected message")
    }
}

impl DispatchHost for Harness {
    fn receive_event(&mut self, event: Event) -> Option<Transition> {
        self.session
            .receive_event(&mut Clock::frozen(self.now), event)
    }

    fn before_transition(&mut self) {}

    fn apply_transition(&mut self, transition: Transition, _: &mut BatchEffects) -> PacketFlow {
        match transition {
            Transition::Redirect(ticket) => apply(self, Packet::Redirect(ticket)),
            Transition::Disconnected {
                reason,
                socket_closed,
            } => apply(
                self,
                Packet::Disconnected {
                    reason,
                    socket_closed,
                },
            ),
            Transition::ServerReturnNotice => {
                self.notices += 1;
                PacketFlow::Continue
            }
            _ => panic!("unexpected transition"),
        }
    }

    fn upload(&mut self) {
        panic!("unexpected upload")
    }
}

fn message(message: ServerMessage) -> Event {
    Event::Message {
        message: Box::new(message),
        tick: 2_000,
    }
}

#[test]
fn a_redirect_keeps_processing_the_old_socket_until_its_delayed_close() {
    let Some(path) = std::env::var_os("WYD_GAME_DIR") else {
        return;
    };
    let mut host = Harness::new(&GameDir::open(path).unwrap());
    let ticket = Redirect {
        client_id: 37,
        payload: [0; 68],
        account: "TRANSFER".into(),
        token: b"*2".to_vec(),
    };

    assert_eq!(
        dispatch::drain(
            &mut host,
            [
                message(ServerMessage::Redirect(ticket.clone())),
                message(ServerMessage::ServerReturnNotice),
            ]
        ),
        ControlFlow::Continue(())
    );
    assert!(host.reconnects.is_empty());
    assert_eq!(host.disconnects, 0);
    assert_eq!(
        host.notices, 1,
        "the ticket must not retire this read batch"
    );
    assert_eq!(host.picked.channel, 1);
    assert!(host.retained.name_prompt_required);

    host.now = 6_000;
    assert_eq!(
        dispatch::drain(&mut host, [message(ServerMessage::ServerReturnNotice)]),
        ControlFlow::Continue(())
    );
    assert!(host.reconnects.is_empty());
    assert_eq!(host.notices, 2);

    host.now = 10_000;
    assert_eq!(
        dispatch::drain(
            &mut host,
            [
                Event::Closed(None),
                message(ServerMessage::ServerReturnNotice),
                Event::Closed(None),
            ]
        ),
        ControlFlow::Continue(())
    );
    assert_eq!(host.reconnects, [("second.invalid:8281".into(), ticket)]);
    assert_eq!(host.disconnects, 0);
    assert_eq!(host.notices, 2, "the close must retire the old read batch");
    assert!(
        host.scene.redirect_after_disconnect(true, 10_001).is_none(),
        "the ticket is consumed exactly once"
    );
}
