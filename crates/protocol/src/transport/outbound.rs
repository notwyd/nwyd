use super::{Diagnostics, FailedWrite, Observer, PacketMetadata, write_counted};
use crate::codec::Decoder;
use std::{
    collections::VecDeque,
    io::{self, Write},
    net::{Shutdown, TcpStream},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_PACKETS: usize = 1_024;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

struct Job {
    body: Vec<u8>,
    metadata: PacketMetadata,
}

#[derive(Clone)]
struct Failure {
    kind: io::ErrorKind,
    message: String,
}

impl Failure {
    fn error(&self) -> io::Error {
        io::Error::new(self.kind, self.message.clone())
    }
}

#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    packets: usize,
    bytes: usize,
    closing: bool,
    closed_at: Option<Instant>,
    finished: bool,
    failure: Option<Failure>,
}

impl State {
    fn available(&self) -> io::Result<()> {
        if let Some(failure) = &self.failure {
            return Err(failure.error());
        }
        if self.closing {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "outbound transport is closing",
            ));
        }
        Ok(())
    }

    fn release(&mut self, bytes: usize) {
        self.packets -= 1;
        self.bytes -= bytes;
    }
}

struct Shared {
    state: Mutex<State>,
    ready: Condvar,
    space: Condvar,
    stopped: Condvar,
    shutdown: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    max_packets: usize,
    max_bytes: usize,
}

pub(super) struct Writer {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

pub(super) struct Reservation {
    shared: Arc<Shared>,
    bytes: Option<usize>,
}

impl Writer {
    pub(super) fn new(
        socket: TcpStream,
        id: u64,
        observer: Observer,
        diagnostics: Arc<Diagnostics>,
    ) -> io::Result<Self> {
        let shutdown = socket.try_clone()?;
        Self::spawn(
            socket,
            move || {
                let _ = shutdown.shutdown(Shutdown::Both);
            },
            id,
            observer,
            diagnostics,
            MAX_PACKETS,
            MAX_BYTES,
        )
    }

    fn spawn(
        writer: impl Write + Send + 'static,
        shutdown: impl FnOnce() + Send + 'static,
        id: u64,
        observer: Observer,
        diagnostics: Arc<Diagnostics>,
        max_packets: usize,
        max_bytes: usize,
    ) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            ready: Condvar::new(),
            space: Condvar::new(),
            stopped: Condvar::new(),
            shutdown: Mutex::new(Some(Box::new(shutdown))),
            max_packets,
            max_bytes,
        });
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name(format!("nwyd-write {id}"))
            .spawn(move || {
                let _shutdown = WorkerShutdown {
                    shared: worker_shared.clone(),
                };
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(writer, &worker_shared, id, observer, &diagnostics)
                }))
                .unwrap_or_else(|_| {
                    let error = io::Error::other("outbound worker panicked");
                    fail_queue(&worker_shared, &error);
                    diagnostics.report("writer_panic", Some(&error), None, None, true);
                    Err(error)
                })
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub(super) fn reserve(&self, bytes: usize) -> io::Result<Reservation> {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.available()?;
        if bytes == 0 || bytes > self.shared.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid outbound reservation size",
            ));
        }
        while state.packets >= self.shared.max_packets
            || bytes > self.shared.max_bytes.saturating_sub(state.bytes)
        {
            state = self
                .shared
                .space
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
            state.available()?;
        }
        state.packets += 1;
        state.bytes += bytes;
        Ok(Reservation {
            shared: self.shared.clone(),
            bytes: Some(bytes),
        })
    }

    pub(super) fn close(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.closing = true;
        let watch = state.closed_at.is_none() && !state.finished;
        state.closed_at.get_or_insert_with(Instant::now);
        drop(state);
        self.shared.ready.notify_one();
        self.shared.space.notify_all();
        if watch {
            let shared = self.shared.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("nwyd-net-close".into())
                .spawn(move || shared.enforce_close_deadline())
            {
                tracing::warn!(%error, "could not start connection shutdown deadline");
                self.shared.shutdown();
            }
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub(super) fn finish(&mut self) -> io::Result<()> {
        self.close();
        if let Some(worker) = self.worker.take() {
            return worker
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("outbound worker panicked")));
        }
        let state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state
            .failure
            .as_ref()
            .map_or(Ok(()), |failure| Err(failure.error()))
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.close();
    }
}

