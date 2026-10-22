use super::*;
use formats::net::{action::Action, attack::Attack};
use protocol::{client, transport::Connection};
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

fn connected() -> (Session, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let connection = Connection::connect(
        &address.ip().to_string(),
        address.port(),
        Duration::from_secs(2),
        123,
    )
    .unwrap();
    let (peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    (Session::connected_for_test(connection), peer)
}

fn packets(session: &mut Session, mut peer: TcpStream) -> Vec<protocol::Packet> {
    session.shutdown_transport();

    let mut wire = Vec::new();
    peer.read_to_end(&mut wire).unwrap();
    assert_eq!(&wire[..4], &[0x11, 0xF3, 0x11, 0x1F]);

    let mut decoder = protocol::codec::Decoder::new();
    decoder.feed(&wire[4..]);
    let (packets, error) = decoder.drain();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(decoder.pending(), 0);
    assert!(packets.iter().all(|packet| packet.checksum_ok));

    packets
}

fn walk(from: (i32, i32), to: (i32, i32)) -> Vec<u8> {
    client::move_to(&Action::walk(from, to, 2, b"66"))
}

#[test]
fn throttled_movement_retries_after_the_send_deadline_before_stop_and_attack() {
    let (mut session, peer) = connected();
    let first = walk((10, 10), (12, 10));
    let second = walk((12, 10), (14, 10));
    let stop = client::move_stop(client::MoveStop {
        next: (14, 10),
        current: (14, 10),
        last: (12, 10),
    });
    let attack = Attack::default().to_bytes(formats::net::attack::ATTACK);

    assert!(send_pending(vec![first], &mut session, &Clock::frozen(10_050)).is_empty());

    let expected = vec![second, stop, attack];
    let pending = send_pending(expected.clone(), &mut session, &Clock::frozen(11_002));
    assert_eq!(pending, expected);

    let pending = send_pending(pending, &mut session, &Clock::frozen(11_050));
    assert_eq!(pending, expected);
    assert!(send_pending(pending, &mut session, &Clock::frozen(11_051)).is_empty());
    assert!(send_pending(Vec::new(), &mut session, &Clock::frozen(12_052)).is_empty());

    let packets = packets(&mut session, peer);
    let opcodes: Vec<_> = packets.iter().map(|packet| packet.header.opcode).collect();
    assert_eq!(
        opcodes,
        [client::MOVE, client::MOVE, client::MOVE_STOP, 0x39D]
    );
    assert_eq!(packets[0].header.tick, 10_050);
    assert_eq!(packets[1].header.tick, 11_051);
    let action = formats::net::action::parse(&packets[1].payload).unwrap();
    assert_eq!(action.to(), (14, 10));
}

#[test]
fn throttled_attacks_are_not_retained_or_replayed() {
    let (mut session, peer) = connected();
    let attack = Attack::default().to_bytes(formats::net::attack::ATTACK);

    assert!(send_pending(vec![attack.clone()], &mut session, &Clock::frozen(10_050)).is_empty());
    assert!(send_pending(vec![attack], &mut session, &Clock::frozen(11_002)).is_empty());
    assert!(send_pending(Vec::new(), &mut session, &Clock::frozen(12_052)).is_empty());
    assert_eq!(packets(&mut session, peer).len(), 1);
}

#[test]
fn disconnected_sessions_do_not_retain_throttled_movement() {
    let (mut session, peer) = connected();

    assert!(
        send_pending(
            vec![walk((10, 10), (12, 10))],
            &mut session,
            &Clock::frozen(10_050),
        )
        .is_empty()
    );
    session.disconnect();
    assert!(
        send_pending(
            vec![walk((12, 10), (14, 10))],
            &mut session,
            &Clock::frozen(11_002),
        )
        .is_empty()
    );
    assert_eq!(packets(&mut session, peer).len(), 1);

    assert!(
        send_pending(
            vec![walk((12, 10), (14, 10))],
            &mut Session::default(),
            &Clock::frozen(12_052),
        )
        .is_empty()
    );
}
