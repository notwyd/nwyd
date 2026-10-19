use super::FrameDiagnostics;
use crate::clock::{Calendar, WireTime};
use crate::net::Session;
use crate::scene::login_static::{Layout, SceneTime};
use crate::scene::{Scene, listener, server_select};
use crate::window::{Client, Settings};
use protocol::client::Fingerprint;
use std::io::{ErrorKind, Read};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

fn client(address: String) -> Client {
    let mut panels = crate::scene::login_panels::language_tests::login();
    panels.session.address = address;
    Client::new(
        Settings {
            title: "Benchmark isolation test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            renderer: None,
            overlay: false,
            pointers: crate::scene::cursor::Pointers::none(),
        },
        Scene::ServerSelect(Box::new(server_select::ServerSelect {
            layout: Layout::new(800, 600),
            time: SceneTime::pinned(2500, Calendar::GOLDEN),
            world: crate::scene::world::fog_tests::field_world(),
            panels: Box::new(panels),
        })),
    )
    .with_session(Session::default().with_fingerprint(Some(Fingerprint::ZERO)))
}

fn benchmark(client: Client) -> Client {
    client.with_frame_benchmark(0, 10, PathBuf::new())
}

fn connected_session() -> (Session, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let connection = protocol::transport::Connection::connect(
        &address.ip().to_string(),
        address.port(),
        Duration::from_secs(5),
        123,
    )
    .unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut greeting = [0; 4];
    peer.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [0x11, 0xF3, 0x11, 0x1F]);
    (Session::connected_for_test(connection), peer)
}

fn launcher_credentials() -> formats::launcher::Credentials {
    let mut command = vec![b'0'; 157];
    command[..formats::launcher::COMMAND_MARKER.len()]
        .copy_from_slice(&formats::launcher::COMMAND_MARKER);
    command[22..25].copy_from_slice(b"009");
    command[25..28].copy_from_slice(b"001");
    for (index, byte) in b"Synthetic".iter().enumerate() {
        command[34 + index * 3..37 + index * 3].copy_from_slice(format!("{byte:03}").as_bytes());
    }
    for (index, byte) in b"testpass".iter().enumerate() {
        command[109 + index * 3..112 + index * 3].copy_from_slice(format!("{byte:03}").as_bytes());
    }
    formats::launcher::parse(&command).unwrap().unwrap()
}

#[test]
fn benchmark_rejects_manual_and_launcher_logins_before_connecting() {
    for launcher in [false, true] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut client = benchmark(client(listener.local_addr().unwrap().to_string()));
        let request = if launcher {
            client = client.with_launcher(launcher_credentials());
            listener::Refusal::LauncherAccepted
        } else {
            listener::Refusal::LoginAccepted {
                account: "Synthetic".into(),
                password: "testpass".into(),
            }
        };
        client.raised(request);
        let connected = client.runtime.net.is_connected();
        client.runtime.net.shutdown_transport();
        assert!(!connected, "launcher={launcher}");
        assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
        assert!(client.picked.is_none());
    }
}

#[test]
fn connected_sessions_keep_their_live_clock_in_both_benchmark_builder_orders() {
    for session_first in [false, true] {
        let (session, _peer) = connected_session();
        let client = client(String::new());
        let mut client = if session_first {
            benchmark(client.with_session(session))
        } else {
            benchmark(client).with_session(session)
        };
        client
            .runtime
            .clock
            .synchronize(WireTime::from_millis(90_000));
        client.runtime.clock.set_offset_ms(42);
        assert!(client.validate_benchmark_session().is_err());
        client.tick();
        assert!(!client.runtime.clock.is_frozen());
        assert_eq!(client.runtime.clock.offset_ms(), 42);
        assert!(client.runtime.clock.wire_tick().wrapping_sub(90_000) < 5000);
        client.runtime.net.shutdown_transport();
    }
}

#[test]
fn ordinary_live_diagnostics_preserve_network_clock_synchronization() {
    let (session, _peer) = connected_session();
    let mut client = client(String::new()).with_session(session);
    client.diagnostics = FrameDiagnostics {
        enabled: true,
        ..Default::default()
    };
    client
        .runtime
        .clock
        .synchronize(WireTime::from_millis(90_000));
    client.runtime.clock.set_offset_ms(42);
    assert!(client.validate_benchmark_session().is_ok());
    client.tick();
    assert!(!client.runtime.clock.is_frozen());
    assert_eq!(client.runtime.clock.offset_ms(), 42);
    assert!(client.runtime.clock.wire_tick().wrapping_sub(90_000) < 5000);
    client.runtime.net.shutdown_transport();
}

#[test]
fn an_offline_benchmark_keeps_deterministic_frame_time() {
    let mut client = benchmark(client(String::new()));
    assert!(client.validate_benchmark_session().is_ok());
    client.tick();
    assert!(client.runtime.clock.is_frozen());
    assert_eq!(client.runtime.clock.wire_tick(), 10_000);
    client.frames = 3;
    client.tick();
    assert_eq!(client.runtime.clock.wire_tick(), 10_048);
}
