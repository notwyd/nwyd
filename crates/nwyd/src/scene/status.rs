#[cfg(not(any(windows, target_arch = "wasm32")))]
use std::io::Read;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
#[cfg(not(any(windows, target_arch = "wasm32")))]
use std::time::Instant;

pub const MAX_BODY: usize = 0x400;

pub const FIELDS: usize = 12;

pub const CHANNELS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Status {
    pub populations: [i32; CHANNELS],
    pub spare: i32,
    pub day: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ParsedStatus {
    pub status: Status,
    pub converted: usize,
}

fn plain_answers(values: Vec<Option<ParsedStatus>>) -> Vec<Option<Status>> {
    values.into_iter().map(|s| s.map(|s| s.status)).collect()
}

#[derive(Debug)]

struct Batch {
    generation: u64,
    answers: Vec<Option<Option<ParsedStatus>>>,
    pending: usize,
}

#[derive(Debug)]

struct Reply {
    generation: u64,
    record: usize,
    status: Option<ParsedStatus>,
}

#[derive(Debug)]

pub struct AggregateFetch {
    pub timeout: Option<Duration>,
    pub requested: Vec<(u64, usize, String)>,
    next_generation: u64,
    active: Option<Batch>,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
    #[cfg(not(target_arch = "wasm32"))]
    worker_gate: Arc<Mutex<()>>,
    visible_generation: Arc<AtomicU64>,
}

impl Default for AggregateFetch {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            timeout: None,
            requested: Vec::new(),
            next_generation: 1,
            active: None,
            tx,
            rx,
            #[cfg(not(target_arch = "wasm32"))]
            worker_gate: Arc::new(Mutex::new(())),
            visible_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl AggregateFetch {
    pub fn begin(&mut self, urls: &[String]) -> (u64, Option<Vec<Option<Status>>>) {
        let (generation, answers) = self.begin_fields(urls);

        (generation, answers.map(plain_answers))
    }

    pub fn begin_fields(&mut self, urls: &[String]) -> (u64, Option<Vec<Option<ParsedStatus>>>) {
        let generation = self.next_generation;

        self.next_generation = self.next_generation.wrapping_add(1).max(1);

        self.active = None;

        self.visible_generation.store(generation, Ordering::Relaxed);

        for (record, url) in urls.iter().enumerate() {
            self.requested.push((generation, record, url.clone()));
        }

        let Some(timeout) = self.timeout else {
            return (generation, Some(vec![None; urls.len()]));
        };

        if urls.is_empty() {
            return (generation, Some(Vec::new()));
        }

        self.active = Some(Batch {
            generation,
            answers: vec![None; urls.len()],
            pending: urls.len(),
        });

        let urls = urls.to_vec();

        let tx = self.tx.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let gate = self.worker_gate.clone();

        let visible = self.visible_generation.clone();

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let _serial = gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

            for (record, url) in urls.into_iter().enumerate() {
                if visible.load(Ordering::Relaxed) != generation {
                    break;
                }

                let status = fetch_fields(&url, timeout).map_err(|error| {
                    tracing::info!(
                        % url, % error, "aggregate server status unavailable"
                    );

                    error
                });

                if tx
                    .send(Reply {
                        generation,
                        record,
                        status: status.ok(),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            for (record, url) in urls.into_iter().enumerate() {
                if visible.load(Ordering::Relaxed) != generation {
                    break;
                }

                let status = fetch_fields_async(&url, timeout).await.map_err(|error| {
                    tracing::info!(%url, %error, "aggregate server status unavailable");

                    error
                });

                if tx
                    .send(Reply {
                        generation,
                        record,
                        status: status.ok(),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        (generation, None)
    }

    pub fn cancel(&mut self) {
        self.active = None;

        self.visible_generation.store(0, Ordering::Relaxed);
    }

    fn accept(&mut self, reply: Reply) -> Option<Vec<Option<ParsedStatus>>> {
        let batch = self.active.as_mut()?;

        if batch.generation != reply.generation || reply.record >= batch.answers.len() {
            return None;
        }

        let answer = &mut batch.answers[reply.record];

        if answer.is_some() {
            return None;
        }

        *answer = Some(reply.status);

        batch.pending -= 1;

        if batch.pending != 0 {
            return None;
        }

        let batch = self.active.take().expect("the completed batch is active");

        Some(
            batch
                .answers
                .into_iter()
                .map(|answer| answer.expect("pending reached zero only after every answer"))
                .collect(),
        )
    }

    pub fn poll(&mut self) -> Option<Vec<Option<Status>>> {
        self.poll_fields().map(plain_answers)
    }

    pub fn poll_fields(&mut self) -> Option<Vec<Option<ParsedStatus>>> {
        let landed: Vec<Reply> = self.rx.try_iter().collect();

        let mut ready = None;

        for reply in landed {
            if let Some(statuses) = self.accept(reply) {
                ready = Some(statuses);
            }
        }

        ready
    }
}

impl Default for Status {
    fn default() -> Status {
        Status {
            populations: [-1; CHANNELS],
            spare: -1,
            day: -1,
        }
    }
}

impl Status {
    pub fn as_slice(&self) -> &[i32] {
        &self.populations
    }

    pub fn rotation(&self, count: i32, w_day: i32) -> i32 {
        if count == 0 {
            return 0;
        }

        let day = if self.day == -1 { w_day } else { self.day };

        let n = day % count;

        if n == 0 { count } else { n }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Error {
    NotHttp(String),
    Unreachable(String),
    Io(String),
    Status(u16),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotHttp(u) => write!(f, "{u} is not a valid HTTP or HTTPS status URL"),
            Error::Unreachable(e) => write!(f, "unreachable: {e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Status(code) => write!(f, "the status server answered HTTP {code}"),
        }
    }
}

fn status_url(value: &str) -> Result<url::Url, Error> {
    let url = url::Url::parse(value).map_err(|_| Error::NotHttp(value.into()))?;

    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.bytes().any(|b| b.is_ascii_control())
    {
        return Err(Error::NotHttp(value.into()));
    }

    Ok(url)
}

pub fn decode_body(headers: &str, body: &str) -> String {
    String::from_utf8_lossy(&decode_body_bytes(headers, body.as_bytes())).into_owned()
}

pub(crate) fn decode_body_bytes(headers: &str, body: &[u8]) -> Vec<u8> {
    let chunked = headers
        .lines()
        .filter_map(|l| l.split_once(':'))
        .any(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        });

    if !chunked {
        return body.to_vec();
    }

    let mut out = Vec::new();

    let mut rest = body;

    while let Some(end) = rest.iter().position(|b| *b == b'\n') {
        let line = String::from_utf8_lossy(&rest[..end]);

        let tail = &rest[end + 1..];

        let token = line.trim().split(';').next().unwrap_or("").trim();

        let Ok(len) = usize::from_str_radix(token, 16) else {
            break;
        };

        if len == 0 {
            break;
        }

        if tail.len() < len {
            out.extend_from_slice(tail);

            break;
        }

        out.extend_from_slice(&tail[..len]);

        rest = &tail[len..];

        while matches!(rest.first(), Some(b'\r' | b'\n')) {
            rest = &rest[1..];
        }
    }

    out
}

pub fn parse_response(raw: &[u8]) -> Result<Status, Error> {
    parse_response_fields(raw).map(|p| p.status)
}

pub fn parse_response_fields(raw: &[u8]) -> Result<ParsedStatus, Error> {
    let text = String::from_utf8_lossy(raw);

    let code = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| Error::Io("no status line".into()))?;

    if code != 200 {
        return Err(Error::Status(code));
    }

    let boundary = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, i + 4))
        .or_else(|| {
            raw.windows(2)
                .position(|w| w == b"\n\n")
                .map(|i| (i, i + 2))
        });

    let (header_end, body_start) = boundary.unwrap_or((raw.len(), raw.len()));

    let headers = String::from_utf8_lossy(&raw[..header_end]);

    let decoded = decode_body_bytes(&headers, &raw[body_start..]);

    Ok(parse_fields(&String::from_utf8_lossy(
        &decoded[..decoded.len().min(MAX_BODY)],
    )))
}

pub fn parse(body: &str) -> Status {
    parse_fields(body).status
}

pub fn parse_fields(body: &str) -> ParsedStatus {
    let mut fields = [-1i32; FIELDS];

    let mut converted = 0;

    for (slot, token) in fields.iter_mut().zip(body.split_ascii_whitespace()) {
        match token.parse::<i32>() {
            Ok(v) => {
                *slot = v;

                converted += 1;
            }
            Err(_) => break,
        }
    }

    let mut populations = [-1i32; CHANNELS];

    populations.copy_from_slice(&fields[..CHANNELS]);

    ParsedStatus {
        converted,
        status: Status {
            populations,
            spare: fields[10],
            day: fields[11],
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]

pub fn fetch(url: &str, timeout: Duration) -> Result<Status, Error> {
    fetch_fields(url, timeout).map(|p| p.status)
}

#[cfg(not(target_arch = "wasm32"))]

pub fn fetch_fields(url: &str, timeout: Duration) -> Result<ParsedStatus, Error> {
    #[cfg(windows)]
    {
        let _ = timeout;

        status_url(url)?;

        let mut body = crate::http_native::Get::new(None)
            .get(url, MAX_BODY, true)
            .map_err(|error| Error::Unreachable(error.to_string()))?;

        body.truncate(MAX_BODY - 1);

        return Ok(parse_fields(&String::from_utf8_lossy(&body)));
    }

    #[cfg(not(any(windows, target_arch = "wasm32")))]
    fetch_with_agent(url, timeout, &status_agent(ureq::tls::TlsConfig::default()))
}

#[cfg(not(any(windows, target_arch = "wasm32")))]

fn status_agent(tls: ureq::tls::TlsConfig) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .proxy(None)
        .user_agent("MS")
        .max_redirects(0)
        .http_status_as_error(false)
        .max_response_header_size(8192)
        .input_buffer_size(4096)
        .build();

    config.into()
}

#[cfg(not(any(windows, target_arch = "wasm32")))]

fn fetch_with_agent(
    value: &str,
    timeout: Duration,
    agent: &ureq::Agent,
) -> Result<ParsedStatus, Error> {
    let started = Instant::now();

    let mut url = status_url(value)?;

    let mut secured = url.scheme() == "https";

    for redirects in 0..=3 {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| Error::Io("status request deadline exceeded".into()))?;

        let mut response = agent
            .get(url.as_str())
            .header("Connection", "close")
            .config()
            .timeout_global(Some(remaining))
            .build()
            .call()
            .map_err(|e| Error::Unreachable(e.to_string()))?;

        let code = response.status().as_u16();

        if matches!(code, 301 | 302 | 303 | 307 | 308) {
            if redirects == 3 {
                return Err(Error::Io("too many status redirects".into()));
            }

            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| Error::Io("status redirect has no valid Location".into()))?;

            let next = url.join(location).map_err(|e| Error::Io(e.to_string()))?;

            let next = status_url(next.as_str())?;

            if secured && next.scheme() != "https" {
                return Err(Error::Io("refusing HTTPS status downgrade".into()));
            }

            secured |= next.scheme() == "https";

            url = next;

            continue;
        }

        if code != 200 {
            return Err(Error::Status(code));
        }

        if response
            .headers()
            .get("content-encoding")
            .is_some_and(|v| v != "identity")
        {
            return Err(Error::Io("unsupported status content encoding".into()));
        }

        let mut body = Vec::with_capacity(MAX_BODY);

        response
            .body_mut()
            .as_reader()
            .take(MAX_BODY as u64)
            .read_to_end(&mut body)
            .map_err(|e| Error::Io(e.to_string()))?;

        return Ok(parse_fields(&String::from_utf8_lossy(&body)));
    }

    unreachable!("the last redirect returns an error")
}

#[cfg(target_arch = "wasm32")]

pub async fn fetch_fields_async(value: &str, timeout: Duration) -> Result<ParsedStatus, Error> {
    let url = status_url(value)?;

    let (code, bytes) = crate::http_web::fetch(url.as_str(), None, timeout, MAX_BODY - 1)
        .await
        .map_err(Error::Unreachable)?;

    if code != 200 {
        return Err(Error::Status(code));
    }

    Ok(parse_fields(&String::from_utf8_lossy(&bytes)))
}
