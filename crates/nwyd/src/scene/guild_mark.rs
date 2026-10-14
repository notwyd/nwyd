use formats::data::guild_mark::{Mark, READ_MAX, URL_RECORD, decode, file_name};
use std::collections::HashMap;
#[cfg(not(any(windows, target_arch = "wasm32")))]
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;
#[cfg(not(any(windows, target_arch = "wasm32")))]
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod worker;
#[cfg(target_arch = "wasm32")]
#[path = "guild_mark/worker_web.rs"]
mod worker;

#[cfg(not(any(windows, target_arch = "wasm32")))]
use super::guild_http::CONNECT_TIMEOUT;
use super::guild_http::Error;

pub const SLOTS: usize = 0xBB8;

pub const SLOT_CEILING: usize = 0xBB8;

pub const FREE: u32 = 0xFFFF_FFFF;

pub const fn key(mark: Mark) -> u32 {
    ((mark.series as u32) << 16) | mark.guild as u32
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Slot {
    pub key: u32,
    pub pixels: Option<Vec<u8>>,
    pub serial: u64,
}

#[derive(Debug)]

pub struct Bank {
    identity: u64,
    slots: Vec<Slot>,
    serial: u64,
}

impl Default for Bank {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        Bank {
            identity: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            slots: vec![Bank::free(); SLOTS],
            serial: 0,
        }
    }
}

