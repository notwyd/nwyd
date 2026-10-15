use crate::{RelayConfig, server};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayStatus {
    Launching,
    Ready,
    Failed(String),
    Stopped,
}

#[derive(Clone, Debug)]
pub struct RelaySnapshot {
    pub status: RelayStatus,
    pub listen_addr: Option<SocketAddr>,
    pub active_sessions: u64,
    pub bytes_to_server: u64,
    pub bytes_from_server: u64,
    pub last_error: Option<String>,
}

pub(crate) type SharedSnapshot = Arc<Mutex<RelaySnapshot>>;

pub(crate) fn io_error_message(context: &str, error: &std::io::Error) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::AddrInUse => {
            "endereço já em uso; feche o outro relay e tente novamente"
        }
        std::io::ErrorKind::AddrNotAvailable => "endereço indisponível",
        std::io::ErrorKind::PermissionDenied => "permissão negada pelo sistema",
        std::io::ErrorKind::TimedOut => "o tempo de espera esgotou",
        std::io::ErrorKind::OutOfMemory => "memória insuficiente",
        _ => "falha do sistema; tente novamente",
    };
    match error.raw_os_error() {
        Some(code) => format!("{context}: {reason} (código {code})."),
        None => format!("{context}: {reason}."),
    }
}

pub struct RelayController {
    config: RelayConfig,
    snapshot: SharedSnapshot,
    shutdown: Mutex<Option<watch::Sender<bool>>>,
}

impl RelayController {
    pub fn new(config: RelayConfig) -> Self {
        let controller = Self {
            config,
            snapshot: Arc::new(Mutex::new(RelaySnapshot {
                status: RelayStatus::Stopped,
                listen_addr: None,
                active_sessions: 0,
                bytes_to_server: 0,
                bytes_from_server: 0,
                last_error: None,
            })),
            shutdown: Mutex::new(None),
        };
        controller.retry();
        controller
    }

    pub fn snapshot(&self) -> RelaySnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn retry(&self) {
        let mut control = self.shutdown.lock().unwrap_or_else(|e| e.into_inner());
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(snapshot.status, RelayStatus::Launching | RelayStatus::Ready) {
            return;
        }
        snapshot.status = RelayStatus::Launching;
        snapshot.listen_addr = None;
        snapshot.last_error = None;
        let (shutdown, receiver) = watch::channel(false);
        *control = Some(shutdown);
        drop(snapshot);
        let snapshot = self.snapshot.clone();
        let config = self.config.clone();
        let failed_snapshot = snapshot.clone();
        let result = std::thread::Builder::new()
            .name("nwyd-relay".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|e| io_error_message("Não foi possível preparar o relay", &e))
                    .and_then(|runtime| {
                        runtime.block_on(server::run(config, snapshot.clone(), receiver))
                    });
                let mut state = snapshot.lock().unwrap_or_else(|e| e.into_inner());
                state.active_sessions = 0;
                state.listen_addr = None;
                state.status = match result {
                    Ok(()) => RelayStatus::Stopped,
                    Err(error) => {
                        state.last_error = Some(error.clone());
                        RelayStatus::Failed(error)
                    }
                };
            });
        if let Err(error) = result {
            let mut snapshot = failed_snapshot.lock().unwrap_or_else(|e| e.into_inner());
            snapshot.status =
                RelayStatus::Failed(io_error_message("Não foi possível iniciar o relay", &error));
        }
    }

    pub fn shutdown(&self) {
        if let Some(sender) = self
            .shutdown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let _ = sender.send(true);
        }
    }
}

impl Drop for RelayController {
    fn drop(&mut self) {
        self.shutdown();
    }
}
