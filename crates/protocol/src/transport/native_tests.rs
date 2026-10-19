use super::*;
use std::io::{Cursor, Read};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::mpsc::{self, Receiver, Sender};

const TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn connected() -> (Connection, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let connection = Connection::connect(
        "127.0.0.1",
        listener.local_addr().unwrap().port(),
        TIMEOUT,
        7,
    )
    .unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut hello = [0; HELLO.len()];
    peer.read_exact(&mut hello).unwrap();
    assert_eq!(hello, HELLO);
    assert_eq!(connection.socket.write_timeout().unwrap(), None);
    (connection, peer)
}

#[test]
fn active_sockets_keep_blocking_writes_without_an_added_timeout() {
    let (mut connection, _peer) = connected();
    assert_eq!(connection.socket.write_timeout().unwrap(), None);
    connection.close().unwrap();
}

#[test]
fn queue_saturation_waits_before_sealing_and_resumes_without_disconnect() {
    let (mut connection, mut peer) = connected();
    connection.schedule.install([0x31; keys::TABLE]);
    connection.send_state.lock().unwrap().last_tick = 77;
    let sender = connection.send_state.clone();
    let mut reservations = (0..1_024)
        .map(|_| connection.writer.reserve(crate::header::MIN_SIZE).unwrap())
        .collect::<Vec<_>>();
    let (started_tx, started) = mpsc::channel();
    let (finished_tx, finished) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut body = client::keepalive();
        started_tx.send(()).unwrap();
        let result = connection.send(&mut body, 1234);
        finished_tx.send((connection, body, result)).unwrap();
    });
    started.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        finished.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(sender.lock().unwrap().last_tick, 77);
    drop(reservations.pop());
    let (mut connection, body, result) = finished.recv_timeout(TIMEOUT).unwrap();
    result.unwrap();
    worker.join().unwrap();
    let mut expected = client::keepalive();
    codec::seal(&mut expected, !0x31, 0, 1234);
    assert_eq!(body, expected);
    assert_eq!(connection.schedule.index, 1);
    assert_eq!(connection.counters(), (1, 0));
    assert_eq!(sender.lock().unwrap().last_tick, 1234);
    assert!(connection.disconnect_report().is_none());
    let mut received = vec![0; expected.len()];
    peer.read_exact(&mut received).unwrap();
    assert_eq!(received, expected);
    drop(reservations);
    connection.close().unwrap();
}

#[test]
fn explicit_close_finishes_accepted_packets_before_returning() {
    let (mut connection, mut peer) = connected();
    let mut expected = Vec::new();
    for tick in 0..64 {
        let mut body = client::keepalive();
        connection.send(&mut body, tick).unwrap();
        expected.extend(body);
    }
    let mut quit = client::quit();
    connection.send(&mut quit, 65).unwrap();
    expected.extend(quit);
    connection.close().unwrap();
    assert_eq!(
        connection.disconnect_report().unwrap().operation,
        "local_close"
    );
    assert!(connection.is_closed());
    assert!(connection.close_finished());
    let mut received = Vec::new();
    peer.read_to_end(&mut received).unwrap();
    assert_eq!(received, expected);
    let mut later = client::keepalive();
    let original = later.clone();
    assert_eq!(
        connection.send(&mut later, 66).unwrap_err().kind(),
        std::io::ErrorKind::NotConnected
    );
    assert_eq!(later, original);
    connection.close().unwrap();
}

struct ObserveRead(Sender<u32>);

impl TrafficObserver for ObserveRead {
    fn packet(&self, _: u64, outgoing: bool, packet: &Packet) {
        assert!(!outgoing);
        self.0.send(packet.header.tick).unwrap();
    }
    fn event(&self, _: u64, _: &str) {}
}

fn bounded_reader() -> (
    Receiver<Event>,
    Receiver<u32>,
    Receiver<()>,
    std::thread::JoinHandle<()>,
) {
    let bytes = (0..3)
        .flat_map(|tick| {
            let mut body = client::keepalive();
            codec::seal(&mut body, 7, 0, tick);
            body
        })
        .collect::<Vec<_>>();
    let (tx, rx) = mpsc::sync_channel(1);
    let (observed_tx, observed) = mpsc::channel();
    let (stopped_tx, stopped) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        read_loop(
            Cursor::new(bytes),
            tx,
            1,
            Some(Arc::new(ObserveRead(observed_tx))),
            Arc::new(Diagnostics::new(1, "synthetic".into(), None)),
        );
        stopped_tx.send(()).unwrap();
    });
    (rx, observed, stopped, worker)
}

#[test]
fn bounded_receive_queue_preserves_every_message_and_orders_eof_last() {
    let (events, observed, stopped, worker) = bounded_reader();
    assert_eq!(observed.recv_timeout(TIMEOUT).unwrap(), 0);
    assert_eq!(observed.recv_timeout(TIMEOUT).unwrap(), 1);
    assert!(observed.try_recv().is_err());
    for expected in 0..3 {
        let Event::Message { tick, .. } = events.recv_timeout(TIMEOUT).unwrap() else {
            panic!("message lost before EOF")
        };
        assert_eq!(tick, expected);
    }
    assert!(matches!(
        events.recv_timeout(TIMEOUT).unwrap(),
        Event::Closed(_)
    ));
    stopped.recv_timeout(TIMEOUT).unwrap();
    worker.join().unwrap();
    assert!(matches!(
        events.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn dropping_a_full_receive_queue_unblocks_the_reader() {
    let (events, observed, stopped, worker) = bounded_reader();
    assert_eq!(observed.recv_timeout(TIMEOUT).unwrap(), 0);
    assert_eq!(observed.recv_timeout(TIMEOUT).unwrap(), 1);
    assert!(observed.try_recv().is_err());
    drop(events);
    stopped.recv_timeout(TIMEOUT).unwrap();
    worker.join().unwrap();
    assert!(matches!(
        observed.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}