impl Bank {
    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }

    pub fn find(&self, key: u32) -> Option<usize> {
        self.slots.iter().position(|slot| slot.key == key)
    }

    fn free() -> Slot {
        Slot {
            key: FREE,
            ..Slot::default()
        }
    }

    pub fn allocate(&mut self) -> Option<usize> {
        self.slots.iter().position(|slot| slot.key == FREE)
    }

    pub fn claim(&mut self, at: usize, key: u32) {
        self.slots[at].key = key;

        self.slots[at].pixels = None;
    }

    pub fn publish(&mut self, at: usize, pixels: Vec<u8>) {
        self.serial += 1;

        self.slots[at].pixels = Some(pixels);

        self.slots[at].serial = self.serial;
    }

    pub fn release(&mut self, at: usize) {
        self.slots[at] = Bank::free();
    }

    pub fn slot(&self, at: usize) -> Option<&Slot> {
        self.slots.get(at)
    }

    pub fn ready(&self) -> impl Iterator<Item = (usize, &Slot)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.pixels.is_some())
    }

    pub fn taken(&self) -> usize {
        self.slots.iter().filter(|slot| slot.key != FREE).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Want {
    pub slot: usize,
    pub key: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Got {
    pub slot: usize,
    pub key: u32,
    pub pixels: Option<Vec<u8>>,
}

#[derive(Default)]

pub struct Marks {
    pub bank: Bank,
    pub prefix: String,
    pub server: u32,
    pub off: bool,
    to_worker: Option<worker::Sender>,
    from_worker: Option<Receiver<Got>>,
    inflight: HashMap<u32, usize>,
    pub failed: usize,
    pub loaded: usize,
}

impl Marks {
    pub fn with_strings(mut self, strings: &ui::Strings, server: usize) -> Self {
        let record = strings.translate(&format!("ss_{URL_RECORD}"));

        self.prefix = match record.starts_with("ss_") {
            true => String::new(),
            false => record,
        };

        self.server = server as u32;

        self
    }

    pub fn spawn(mut self, timeout: Duration) -> Self {
        let (done, from_worker) = channel::<Got>();

        let to_worker = worker::sender(timeout, done);

        self.to_worker = Some(to_worker);

        self.from_worker = Some(from_worker);

        let mut reserved: Vec<_> = self
            .inflight
            .iter()
            .map(|(&key, &slot)| (slot, key))
            .collect();

        reserved.sort_unstable();

        for (slot, key) in reserved {
            let mark = Mark {
                series: (key >> 16) as u16,
                guild: key as u16,
            };

            if self
                .to_worker
                .as_ref()
                .unwrap()
                .send(Want {
                    slot,
                    key,
                    url: format!("{}{}", self.prefix, file_name(self.server, mark)),
                })
                .is_err()
            {
                self.bank.release(slot);

                self.inflight.remove(&key);

                self.failed += 1;
            }
        }

        self
    }

    pub fn start(&mut self, timeout: Duration) {
        if self.to_worker.is_some() || self.prefix.is_empty() {
            return;
        }

        let started = std::mem::take(self).spawn(timeout);

        *self = started;
    }

    pub fn wanted(&self) -> bool {
        !self.off && !self.prefix.is_empty()
    }

    pub fn resolve(&mut self, guild_id: u16) -> Option<usize> {
        if Mark::absent(guild_id) {
            return None;
        }

        let mark = Mark::split(guild_id);

        let k = key(mark);

        if let Some(at) = self.bank.find(k) {
            return Some(at);
        }

        if !self.wanted() {
            return None;
        }

        let at = self.bank.allocate()?;

        self.bank.claim(at, k);

        self.inflight.insert(k, at);

        let url = format!("{}{}", self.prefix, file_name(self.server, mark));

        if let Some(sender) = self.to_worker.as_ref() {
            if sender
                .send(Want {
                    slot: at,
                    key: k,
                    url,
                })
                .is_err()
            {
                self.bank.release(at);

                self.inflight.remove(&k);

                self.failed += 1;

                return None;
            }
        }

        Some(at)
    }

    pub fn pump(&mut self) {
        let Some(rx) = self.from_worker.as_ref() else {
            return;
        };

        worker::pump();

        for _ in 0..8 {
            match rx.try_recv() {
                Ok(got) => {
                    if self.inflight.get(&got.key) != Some(&got.slot) {
                        continue;
                    }

                    self.inflight.remove(&got.key);

                    if self.bank.slot(got.slot).map(|s| s.key) != Some(got.key) {
                        continue;
                    }

                    match got.pixels {
                        Some(pixels) => {
                            self.bank.publish(got.slot, pixels);

                            self.loaded += 1;

                            tracing::debug!(
                                key = got.key,
                                slot = got.slot,
                                "guildmark ready for rendering"
                            );
                        }
                        None => {
                            self.bank.release(got.slot);

                            self.failed += 1;
                        }
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return,
            }
        }
    }

    pub fn in_flight(&self) -> usize {
        self.inflight.len()
    }
}

#[cfg(not(target_arch = "wasm32"))]

pub fn fetch(url: &str, timeout: Duration) -> Result<Vec<u8>, Error> {
    #[cfg(windows)]
    {
        let _ = timeout;

        return fetch_native(url);
    }

    #[cfg(not(any(windows, target_arch = "wasm32")))]
    fetch_with_agent(url, timeout, &badge_agent(ureq::tls::TlsConfig::default()))
}

#[cfg(windows)]

fn fetch_native(url: &str) -> Result<Vec<u8>, Error> {
    badge_url(url)?;

    crate::http_native::guildmark_get(url, READ_MAX).map_err(|error| Error::Io(error.to_string()))
}

#[cfg(not(any(windows, target_arch = "wasm32")))]

fn badge_agent(tls: ureq::tls::TlsConfig) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .proxy(None)
        .user_agent("MS")
        .max_redirects(0)
        .http_status_as_error(false)
        .max_response_header_size(8192)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .build();

    config.into()
}

fn badge_url(value: &str) -> Result<url::Url, Error> {
    let url = url::Url::parse(value).map_err(|e| Error::BadUrl(e.to_string()))?;

    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::BadUrl(
            "expected an HTTP(S) URL without credentials".into(),
        ));
    }

    Ok(url)
}

#[cfg(not(any(windows, target_arch = "wasm32")))]

fn fetch_with_agent(value: &str, timeout: Duration, agent: &ureq::Agent) -> Result<Vec<u8>, Error> {
    let started = Instant::now();

    let mut url = badge_url(value)?;

    for redirects in 0..=3 {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| Error::Io("guildmark request deadline exceeded".into()))?;

        let mut response = agent
            .get(url.as_str())
            .header("Accept", "*/*")
            .header("Cache-Control", "no-cache")
            .header("Connection", "close")
            .config()
            .timeout_global(Some(remaining))
            .build()
            .call()
            .map_err(|e| Error::Io(e.to_string()))?;

        let status = response.status().as_u16();

        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            if redirects == 3 {
                return Err(Error::Io("too many guildmark redirects".into()));
            }

            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| Error::Io("guildmark redirect has no valid Location".into()))?;

            let next = url
                .join(location)
                .map_err(|e| Error::BadUrl(e.to_string()))?;

            let next = badge_url(next.as_str())?;

            if url.scheme() == "https" && next.scheme() != "https" {
                return Err(Error::Io("refusing HTTPS guildmark downgrade".into()));
            }

            url = next;

            continue;
        }

        if status != super::guild_http::ACCEPTED_STATUS {
            return Err(Error::Status(status));
        }

        if response
            .headers()
            .get("content-encoding")
            .is_some_and(|v| v != "identity")
        {
            return Err(Error::Io("unsupported guildmark content encoding".into()));
        }

        let mut reader = response.body_mut().as_reader();

        let body = crate::http_response::read_bounded(READ_MAX, false, |out| reader.read(out));

        return Ok(body);
    }

    unreachable!("the last redirect returns an error")
}

#[cfg(target_arch = "wasm32")]

async fn fetch_async(value: &str, timeout: Duration) -> Result<Vec<u8>, Error> {
    let url = badge_url(value)?;

    let (status, bytes) = crate::http_web::fetch(url.as_str(), None, timeout, READ_MAX)
        .await
        .map_err(Error::Io)?;

    if status != super::guild_http::ACCEPTED_STATUS {
        return Err(Error::Status(status));
    }

    Ok(bytes)
}
