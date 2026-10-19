use super::*;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};

struct PausedWriter {
    first: AtomicBool,
    entered: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    completed: AtomicUsize,
}

impl transport::TrafficObserver for PausedWriter {
    fn packet(&self, _: u64, outgoing: bool, _: &protocol::Packet) {
        if !outgoing {
            return;
        }
        if self.first.swap(false, Ordering::SeqCst) {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
    }

    fn event(&self, _: u64, _: &str) {}
}

fn queued_session() -> (Session, TcpStream, Arc<PausedWriter>, mpsc::SyncSender<()>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let observer = Arc::new(PausedWriter {
        first: AtomicBool::new(true),
        entered: entered_tx,
        release: Mutex::new(release_rx),
        completed: AtomicUsize::new(0),
    });
    let connection = Connection::connect_observed(
        &address.ip().to_string(),
        address.port(),
        Duration::from_secs(5),
        123,
        Some(observer.clone()),
    )
    .unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut greeting = [0; 4];
    peer.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [0x11, 0xF3, 0x11, 0x1F]);
    let mut session =
        Session::connected_for_test(connection).with_fingerprint(Some(Fingerprint::ZERO));
    session
        .send(&mut client::keepalive(), "first queued packet", 5000)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    session
        .send(&mut client::quit(), "last queued packet", 5001)
        .unwrap();
    (session, peer, observer, release_tx)
}

fn assert_queued_packets_delivered(mut peer: TcpStream) {
    let mut wire = Vec::new();
    peer.read_to_end(&mut wire).unwrap();
    let mut decoder = protocol::codec::Decoder::new();
    decoder.feed(&wire);
    let (packets, error) = decoder.drain();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(decoder.pending(), 0);
    assert_eq!(packets.len(), 2);
    assert!(packets.iter().all(|packet| packet.checksum_ok));
    assert_eq!(packets[0].header.opcode, client::KEEPALIVE);
    assert_eq!(packets[1].header.opcode, client::QUIT);
}

#[test]
fn disconnect_retains_pending_writes_until_shutdown_finishes_them() {
    let (mut session, peer, observer, release) = queued_session();
    session.disconnect();
    assert!(session.connection.is_none());
    assert_eq!(session.phase(), Phase::Offline);
    assert_eq!(session.retiring.len(), 1);
    assert!(session.drain_events().is_empty());
    assert_eq!(session.retiring.len(), 1);

    let started = Arc::new(Barrier::new(2));
    let shutting_down = started.clone();
    let shutdown = std::thread::spawn(move || {
        shutting_down.wait();
        session.shutdown_transport();
        session
    });
    started.wait();
    release.send(()).unwrap();
    let session = shutdown.join().unwrap();
    assert!(session.retiring.is_empty());
    assert_eq!(observer.completed.load(Ordering::SeqCst), 2);
    assert_queued_packets_delivered(peer);
}

#[test]
fn event_polling_prunes_finished_retired_writers() {
    let (mut session, peer, _, release) = queued_session();
    session.disconnect();
    assert_eq!(session.retiring.len(), 1);
    release.send(()).unwrap();
    session.retiring[0].close().unwrap();
    assert!(session.drain_events().is_empty());
    assert!(session.retiring.is_empty());
    assert_queued_packets_delivered(peer);
}

#[test]
fn a_new_login_waits_for_the_previous_connections_accepted_packets() {
    let (mut session, old_peer, observer, release) = queued_session();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (accepted_tx, accepted_rx) = mpsc::sync_channel(1);
    let accepting = std::thread::spawn(move || {
        accepted_tx.send(listener.accept().unwrap().0).unwrap();
    });
    let started = Arc::new(Barrier::new(2));
    let logging_in = started.clone();
    let login = std::thread::spawn(move || {
        logging_in.wait();
        session
            .login(&address, "Synthetic", "testpass", 456, 6000)
            .unwrap();
        session
    });
    started.wait();
    let early_connection = accepted_rx.recv_timeout(Duration::from_millis(100));
    release.send(()).unwrap();
    let mut session = login.join().unwrap();
    let connected_early = early_connection.is_ok();
    let new_peer = early_connection
        .unwrap_or_else(|_| accepted_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    accepting.join().unwrap();

    assert!(!connected_early);
    assert!(session.retiring.is_empty());
    assert_eq!(observer.completed.load(Ordering::SeqCst), 2);
    assert_eq!(session.phase(), Phase::Authenticating);
    session.shutdown_transport();
    assert_queued_packets_delivered(old_peer);
    drop(new_peer);
}
