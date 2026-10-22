#![cfg(not(target_arch = "wasm32"))]

use super::*;
use protocol::send_state::Profile;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener, TcpStream};

fn accept(listener: &TcpListener) -> TcpStream {
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut greeting = [0; 4];
    peer.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [0x11, 0xF3, 0x11, 0x1F]);
    peer
}

fn receive(peer: &mut TcpStream) -> protocol::Packet {
    let mut header = [0; protocol::header::LEN];
    peer.read_exact(&mut header).unwrap();
    let size = usize::from(u16::from_le_bytes([header[0], header[1]]));
    assert!((protocol::header::LEN..=10240).contains(&size));
    let mut wire = vec![0; size];
    wire[..header.len()].copy_from_slice(&header);
    peer.read_exact(&mut wire[header.len()..]).unwrap();
    let mut decoder = protocol::codec::Decoder::new();
    decoder.feed(&wire);
    let packet = decoder.next_packet().unwrap().unwrap();
    assert!(packet.checksum_ok);
    assert_eq!(decoder.pending(), 0);
    packet
}

fn assert_login(packet: &protocol::Packet, version: i32, tick: u32, account: &[u8]) {
    assert_eq!(packet.header.opcode, client::LOGIN_MANUAL);
    assert_eq!(packet.header.client_id, 0);
    assert_eq!(packet.header.tick, tick);
    let version_at = client::VERSION_OFF - protocol::header::LEN;
    assert_eq!(
        &packet.payload[version_at..version_at + 4],
        &version.to_le_bytes(),
    );
    let account_at = client::ACCOUNT_OFF - protocol::header::LEN;
    assert_eq!(
        &packet.payload[account_at..account_at + account.len()],
        account,
    );
    assert_eq!(packet.payload[account_at + account.len()], 0);
}

#[test]
fn profile_version_and_watermark_survive_redirect_and_a_new_login() {
    for (profile, version, login_tick, field_tick, redirect_tick, next_login_tick) in [
        (Profile::BuildC9D8, 0x0C9D_8301, 5000, 25003, 30001, 40001),
        (Profile::BuildC9D9, 0x0C9D_9301, 5000, 25002, 30000, 40000),
        (Profile::BuildC9DA, 0x0C9D_A301, 5001, 25003, 30000, 40001),
    ] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let mut session = Session::new(Verbosity::Quiet)
            .with_profile(profile)
            .with_fingerprint(Some(Fingerprint::ZERO));
        assert_eq!(session.version(), version);
        session
            .login(&address, "Player", "testpass", 123, 5000)
            .unwrap();
        let mut initial = accept(&listener);
        assert_login(&receive(&mut initial), version, login_tick, b"Player");
        session
            .send(&mut client::keepalive(), "profile probe", 25002)
            .unwrap();
        let packet = receive(&mut initial);
        assert_eq!(packet.header.opcode, client::KEEPALIVE);
        assert_eq!(packet.header.tick, field_tick);

        let redirect = server::Redirect {
            client_id: 37,
            payload: [0; 68],
            account: "Other".into(),
            token: b"*2".to_vec(),
        };
        session.redirect(&address, &redirect, 30000).unwrap();
        let mut redirected = accept(&listener);
        let packet = receive(&mut redirected);
        assert_login(&packet, version, redirect_tick, b"Other");
        let token_at = client::EXTRA_OFF - protocol::header::LEN;
        assert_eq!(&packet.payload[token_at..token_at + 3], b"*2\0");
        session
            .send(&mut client::keepalive(), "redirect profile probe", 30002)
            .unwrap();
        let packet = receive(&mut redirected);
        assert_eq!(packet.header.opcode, client::KEEPALIVE);
        assert_eq!(packet.header.tick, 30003);

        session.disconnect();
        session
            .login(&address, "Third", "testpass", 456, 40000)
            .unwrap();
        let mut next = accept(&listener);
        assert_login(&receive(&mut next), version, next_login_tick, b"Third");
        assert_eq!(session.version(), version);
        session.disconnect();
    }
}
