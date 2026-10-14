use super::super::status;
use formats::config::serverlist::{self, ServerList};

#[derive(Debug)]
struct Pending {
    record: usize,
    selectors: [i32; 10],
    replies: Vec<Option<status::ParsedStatus>>,
}

#[derive(Debug)]
pub struct State {
    selectors: [i32; 10],
    pending: Option<Pending>,
    fetch: status::AggregateFetch,
}

impl State {
    pub fn new(table: &ServerList, day: i32) -> Self {
        let mut state = Self {
            selectors: [0; 10],
            pending: None,
            fetch: status::AggregateFetch::default(),
        };

        state.add_and_rotate(table, day);

        state.add_and_rotate(table, day);

        state
    }

    fn add_and_rotate(&mut self, table: &ServerList, day: i32) {
        for (slot, server) in self.selectors.iter_mut().zip(&table.servers) {
            let count = server
                .channels
                .iter()
                .take(10)
                .filter(|s| !s.is_empty())
                .count() as i32;

            *slot += count;

            if *slot != 0 {
                let n = day % *slot;

                *slot = if n == 0 { *slot } else { n };
            }
        }
    }

    pub fn refresh(&mut self, table: &ServerList, day: i32) {
        self.selectors = [0; 10];

        self.add_and_rotate(table, day);
    }

    pub fn cancel(&mut self) {
        self.fetch.cancel();

        self.pending = None;
    }

    pub fn start(
        &mut self,
        table: &mut ServerList,
        record: usize,
        timeout: Option<std::time::Duration>,
    ) -> Vec<(usize, String)> {
        self.cancel();

        let count = record.min(serverlist::SERVERS);

        let addresses: Vec<_> = (0..count)
            .rev()
            .map(|source| {
                self.selectors[source]
                    .checked_sub(1)
                    .filter(|n| (0..10).contains(n))
                    .and_then(|n| table.servers.get(source)?.channels.get(n as usize))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        let urls: Vec<_> = (0..count)
            .map(|source| {
                table
                    .servers
                    .get(source)
                    .map(|s| s.status_url.clone())
                    .unwrap_or_default()
            })
            .collect();

        if let Some(target) = table.servers.get_mut(record) {
            target.channels.resize(10, String::new());

            for index in count.saturating_sub(1)..9 {
                target.channels[index].clear();
            }

            for (index, address) in addresses.into_iter().enumerate() {
                target.channels[index] = address;
            }
        }

        self.fetch.timeout = timeout;

        let (_, ready) = self.fetch.begin_fields(&urls);

        self.pending = Some(Pending {
            record,
            selectors: self.selectors,
            replies: ready.unwrap_or_else(|| vec![None; count]),
        });

        urls.into_iter().enumerate().collect()
    }

    pub fn active(&self, record: usize) -> bool {
        self.pending.as_ref().is_some_and(|p| p.record == record)
    }

    pub fn result(&self) -> status::Status {
        let Some(p) = &self.pending else {
            return status::Status::default();
        };

        let mut status = status::Status::default();

        for (source, reply) in p.replies.iter().enumerate() {
            if let Some(reply) = reply {
                if reply.converted > 10 {
                    status.spare = reply.status.spare;
                }

                if reply.converted > 11 {
                    status.day = reply.status.day;
                }
            }

            if let Some(value) = p.selectors[source]
                .checked_sub(1)
                .filter(|n| *n >= 0)
                .and_then(|n| reply.as_ref()?.status.populations.get(n as usize))
            {
                status.populations[p.replies.len() - source - 1] = *value;
            }
        }

        status
    }

    pub fn poll(&mut self) -> bool {
        if let Some(replies) = self.fetch.poll_fields()
            && let Some(p) = self.pending.as_mut()
        {
            p.replies = replies;

            return true;
        }

        false
    }
}
