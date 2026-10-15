mod policy;
mod server;
mod state;

pub use state::{RelayController, RelaySnapshot, RelayStatus};

use std::net::{Ipv4Addr, SocketAddr};

pub const DEFAULT_PORT: u16 = 8787;
pub const PLAY_URL: &str = "https://notwyd.app/play";
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub allowed_origins: Vec<String>,
    pub allow_private_targets: bool,
}

impl RelayConfig {
    pub(crate) fn allows_origin(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|allowed| allowed == origin)
            || (cfg!(debug_assertions) && policy::is_loopback_origin(origin))
    }
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            listen_addr: (Ipv4Addr::LOCALHOST, DEFAULT_PORT).into(),
            allowed_origins: vec!["https://notwyd.app".into()],
            allow_private_targets: false,
        }
    }
}
