use super::*;
use crate::{client, codec, transport::TrafficObserver};
use std::sync::mpsc::{self, Receiver, Sender};

enum Step {
    Accept(usize),
    Interrupt,
    Fail(io::ErrorKind),
    Zero,
}

struct Sink {
    bytes: Arc<Mutex<Vec<u8>>>,
    steps: VecDeque<Step>,
    gate: Option<(Sender<()>, Receiver<()>)>,
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some((started, resume)) = self.gate.take() {
            started.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(5)).map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "test writer was not released")
            })?;
        }
        let count = match self.steps.pop_front() {
            Some(Step::Accept(count)) => count.min(bytes.len()),
            Some(Step::Interrupt) => return Err(io::ErrorKind::Interrupted.into()),
            Some(Step::Fail(kind)) => return Err(kind.into()),
            Some(Step::Zero) => 0,
            None => bytes.len(),
        };
        self.bytes
            .lock()
            .unwrap()
            .extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct Observed {
    packets: Mutex<Vec<codec::Packet>>,
}

impl TrafficObserver for Observed {
    fn packet(&self, connection: u64, outgoing: bool, packet: &codec::Packet) {
        assert_eq!(connection, 17);
        assert!(outgoing);
        self.packets.lock().unwrap().push(packet.clone());
    }

    fn event(&self, _: u64, _: &str) {}
}

struct Harness {
    writer: Writer,
    bytes: Arc<Mutex<Vec<u8>>>,
    started: Receiver<()>,
    resume: Sender<()>,
    stopped: Receiver<()>,
    diagnostics: Arc<Diagnostics>,
}

fn harness(
    steps: impl IntoIterator<Item = Step>,
    packets: usize,
    bytes: usize,
    observer: Observer,
) -> Harness {
    let output = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started) = mpsc::channel();
    let (resume, resume_rx) = mpsc::channel();
    let (stopped_tx, stopped) = mpsc::channel();
    let diagnostics = Arc::new(Diagnostics::new(17, "synthetic".into(), observer.clone()));
    let writer = Writer::spawn(
        Sink {
            bytes: output.clone(),
            steps: steps.into_iter().collect(),
            gate: Some((started_tx, resume_rx)),
        },
        move || {
            let _ = stopped_tx.send(());
        },
        17,
        observer,
        diagnostics.clone(),
        packets,
        bytes,
    )
    .unwrap();
    Harness {
        writer,
        bytes: output,
        started,
        resume,
        stopped,
        diagnostics,
    }
}

fn packet(key: u8, tick: u32) -> Vec<u8> {
    let mut body = client::keepalive();
    codec::seal(&mut body, key, 37, tick);
    body
}

fn metadata(body: &[u8]) -> PacketMetadata {
    let mut decoder = Decoder::new();
    decoder.feed(body);
    let header = decoder.next_packet().unwrap().unwrap().header;
    PacketMetadata {
        elapsed_ms: 0,
        outgoing: true,
        opcode: header.opcode,
        size: body.len(),
        wire_tick: header.tick,
    }
}

fn submit(writer: &Writer, body: &[u8]) {
    writer
        .reserve(body.len())
        .unwrap()
        .send(body, metadata(body))
        .unwrap();
}

