use formats::net::guild_web::Endpoint;
#[cfg(not(any(windows, target_arch = "wasm32")))]
use std::io::Read;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

pub const ANSWER_AT: usize = 0x100C;

pub const ANSWER_MAX: usize = 0x32000;

pub const FIELD_MAX: usize = 0x800;

pub const METHOD_POST: u32 = 2;

pub const ACCEPT: &str = "*/*";

pub const AGENT: &str = "HTTP";

pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(0x3E8);

pub const ACCEPTED_STATUS: u16 = 0xC8;

pub const SLOTS: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum State {
    Failed = 3,
    Done = 4,
}

#[derive(Clone, PartialEq, Eq)]

pub struct Request {
    pub tag: i32,
    pub url: String,
    pub body: String,
    pub delay_ms: u32,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuildRequest")
            .field("tag", &self.tag)
            .field("delay_ms", &self.delay_ms)
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl Request {
    pub fn new(host: &str, endpoint: Endpoint, body: String) -> Self {
        let (path, _) = endpoint.parts();

        Request {
            tag: endpoint.tag(),
            url: format!("{host}{path}"),
            body,
            delay_ms: 0,
        }
    }

    pub fn truncated(&self) -> bool {
        self.url.len() >= FIELD_MAX || self.body.len() >= FIELD_MAX
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Answer {
    pub tag: i32,
    pub state: State,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Error {
    BadUrl(String),
    Io(String),
    Status(u16),
}

pub fn crack(address: &str) -> Result<(String, u16, String), Error> {
    let url = url::Url::parse(address).map_err(|e| Error::BadUrl(e.to_string()))?;

    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::BadUrl("unsupported guild URL".into()));
    }

    let host = url
        .host_str()
        .ok_or_else(|| Error::BadUrl("no host".into()))?;

    let mut path = url.path().to_owned();

    if let Some(query) = url.query() {
        path.push('?');

        path.push_str(query);
    }

    Ok((host.to_owned(), url.port_or_known_default().unwrap(), path))
}

#[cfg(not(target_arch = "wasm32"))]

pub fn fetch(request: &Request, timeout: Duration) -> Result<String, Error> {
    Transport::new(timeout).fetch(request)
}

#[cfg(not(target_arch = "wasm32"))]

struct Transport {
    #[cfg(not(any(windows, target_arch = "wasm32")))]
    agent: ureq::Agent,
    #[cfg(windows)]
    native: crate::http_native::Post,
}

#[cfg(not(target_arch = "wasm32"))]

impl Transport {
    fn new(_timeout: Duration) -> Self {
        #[cfg(not(any(windows, target_arch = "wasm32")))]
        let config = ureq::Agent::config_builder()
            .proxy(None)
            .user_agent(AGENT)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(_timeout))
            .http_status_as_error(false)
            .build();

        Self {
            #[cfg(not(any(windows, target_arch = "wasm32")))]
            agent: config.into(),
            #[cfg(windows)]
            native: crate::http_native::Post::default(),
        }
    }

    fn fetch(&mut self, request: &Request) -> Result<String, Error> {
        crack(&request.url)?;

        #[cfg(windows)]
        {
            let (status, body) = self
                .native
                .post(&request.url, request.body.as_bytes(), ANSWER_MAX)
                .map_err(|e| Error::Io(e.to_string()))?;

            if status != ACCEPTED_STATUS {
                return Err(Error::Status(status));
            }

            return Ok(answer_text(&body));
        }

        #[cfg(not(any(windows, target_arch = "wasm32")))]
        fetch_with_agent(request, &self.agent)
    }
}

#[cfg(not(any(windows, target_arch = "wasm32")))]

fn fetch_with_agent(request: &Request, agent: &ureq::Agent) -> Result<String, Error> {
    let mut response = agent
        .post(&request.url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", ACCEPT)
        .send(request.body.as_bytes())
        .map_err(|e| Error::Io(e.to_string()))?;

    let status = response.status().as_u16();

    if status != ACCEPTED_STATUS {
        return Err(Error::Status(status));
    }

    let mut reader = response.body_mut().as_reader();

    let body = crate::http_response::read_bounded(ANSWER_MAX, false, |out| reader.read(out));

    Ok(answer_text(&body))
}

fn answer_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(crate::http_response::c_body(bytes, ANSWER_MAX)).into_owned()
}

pub struct Ring {
    #[cfg(not(target_arch = "wasm32"))]
    to_worker: Sender<Request>,
    #[cfg(target_arch = "wasm32")]
    browser: BrowserWorker,
    from_worker: Receiver<Answer>,
    queued: usize,
    dropped: usize,
}

impl Ring {
    #[cfg(not(target_arch = "wasm32"))]

