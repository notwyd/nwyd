use formats::config::serverlist::ServerList;
use protocol::server::Redirect;

pub const GRACE_MS: u32 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Cached {
    WrongClient { expected: u16, got: u16 },
    Stored { channel: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Closed {
    Lost,
    Follow(Redirect),
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Destination {
    pub channel: i32,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct State {
    grace_at_ms: u32,
    cached: Option<Redirect>,
}

impl State {
    pub fn arm_transfer(&mut self, now_ms: u32) {
        self.grace_at_ms = now_ms;
    }

    pub fn cache(&mut self, local_client_id: u16, redirect: Redirect) -> Cached {
        if redirect.client_id != local_client_id {
            return Cached::WrongClient {
                expected: local_client_id,
                got: redirect.client_id,
            };
        }

        let channel = redirect.channel().unwrap_or(0);

        self.cached = Some(redirect);

        Cached::Stored { channel }
    }

    pub fn disconnected(&mut self, now_ms: u32) -> Closed {
        let armed = self.grace_at_ms != 0 && now_ms.wrapping_sub(self.grace_at_ms) <= GRACE_MS;

        self.grace_at_ms = 0;

        if armed && let Some(redirect) = self.cached.take() {
            Closed::Follow(redirect)
        } else {
            Closed::Lost
        }
    }

    pub fn stream_failed(&mut self, socket_closed: bool, now_ms: u32) -> Closed {
        match socket_closed {
            true => self.disconnected(now_ms),
            false => Closed::Lost,
        }
    }
}

pub fn destination(table: &ServerList, server: usize, redirect: &Redirect) -> Destination {
    let channel = redirect.channel().unwrap_or(0);

    let address = table.servers.get(server).and_then(|record| match channel {
        0 => Some(record.status_url.as_str()),
        1.. => usize::try_from(channel - 1)
            .ok()
            .and_then(|index| record.channels.get(index))
            .map(String::as_str),
        _ => None,
    });

    Destination {
        channel,
        address: address
            .filter(|address| !address.is_empty())
            .map(str::to_owned),
    }
}
