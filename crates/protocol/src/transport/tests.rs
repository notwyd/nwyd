use super::*;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener};
use std::time::Instant;

const TIMEOUT: Duration = Duration::from_secs(3);

fn listener() -> TcpListener {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap()
}

fn accept(listener: &TcpListener) -> TcpStream {
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(TIMEOUT)).unwrap();
    peer.set_write_timeout(Some(TIMEOUT)).unwrap();
    let mut greeting = [0; HELLO.len()];
    peer.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, HELLO);
    peer
}

fn connected(seed: u32) -> (Connection, TcpStream) {
    let listener = listener();
    let connection = Connection::connect(
        "127.0.0.1",
        listener.local_addr().unwrap().port(),
        TIMEOUT,
        seed,
    )
    .unwrap();
    (connection, accept(&listener))
}

fn read_frame(peer: &mut TcpStream) -> Vec<u8> {
    let mut header = [0; crate::header::LEN];
    peer.read_exact(&mut header).unwrap();
    let size = usize::from(u16::from_le_bytes([header[0], header[1]]));
    assert!((crate::header::MIN_SIZE..=crate::header::MAX_SIZE).contains(&size));
    let mut wire = vec![0; size];
    wire[..header.len()].copy_from_slice(&header);
    peer.read_exact(&mut wire[header.len()..]).unwrap();
    wire
}

fn decode(wire: &[u8]) -> Packet {
    let mut decoder = Decoder::new();
    decoder.feed(wire);
    let packet = decoder.next_packet().unwrap().unwrap();
    assert!(packet.checksum_ok);
    assert_eq!(decoder.pending(), 0);
    packet
}

fn framed(opcode: u16, payload: &[u8], tick: u32) -> Vec<u8> {
    let mut frame = vec![0; crate::header::LEN + payload.len()];
    crate::header::set_opcode(&mut frame, opcode);
    frame[crate::header::LEN..].copy_from_slice(payload);
    codec::seal(&mut frame, 0x2a, 0, tick);
    frame
}

fn accepted(keys: [u8; keys::TABLE]) -> formats::net::account::Accept {
    use formats::net::account;
    let size =
        account::KEY_LEN + 4 + account::BLOCK_LEN + account::CARGO_LEN + 4 + account::NAME_LEN;
    let mut accept = account::parse_accept(&vec![0; size]).unwrap();
    accept.key = keys;
    accept
}

fn entered(client_id: u16) -> formats::net::enter::EnterWorld {
    let mut world = formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    world.client_id = client_id;
    world
}