    pub fn spawn(timeout: Duration) -> Self {
        let (to_worker, work) = channel::<Request>();

        let (done, from_worker) = channel::<Answer>();

        std::thread::spawn(move || {
            let mut client = Transport::new(timeout);

            while let Ok(request) = work.recv() {
                if request.delay_ms != 0 {
                    std::thread::sleep(Duration::from_millis(u64::from(request.delay_ms)));
                }

                let answer = match client.fetch(&request) {
                    Ok(body) => Answer {
                        tag: request.tag,
                        state: State::Done,
                        body,
                    },
                    Err(_) => Answer {
                        tag: request.tag,
                        state: State::Failed,
                        body: String::new(),
                    },
                };

                if done.send(answer).is_err() {
                    break;
                }
            }
        });

        Ring {
            to_worker,
            from_worker,
            queued: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, request: Request) -> bool {
        if self.queued >= SLOTS {
            self.dropped += 1;

            return false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.to_worker.send(request).is_err() {
            self.dropped += 1;

            return false;
        }

        #[cfg(target_arch = "wasm32")]
        self.browser.push(request);

        self.queued += 1;

        true
    }

    pub fn poll(&mut self) -> Option<Answer> {
        match self.from_worker.try_recv() {
            Ok(answer) => {
                self.queued = self.queued.saturating_sub(1);

                Some(answer)
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn queued(&self) -> usize {
        self.queued
    }
}

#[cfg(target_arch = "wasm32")]

async fn fetch_async(request: &Request, timeout: Duration) -> Result<String, Error> {
    crack(&request.url)?;

    let (status, body) =
        crate::http_web::fetch(&request.url, Some(&request.body), timeout, ANSWER_MAX)
            .await
            .map_err(Error::Io)?;

    if status != ACCEPTED_STATUS {
        return Err(Error::Status(status));
    }

    Ok(answer_text(&body))
}

#[cfg(target_arch = "wasm32")]

struct BrowserWorker {
    timeout: Duration,
    done: Sender<Answer>,
    pending: std::rc::Rc<std::cell::RefCell<std::collections::VecDeque<Request>>>,
    running: std::rc::Rc<std::cell::Cell<bool>>,
}

#[cfg(target_arch = "wasm32")]

impl BrowserWorker {
    fn push(&self, request: Request) {
        self.pending.borrow_mut().push_back(request);

        if self.running.replace(true) {
            return;
        }

        let pending = self.pending.clone();

        let running = self.running.clone();

        let done = self.done.clone();

        let timeout = self.timeout;

        wasm_bindgen_futures::spawn_local(async move {
            loop {
                let Some(request) = pending.borrow_mut().pop_front() else {
                    break;
                };

                if request.delay_ms != 0 {
                    crate::http_web::delay(request.delay_ms).await;
                }

                let answer = match fetch_async(&request, timeout).await {
                    Ok(body) => Answer {
                        tag: request.tag,
                        state: State::Done,
                        body,
                    },
                    Err(_) => Answer {
                        tag: request.tag,
                        state: State::Failed,
                        body: String::new(),
                    },
                };

                if done.send(answer).is_err() {
                    pending.borrow_mut().clear();

                    break;
                }
            }

            running.set(false);
        });
    }
}

#[cfg(target_arch = "wasm32")]

impl Ring {
    pub fn spawn(timeout: Duration) -> Self {
        let (done, from_worker) = channel();

        Self {
            browser: BrowserWorker {
                timeout,
                done,
                pending: Default::default(),
                running: Default::default(),
            },
            from_worker,
            queued: 0,
            dropped: 0,
        }
    }
}