fn wait(receiver: &Receiver<()>) {
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn fifo_packets_survive_partial_and_interrupted_writes_without_changing_bytes() {
    let observed = Arc::new(Observed::default());
    let mut test = harness(
        [Step::Accept(3), Step::Interrupt, Step::Accept(2)],
        8,
        1024,
        Some(observed.clone()),
    );
    let first = packet(5, 100);
    let second = packet(9, 200);
    submit(&test.writer, &first);
    wait(&test.started);
    submit(&test.writer, &second);
    assert!(test.bytes.lock().unwrap().is_empty());
    test.resume.send(()).unwrap();
    test.writer.finish().unwrap();
    wait(&test.stopped);
    let expected = [first, second].concat();
    assert_eq!(*test.bytes.lock().unwrap(), expected);
    let mut decoder = Decoder::new();
    decoder.feed(&expected);
    assert_eq!(*observed.packets.lock().unwrap(), decoder.drain().0);
    let report = test.diagnostics.report("snapshot", None, None, None, false);
    assert_eq!(report.bytes_written, expected.len() as u64);
    assert_eq!(report.recent_packets.len(), 2);
    assert!(test.writer.reserve(12).is_err());
    test.writer.finish().unwrap();
}

#[test]
fn both_queue_bounds_include_inflight_and_unsubmitted_packets() {
    let body = packet(1, 10);
    for (packets, bytes) in [(2, 1024), (100, body.len() * 2)] {
        let mut test = harness([], packets, bytes, None);
        submit(&test.writer, &body);
        wait(&test.started);
        let held = test.writer.reserve(body.len()).unwrap();
        std::thread::scope(|scope| {
            let (started_tx, started) = mpsc::channel();
            let (reserved_tx, reserved) = mpsc::channel();
            let writer = &test.writer;
            let bytes = body.len();
            scope.spawn(move || {
                started_tx.send(()).unwrap();
                reserved_tx.send(writer.reserve(bytes)).unwrap();
            });
            wait(&started);
            assert!(matches!(
                reserved.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            drop(held);
            reserved
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .send(&body, metadata(&body))
                .unwrap();
        });
        test.resume.send(()).unwrap();
        test.writer.finish().unwrap();
        assert_eq!(
            *test.bytes.lock().unwrap(),
            [body.clone(), body.clone()].concat()
        );
        let state = test.writer.shared.state.lock().unwrap();
        assert_eq!((state.packets, state.bytes), (0, 0));
    }
}

#[test]
fn full_queue_waits_for_completed_writes_without_losing_or_reordering_packets() {
    let mut test = harness([], 1, 1024, None);
    let first = packet(1, 10);
    let second = packet(2, 20);
    let third = packet(3, 30);
    submit(&test.writer, &first);
    wait(&test.started);
    std::thread::scope(|scope| {
        let (started_tx, started) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        let writer = &test.writer;
        let second = &second;
        let third = &third;
        scope.spawn(move || {
            started_tx.send(()).unwrap();
            submit(writer, second);
            submit(writer, third);
            done_tx.send(()).unwrap();
        });
        wait(&started);
        assert_eq!(
            done.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        test.resume.send(()).unwrap();
        wait(&done);
    });
    test.writer.finish().unwrap();
    assert_eq!(*test.bytes.lock().unwrap(), [first, second, third].concat());
}

#[test]
fn capacity_waiters_wake_when_the_writer_fails_or_closes() {
    for failed in [false, true] {
        let steps = if failed {
            vec![Step::Fail(io::ErrorKind::BrokenPipe)]
        } else {
            Vec::new()
        };
        let mut test = harness(steps, 1, 1024, None);
        let first = packet(4, 40);
        submit(&test.writer, &first);
        wait(&test.started);
        std::thread::scope(|scope| {
            let (started_tx, started) = mpsc::channel();
            let (reserved_tx, reserved) = mpsc::channel();
            let writer = &test.writer;
            let bytes = first.len();
            scope.spawn(move || {
                started_tx.send(()).unwrap();
                reserved_tx.send(writer.reserve(bytes)).unwrap();
            });
            wait(&started);
            assert!(matches!(
                reserved.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            if failed {
                test.resume.send(()).unwrap();
            } else {
                test.writer.close();
            }
            assert_eq!(
                reserved
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .err()
                    .unwrap()
                    .kind(),
                if failed {
                    io::ErrorKind::BrokenPipe
                } else {
                    io::ErrorKind::NotConnected
                }
            );
        });
        if failed {
            assert_eq!(
                test.writer.finish().unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
            assert!(test.bytes.lock().unwrap().is_empty());
        } else {
            test.resume.send(()).unwrap();
            test.writer.finish().unwrap();
            assert_eq!(*test.bytes.lock().unwrap(), first);
        }
    }
}

#[test]
fn a_send_waiting_for_capacity_does_not_mutate_packet_or_state_when_the_writer_fails() {
    let Harness {
        writer,
        bytes,
        started,
        resume,
        stopped,
        ..
    } = harness([Step::Fail(io::ErrorKind::BrokenPipe)], 1, 1024, None);
    submit(&writer, &packet(1, 10));
    wait(&started);
    let (mut connection, _peer) = crate::transport::native_tests::connected();
    let original_writer = std::mem::replace(&mut connection.writer, writer);
    connection.schedule.install([0x31; crate::keys::TABLE]);
    connection.send_state.lock().unwrap().last_tick = 77;
    let original_schedule = connection.schedule;
    let original_rng = connection.rng;
    let sender = connection.send_state.clone();
    let (started_tx, started) = mpsc::channel();
    let (done_tx, done) = mpsc::channel();
    let sending = std::thread::spawn(move || {
        let mut body = client::keepalive();
        started_tx.send(()).unwrap();
        let result = connection.send(&mut body, 1234);
        done_tx.send((connection, body, result)).unwrap();
    });
    wait(&started);
    assert!(matches!(
        done.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(sender.lock().unwrap().last_tick, 77);
    resume.send(()).unwrap();
    let (connection, body, result) = done.recv_timeout(Duration::from_secs(5)).unwrap();
    sending.join().unwrap();
    wait(&stopped);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(body, client::keepalive());
    assert_eq!(connection.schedule, original_schedule);
    assert_eq!(connection.rng, original_rng);
    assert_eq!(connection.counters(), (0, 0));
    assert_eq!(sender.lock().unwrap().last_tick, 77);
    assert!(bytes.lock().unwrap().is_empty());
    drop(original_writer);
}

#[test]
fn uncertain_write_timeout_errors_are_terminal_without_replaying_bytes() {
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut] {
        let mut test = harness([Step::Accept(3), Step::Fail(kind)], 8, 1024, None);
        let first = packet(5, 50);
        let second = packet(6, 60);
        submit(&test.writer, &first);
        wait(&test.started);
        submit(&test.writer, &second);
        test.resume.send(()).unwrap();
        assert_eq!(test.writer.finish().unwrap_err().kind(), kind);
        assert_eq!(*test.bytes.lock().unwrap(), first[..3]);
        assert_eq!(
            test.diagnostics
                .terminal()
                .unwrap()
                .failed_write
                .unwrap()
                .bytes_written,
            3
        );
    }
}

#[test]
fn discarded_and_mismatched_reservations_release_capacity() {
    let body = packet(3, 30);
    let mut test = harness([], 1, body.len(), None);
    drop(test.writer.reserve(body.len()).unwrap());
    let reserved = test.writer.reserve(body.len()).unwrap();
    assert_eq!(
        reserved
            .send(&body[..body.len() - 1], metadata(&body))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    submit(&test.writer, &body);
    wait(&test.started);
    test.resume.send(()).unwrap();
    test.writer.finish().unwrap();
    assert_eq!(*test.bytes.lock().unwrap(), body);
}

#[test]
fn failed_partial_packet_stops_later_packets_and_preserves_failure_details() {
    let mut test = harness(
        [Step::Accept(3), Step::Fail(io::ErrorKind::BrokenPipe)],
        8,
        1024,
        None,
    );
    let first = packet(7, 70);
    let second = packet(8, 80);
    submit(&test.writer, &first);
    wait(&test.started);
    submit(&test.writer, &second);
    let held = test.writer.reserve(second.len()).unwrap();
    test.resume.send(()).unwrap();
    assert_eq!(
        test.writer.finish().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    wait(&test.stopped);
    assert_eq!(*test.bytes.lock().unwrap(), first[..3]);
    let report = test.diagnostics.terminal().unwrap();
    let failed = report.failed_write.unwrap();
    assert_eq!(failed.bytes_written, 3);
    assert_eq!(failed.packet.wire_tick, 70);
    assert_eq!(report.bytes_written, 3);
    assert_eq!(
        test.writer.reserve(second.len()).err().unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        held.send(&second, metadata(&second)).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    let state = test.writer.shared.state.lock().unwrap();
    assert_eq!((state.packets, state.bytes), (0, 0));
}

#[test]
fn zero_write_is_terminal_and_does_not_advance_to_the_next_packet() {
    let mut test = harness([Step::Zero], 8, 1024, None);
    let body = packet(2, 20);
    submit(&test.writer, &body);
    wait(&test.started);
    submit(&test.writer, &body);
    test.resume.send(()).unwrap();
    assert_eq!(
        test.writer.finish().unwrap_err().kind(),
        io::ErrorKind::WriteZero
    );
    assert!(test.bytes.lock().unwrap().is_empty());
    assert_eq!(
        test.diagnostics
            .terminal()
            .unwrap()
            .failed_write
            .unwrap()
            .bytes_written,
        0
    );
}

#[test]
fn dropping_producer_drains_accepted_packets_without_joining_the_caller() {
    let test = harness([], 8, 1024, None);
    let first = packet(4, 40);
    let second = packet(6, 60);
    submit(&test.writer, &first);
    wait(&test.started);
    submit(&test.writer, &second);
    let reserved = test.writer.reserve(second.len()).unwrap();
    let (dropped_tx, dropped) = mpsc::channel();
    let dropper = std::thread::spawn(move || {
        drop(test.writer);
        dropped_tx.send(()).unwrap();
    });
    wait(&dropped);
    assert_eq!(test.stopped.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(
        reserved
            .send(&second, metadata(&second))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotConnected
    );
    test.resume.send(()).unwrap();
    wait(&test.stopped);
    dropper.join().unwrap();
    assert_eq!(*test.bytes.lock().unwrap(), [first, second].concat());
}

#[test]
fn intentional_close_diagnostics_do_not_cancel_accepted_packets() {
    let mut test = harness([], 8, 1024, None);
    let body = packet(2, 20);
    submit(&test.writer, &body);
    wait(&test.started);
    test.diagnostics
        .report("local_close", None, None, None, true);
    test.resume.send(()).unwrap();
    test.writer.finish().unwrap();
    assert_eq!(*test.bytes.lock().unwrap(), body);
}

#[test]
fn graceful_close_deadline_stops_partial_writes_without_waiting_for_each_packet() {
    let mut test = harness([Step::Accept(3)], 8, 1024, None);
    let body = packet(9, 90);
    submit(&test.writer, &body);
    wait(&test.started);
    submit(&test.writer, &body);
    {
        let mut state = test.writer.shared.state.lock().unwrap();
        state.closing = true;
        state.closed_at = Some(Instant::now() - DRAIN_TIMEOUT - Duration::from_secs(1));
    }
    test.resume.send(()).unwrap();
    assert_eq!(
        test.writer.finish().unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(*test.bytes.lock().unwrap(), body[..3]);
    assert_eq!(
        test.diagnostics
            .terminal()
            .unwrap()
            .failed_write
            .unwrap()
            .bytes_written,
        3
    );
}

#[test]
fn close_deadline_interrupts_a_blocked_writer_once_and_releases_waiters() {
    struct BlockedWriter {
        started: Sender<()>,
        interrupted: Receiver<()>,
    }
    impl Write for BlockedWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            self.started.send(()).unwrap();
            self.interrupted
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (started_tx, started) = mpsc::channel();
    let (interrupt, interrupted) = mpsc::channel();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let shutdown_calls = calls.clone();
    let mut writer = Writer::spawn(
        BlockedWriter {
            started: started_tx,
            interrupted,
        },
        move || {
            shutdown_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            interrupt.send(()).unwrap();
        },
        17,
        None,
        Arc::new(Diagnostics::new(17, "synthetic".into(), None)),
        1,
        1024,
    )
    .unwrap();
    let body = packet(9, 90);
    submit(&writer, &body);
    wait(&started);
    assert!(writer.shared.state.lock().unwrap().closed_at.is_none());
    assert!(!writer.is_finished());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    std::thread::scope(|scope| {
        let (reserving_tx, reserving) = mpsc::channel();
        let (result_tx, result) = mpsc::channel();
        let writer = &writer;
        let bytes = body.len();
        scope.spawn(move || {
            reserving_tx.send(()).unwrap();
            result_tx.send(writer.reserve(bytes)).unwrap();
        });
        wait(&reserving);
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        writer.close();
        assert_eq!(
            result
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotConnected
        );
        writer.shared.state.lock().unwrap().closed_at =
            Some(Instant::now() - DRAIN_TIMEOUT - Duration::from_secs(1));
        writer.shared.stopped.notify_all();
    });
    assert_eq!(
        writer.finish().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    writer.shared.enforce_close_deadline();
    writer.shared.shutdown();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let state = writer.shared.state.lock().unwrap();
    assert!(state.finished);
    assert_eq!((state.packets, state.bytes), (0, 0));
}

#[test]
fn observer_panic_fails_the_queue_releases_budgets_and_shuts_down() {
    struct Panicking;
    impl TrafficObserver for Panicking {
        fn packet(&self, _: u64, _: bool, _: &codec::Packet) {
            panic!("synthetic observer failure");
        }

        fn event(&self, _: u64, _: &str) {}
    }
    let mut test = harness([], 8, 1024, Some(Arc::new(Panicking)));
    let first = packet(10, 100);
    let second = packet(11, 110);
    submit(&test.writer, &first);
    wait(&test.started);
    submit(&test.writer, &second);
    test.resume.send(()).unwrap();
    assert_eq!(
        test.writer.finish().unwrap_err().kind(),
        io::ErrorKind::Other
    );
    wait(&test.stopped);
    assert_eq!(*test.bytes.lock().unwrap(), first);
    assert_eq!(
        test.diagnostics.terminal().unwrap().operation,
        "writer_panic"
    );
    assert_eq!(
        test.writer.reserve(second.len()).err().unwrap().kind(),
        io::ErrorKind::Other
    );
    let state = test.writer.shared.state.lock().unwrap();
    assert_eq!((state.packets, state.bytes), (0, 0));
}
