use super::Observer;
use std::{collections::VecDeque, io, net::SocketAddr, sync::Mutex};
use web_time::Instant;

#[derive(Clone, Debug)]

pub struct PacketMetadata {
    pub elapsed_ms: u64,
    pub outgoing: bool,
    pub opcode: u16,
    pub size: usize,
    pub wire_tick: u32,
}

#[derive(Clone, Debug)]

pub struct IoFailure {
    pub kind: io::ErrorKind,
    pub raw_os_error: Option<i32>,
    pub message: String,
}

#[derive(Clone, Debug)]

pub struct FailedWrite {
    pub packet: PacketMetadata,
    pub bytes_written: usize,
}

#[derive(Clone, Debug)]

pub struct DisconnectReport {
    pub connection: u64,
    pub target: String,
    pub peer: Option<SocketAddr>,
    pub operation: &'static str,
    pub first_terminal: bool,
    pub error: Option<IoFailure>,
    pub detail: Option<String>,
    pub elapsed_ms: u64,
    pub bytes_received: u64,
    pub bytes_written: u64,
    pub since_receive_ms: Option<u64>,
    pub since_write_ms: Option<u64>,
    pub pending_receive_bytes: usize,
    pub failed_write: Option<FailedWrite>,
    pub recent_packets: Vec<PacketMetadata>,
}

impl std::fmt::Display for DisconnectReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "connection {} to {}: {}",
            self.connection, self.target, self.operation
        )?;

        if let Some(error) = &self.error {
            write!(
                f,
                ": {} ({:?}, OS {:?})",
                error.message, error.kind, error.raw_os_error
            )?;
        }

        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }

        if let Some(failed) = &self.failed_write {
            write!(
                f,
                "; opcode {:#05x}, wrote {}/{} bytes",
                failed.packet.opcode, failed.bytes_written, failed.packet.size
            )?;
        }

        if self.pending_receive_bytes != 0 {
            write!(
                f,
                "; {} incomplete frame bytes buffered",
                self.pending_receive_bytes
            )?;
        }

        Ok(())
    }
}

#[derive(Default)]

struct State {
    peer: Option<SocketAddr>,
    bytes_received: u64,
    bytes_written: u64,
    last_receive: Option<u64>,
    last_write: Option<u64>,
    pending: usize,
    packets: VecDeque<PacketMetadata>,
    terminal: Option<DisconnectReport>,
}

pub(super) struct Diagnostics {
    id: u64,
    target: String,
    started: Instant,
    observer: Observer,
    state: Mutex<State>,
}

impl Diagnostics {
    pub(super) fn new(id: u64, target: String, observer: Observer) -> Self {
        Self {
            id,
            target,
            started: Instant::now(),
            observer,
            state: Mutex::new(State::default()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn peer(&self, peer: SocketAddr) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).peer = Some(peer);
    }

    pub(super) fn metadata(
        &self,
        outgoing: bool,
        opcode: u16,
        size: usize,
        wire_tick: u32,
    ) -> PacketMetadata {
        PacketMetadata {
            elapsed_ms: self.elapsed(),
            outgoing,
            opcode,
            size,
            wire_tick,
        }
    }

    fn elapsed(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub(super) fn packet(&self, mut packet: PacketMetadata) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        packet.elapsed_ms = self.elapsed();

        if state.packets.len() == 32 {
            state.packets.pop_front();
        }

        state.packets.push_back(packet);
    }

    pub(super) fn bytes(&self, outgoing: bool, count: usize) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if outgoing {
            state.bytes_written += count as u64;

            state.last_write = Some(self.elapsed());
        } else {
            state.bytes_received += count as u64;

            state.last_receive = Some(self.elapsed());
        }
    }

    pub(super) fn pending(&self, count: usize) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).pending = count;
    }

    pub(super) fn terminal(&self) -> Option<DisconnectReport> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .terminal
            .clone()
    }

    pub(super) fn report(
        &self,
        operation: &'static str,
        error: Option<&io::Error>,
        detail: Option<String>,
        failed_write: Option<FailedWrite>,
        terminal: bool,
    ) -> DisconnectReport {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(report) = &state.terminal
            && failed_write.is_none()
        {
            return report.clone();
        }

        let elapsed_ms = self.elapsed();

        let report = DisconnectReport {
            connection: self.id,
            target: self.target.clone(),
            peer: state.peer,
            operation,
            first_terminal: terminal && state.terminal.is_none(),
            error: error.map(|e| IoFailure {
                kind: e.kind(),
                raw_os_error: e.raw_os_error(),
                message: e.to_string(),
            }),
            detail,
            elapsed_ms,
            bytes_received: state.bytes_received,
            bytes_written: state.bytes_written,
            since_receive_ms: state.last_receive.map(|at| elapsed_ms.saturating_sub(at)),
            since_write_ms: state.last_write.map(|at| elapsed_ms.saturating_sub(at)),
            pending_receive_bytes: state.pending,
            failed_write,
            recent_packets: state.packets.iter().cloned().collect(),
        };

        if terminal && state.terminal.is_none() {
            state.terminal = Some(report.clone());
        }

        drop(state);

        tracing::info!(
            target : "protocol::transport", connection = self.id, operation, ? report,
            "connection diagnostic"
        );

        if let Some(observer) = &self.observer {
            observer.diagnostic(&report);
        }

        report
    }
}

pub(super) fn write_counted(
    writer: &mut impl io::Write,
    bytes: &[u8],
    mut progress: impl FnMut(usize),
) -> Result<(), (io::Error, usize)> {
    let mut written = 0;

    while written < bytes.len() {
        match writer.write(&bytes[written..]) {
            Ok(0) => {
                return Err((
                    io::Error::new(io::ErrorKind::WriteZero, "socket accepted zero bytes"),
                    written,
                ));
            }
            Ok(count) => {
                written += count;

                progress(count);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err((error, written)),
        }
    }

    Ok(())
}