fn drain_until(connection: &mut Connection, count: usize) -> Vec<Event> {
    let deadline = Instant::now() + TIMEOUT;
    let mut events = Vec::new();
    while events.len() < count {
        events.extend(connection.drain_events());
        assert!(
            Instant::now() < deadline,
            "received {} of {count} events",
            events.len()
        );
        if events.len() < count {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    events
}

fn wait_closed(connection: &Connection) {
    let deadline = Instant::now() + TIMEOUT;
    while !connection.is_closed() {
        assert!(Instant::now() < deadline, "connection did not close");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn sends_preserve_the_bytes_sealed_before_later_state_and_buffer_changes() {
    let (mut connection, mut peer) = connected(7);
    connection.schedule.install([0x31; keys::TABLE]);
    connection.client_id = 41;
    let mut first = client::keepalive();
    let mut expected_first = first.clone();
    codec::seal(&mut expected_first, !0x31, 41, 1234);
    connection.send(&mut first, 1234).unwrap();
    assert_eq!(first, expected_first);
    assert_eq!(connection.send_state.lock().unwrap().last_tick, 1234);

    first.fill(0xa5);
    connection.schedule.install([0x72; keys::TABLE]);
    connection.client_id = 73;
    let mut second = client::keepalive();
    let mut expected_second = second.clone();
    codec::seal(&mut expected_second, !0x72, 73, 5678);
    connection.send(&mut second, 5678).unwrap();
    second.fill(0x5a);

    assert_eq!(read_frame(&mut peer), expected_first);
    assert_eq!(read_frame(&mut peer), expected_second);
    assert_eq!(connection.counters(), (2, 0));
}

#[test]
fn draining_defers_transport_state_until_each_message_is_applied_before_its_reply() {
    let (mut connection, mut peer) = connected(9);
    let keys = std::array::from_fn(|i| i as u8 + 1);
    let mut incoming = framed(server::LOGIN_ACCEPTED, &accepted(keys).to_bytes(), 7000);
    incoming.extend(framed(server::ENTER_WORLD, &entered(37).to_bytes(), 8000));
    peer.write_all(&incoming).unwrap();
    let events = drain_until(&mut connection, 2);
    assert_eq!(events.len(), 2);
    assert!(!connection.schedule_is_live());
    assert_eq!(connection.client_id(), 0);
    assert_eq!(connection.counters(), (0, 2));

    for (index, event) in events.into_iter().enumerate() {
        let Event::Message { message, tick } = event else {
            panic!("expected ordered handshake message");
        };
        assert_eq!(tick, [7000, 8000][index]);
        connection.apply_message_state(&message);
        connection.keepalive(tick + 1).unwrap();
    }

    let first = decode(&read_frame(&mut peer));
    let second = decode(&read_frame(&mut peer));
    assert_eq!(
        (first.header.client_id, first.header.key, first.header.tick),
        (0, !keys[0], 7001)
    );
    assert_eq!(
        (
            second.header.client_id,
            second.header.key,
            second.header.tick
        ),
        (37, !keys[1], 8001)
    );
    assert_eq!(connection.client_id(), 37);
    assert_eq!(connection.schedule.index, 2);
}

#[test]
fn reconnect_retains_session_state_and_discards_events_from_the_old_link() {
    let (mut connection, mut old_peer) = connected(11);
    let keys = std::array::from_fn(|i| i as u8 + 1);
    connection.schedule.install(keys);
    connection.set_dynamic_key(0x0102_0304);
    connection.client_id = 37;
    let sender = connection.send_state.clone();
    connection.keepalive(10).unwrap();
    assert_eq!(decode(&read_frame(&mut old_peer)).header.key, !keys[0]);
    old_peer.write_all(&framed(0x4241, &[1], 100)).unwrap();
    old_peer.shutdown(Shutdown::Write).unwrap();
    wait_closed(&connection);
    let first_link = connection.id();

    let listener = listener();
    connection
        .reconnect("127.0.0.1", listener.local_addr().unwrap().port(), TIMEOUT)
        .unwrap();
    let mut new_peer = accept(&listener);
    assert_ne!(connection.id(), first_link);
    assert!(!connection.is_closed());
    assert_eq!(connection.client_id(), 37);
    assert_eq!(connection.schedule.index, 1);
    assert_eq!(connection.schedule.dynamic, 0x0102_0304);
    assert!(Arc::ptr_eq(&connection.send_state, &sender));
    assert_eq!(connection.counters(), (1, 0));

    connection
        .send_with_client_id(&mut client::keepalive(), 0, 11)
        .unwrap();
    connection.keepalive(12).unwrap();
    assert_eq!(connection.client_id(), 37);
    let redirect = decode(&read_frame(&mut new_peer));
    let gameplay = decode(&read_frame(&mut new_peer));
    assert_eq!(
        (
            redirect.header.client_id,
            redirect.header.key,
            redirect.header.tick
        ),
        (0, !keys[1], 11)
    );
    assert_eq!(
        (
            gameplay.header.client_id,
            gameplay.header.key,
            gameplay.header.tick
        ),
        (37, !keys[2], 12)
    );

    new_peer.write_all(&framed(0x4242, &[2], 200)).unwrap();
    let events = drain_until(&mut connection, 1);
    assert!(
        matches!(events.as_slice(), [Event::Message { message, tick: 200 }] if message.opcode() == 0x4242)
    );
    assert_eq!(connection.counters(), (3, 1));
}

#[test]
fn reconnect_continues_the_unscheduled_rng_sequence() {
    let seed = 0x1234_5678;
    let (mut connection, mut first_peer) = connected(seed);
    connection.keepalive(1).unwrap();
    let first = decode(&read_frame(&mut first_peer));
    let listener = listener();
    connection
        .reconnect("127.0.0.1", listener.local_addr().unwrap().port(), TIMEOUT)
        .unwrap();
    let mut second_peer = accept(&listener);
    connection.keepalive(2).unwrap();
    let second = decode(&read_frame(&mut second_peer));
    let mut rng = Lcg::new(seed);
    assert_eq!(first.header.key, rng.next() as u8);
    assert_eq!(second.header.key, rng.next() as u8);
    assert_eq!((first.header.tick, second.header.tick), (1, 2));
    assert!(!connection.schedule_is_live());
}

#[test]
fn reconnect_opens_the_new_link_after_the_old_writer_failed() {
    let (mut connection, mut old_peer) = connected(12);
    let keys = std::array::from_fn(|i| i as u8 + 1);
    connection.schedule.install(keys);
    connection.client_id = 37;
    connection.set_dynamic_key(0x0102_0304);
    connection.socket.shutdown(Shutdown::Write).unwrap();
    connection.keepalive(100).unwrap();
    wait_closed(&connection);
    let failed_link = connection.id();
    let report = connection.disconnect_report().unwrap();
    assert_eq!(report.operation, "write");
    assert_eq!(report.failed_write.unwrap().packet.wire_tick, 100);
    assert_eq!(connection.schedule.index, 1);
    let mut trailing = Vec::new();
    old_peer.read_to_end(&mut trailing).unwrap();
    assert!(trailing.is_empty());

    let listener = listener();
    connection
        .reconnect("127.0.0.1", listener.local_addr().unwrap().port(), TIMEOUT)
        .unwrap();
    let mut peer = accept(&listener);
    assert_ne!(connection.id(), failed_link);
    assert!(!connection.is_closed());
    assert_eq!(connection.schedule.index, 1);
    assert_eq!(connection.schedule.dynamic, 0x0102_0304);
    assert_eq!(connection.client_id(), 37);
    assert!(connection.drain_events().is_empty());
    connection.keepalive(200).unwrap();
    let packet = decode(&read_frame(&mut peer));
    assert_eq!(packet.header.key, !keys[1]);
    assert_eq!(packet.header.client_id, 37);
    assert_eq!(packet.header.tick, 200);
}

#[test]
fn failed_reconnect_closes_the_old_link_without_resetting_or_consuming_key_state() {
    let (mut connection, mut peer) = connected(13);
    connection.schedule.install([1; keys::TABLE]);
    connection.schedule.index = keys::TABLE;
    connection.set_dynamic_key(0x0102_0304);
    connection.client_id = 37;
    connection.keepalive(1).unwrap();
    let packet = decode(&read_frame(&mut peer));
    assert_eq!((packet.header.key, packet.header.client_id), (8, 37));
    let schedule = connection.schedule;
    let counters = connection.counters();
    assert!(connection.reconnect("127.0.0.1", 0, TIMEOUT).is_err());
    assert!(connection.is_closed());
    let mut rejected = client::keepalive();
    let original = rejected.clone();
    assert!(connection.send(&mut rejected, 2).is_err());
    assert_eq!(rejected, original);
    assert_eq!(connection.schedule, schedule);
    assert_eq!(connection.counters(), counters);
    assert_eq!(connection.client_id(), 37);
    assert_eq!(connection.send_state.lock().unwrap().last_tick, 1);
    let mut trailing = Vec::new();
    peer.read_to_end(&mut trailing).unwrap();
    assert!(trailing.is_empty());
}

#[test]
fn immediate_drop_delivers_every_accepted_packet_and_the_final_quit_in_order() {
    let (mut connection, mut peer) = connected(17);
    for tick in 1..=64 {
        connection.keepalive(tick).unwrap();
    }
    connection.quit(65).unwrap();
    drop(connection);
    let mut wire = Vec::new();
    peer.read_to_end(&mut wire).unwrap();
    let mut decoder = Decoder::new();
    decoder.feed(&wire);
    let (packets, error) = decoder.drain();
    assert!(error.is_none());
    assert_eq!(decoder.pending(), 0);
    assert_eq!(packets.len(), 65);
    for (index, packet) in packets.iter().enumerate() {
        assert!(packet.checksum_ok);
        assert_eq!(packet.header.tick, index as u32 + 1);
        assert_eq!(
            packet.header.opcode,
            if index == 64 {
                client::QUIT
            } else {
                client::KEEPALIVE
            }
        );
    }
}

#[test]
fn malformed_frames_recover_and_eof_follows_prior_messages_exactly_once() {
    let (mut connection, mut peer) = connected(19);
    let mut invalid = [0; crate::header::LEN];
    invalid[..2].copy_from_slice(&1u16.to_le_bytes());
    peer.write_all(&invalid).unwrap();
    assert!(matches!(
        drain_until(&mut connection, 1).as_slice(),
        [Event::Decode(DecodeError::BadSize(1))]
    ));
    assert!(!connection.is_closed());
    peer.write_all(&framed(0x4242, &[1, 2, 3], 123)).unwrap();
    peer.shutdown(Shutdown::Write).unwrap();
    let events = drain_until(&mut connection, 2);
    assert!(
        matches!(events.as_slice(), [Event::Message { message, tick: 123 }, Event::Closed(_)] if message.opcode() == 0x4242)
    );
    assert!(connection.is_closed());
    assert!(connection.drain_events().is_empty());
    assert!(connection.drain_events().is_empty());
    assert_eq!(connection.counters(), (0, 1));
}