impl Reservation {
    pub(super) fn send(mut self, body: &[u8], metadata: PacketMetadata) -> io::Result<()> {
        if self.bytes != Some(body.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "outbound packet does not match its reservation",
            ));
        }
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.available()?;
        state.queue.push_back(Job {
            body: body.to_vec(),
            metadata,
        });
        self.bytes = None;
        drop(state);
        self.shared.ready.notify_one();
        Ok(())
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if let Some(bytes) = self.bytes.take() {
            self.shared
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .release(bytes);
            self.shared.space.notify_all();
        }
    }
}

impl Shared {
    fn shutdown(&self) {
        let shutdown = self
            .shutdown
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(shutdown) = shutdown {
            shutdown();
        }
    }

    fn enforce_close_deadline(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        while !state.finished {
            let Some(closed_at) = state.closed_at else {
                return;
            };
            let remaining = DRAIN_TIMEOUT.saturating_sub(closed_at.elapsed());
            if remaining.is_zero() {
                drop(state);
                self.shutdown();
                return;
            }
            (state, _) = self
                .stopped
                .wait_timeout(state, remaining)
                .unwrap_or_else(|p| p.into_inner());
        }
    }
}

struct WorkerShutdown {
    shared: Arc<Shared>,
}

impl Drop for WorkerShutdown {
    fn drop(&mut self) {
        self.shared.shutdown();
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .finished = true;
        self.shared.stopped.notify_all();
    }
}

struct JobBudget<'a> {
    shared: &'a Shared,
    bytes: usize,
}

impl Drop for JobBudget<'_> {
    fn drop(&mut self) {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .release(self.bytes);
        self.shared.space.notify_all();
    }
}

fn fail_queue(shared: &Shared, error: &io::Error) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    state.failure = Some(Failure {
        kind: error.kind(),
        message: error.to_string(),
    });
    state.closing = true;
    while let Some(pending) = state.queue.pop_front() {
        state.release(pending.body.len());
    }
    drop(state);
    shared.space.notify_all();
}

fn run(
    mut writer: impl Write,
    shared: &Shared,
    id: u64,
    observer: Observer,
    diagnostics: &Diagnostics,
) -> io::Result<()> {
    loop {
        let job = {
            let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            while state.queue.is_empty() && !state.closing {
                state = shared.ready.wait(state).unwrap_or_else(|p| p.into_inner());
            }
            let Some(job) = state.queue.pop_front() else {
                return Ok(());
            };
            job
        };
        let _budget = JobBudget {
            shared,
            bytes: job.body.len(),
        };
        let result = write_counted(
            &mut DrainDeadline {
                writer: &mut writer,
                shared,
            },
            &job.body,
            |bytes| diagnostics.bytes(true, bytes),
        );
        if let Err((error, bytes_written)) = result {
            fail_queue(shared, &error);
            diagnostics.report(
                "write",
                Some(&error),
                None,
                Some(FailedWrite {
                    packet: job.metadata,
                    bytes_written,
                }),
                true,
            );
            return Err(error);
        }
        diagnostics.packet(job.metadata);
        if let Some(observer) = &observer {
            let mut decoder = Decoder::new();
            decoder.feed(&job.body);
            for packet in decoder.drain().0 {
                observer.packet(id, true, &packet);
            }
        }
    }
}

struct DrainDeadline<'a, W> {
    writer: &'a mut W,
    shared: &'a Shared,
}

impl<W: Write> Write for DrainDeadline<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let closed_at = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .closed_at;
        if closed_at.is_some_and(|at| at.elapsed() >= DRAIN_TIMEOUT) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "outbound drain deadline exceeded",
            ));
        }
        self.writer.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests;
