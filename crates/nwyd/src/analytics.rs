use crate::scene::enter::Entered;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};

pub const DEFAULT_BASE_URL: &str = "https://notwyd.app";

pub const DEFAULT_ENVIRONMENT_KEY: &str = "nwyd";

#[cfg(not(target_arch = "wasm32"))]

const CAPACITY: usize = 100;

const MAX_BYTES: usize = 2047;

#[cfg(all(not(windows), not(target_arch = "wasm32")))]

const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Config {
    pub base_url: String,
    pub environment_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            environment_key: DEFAULT_ENVIRONMENT_KEY.into(),
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        fn read(name: &str) -> anyhow::Result<Option<String>> {
            match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
            }
        }

        Ok(Self::from_overrides(
            read("NWYD_ANALYTICS_BASE_URL")?,
            read("NWYD_ANALYTICS_ENVIRONMENT_KEY")?,
        ))
    }

    fn from_overrides(base_url: Option<String>, environment_key: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            environment_key: environment_key.unwrap_or_else(|| DEFAULT_ENVIRONMENT_KEY.into()),
        }
    }

    pub fn endpoint(&self) -> anyhow::Result<Option<url::Url>> {
        if self.base_url.is_empty() {
            return Ok(None);
        }

        let mut url = url::Url::parse(&self.base_url)
            .map_err(|_| anyhow::anyhow!("invalid analytics base URL"))?;

        let loopback = url.host_str().is_some_and(|host| {
            host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
        });

        anyhow::ensure!(
            url.has_host()
                && (url.scheme() == "https" || (url.scheme() == "http" && loopback))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "analytics base URL requires HTTPS (or loopback HTTP), without credentials, a query or a fragment"
        );

        anyhow::ensure!(
            !self.environment_key.is_empty()
                && self
                    .environment_key
                    .bytes()
                    .all(|byte| { byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') }),
            "analytics environment key must contain only letters, digits, underscores or hyphens"
        );

        let path = format!(
            "{}/collect/api/{}",
            url.path().trim_end_matches('/'),
            self.environment_key
        );

        url.set_path(&path);

        Ok(Some(url))
    }
}

#[cfg(not(target_arch = "wasm32"))]

pub fn configured(config: &Config) -> anyhow::Result<Analytics> {
    let Some(url) = config.endpoint()? else {
        return Ok(Analytics::default());
    };

    #[cfg(windows)]
    {
        let mut client = native::Client::new(&url);

        Analytics::worker(move |body| client.post(body))
    }

    #[cfg(all(not(windows), not(target_arch = "wasm32")))]
    {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .user_agent("HTTP")
            .max_redirects(10)
            .timeout_connect(Some(TIMEOUT))
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();

        let endpoint = url.to_string();

        Analytics::worker(move |body| {
            match agent
                .post(&endpoint)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .send(body)
            {
                Ok(response) if response.status().as_u16() == 200 => {}
                Ok(response) => tracing::warn!(
                    status = response.status().as_u16(),
                    "analytics event rejected"
                ),
                Err(_) => tracing::warn!("analytics delivery failed"),
            }
        })
    }
}

struct Delivery {
    sender: Option<SyncSender<Vec<u8>>>,
    stopped: Arc<AtomicBool>,
    finished: Mutex<mpsc::Receiver<()>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for Delivery {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);

        self.sender.take();

        let finished = self.finished.get_mut().unwrap_or_else(|e| e.into_inner());

        if finished.recv_timeout(Duration::from_secs(1)).is_ok() {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        } else {
            tracing::debug!("analytics shutdown wait timed out");
        }
    }
}

#[derive(Clone, Default)]

pub struct Analytics {
    delivery: Option<Arc<Delivery>>,
    user_id: Arc<str>,
    session_id: Arc<str>,
}

impl Analytics {
    #[cfg(not(target_arch = "wasm32"))]

    fn worker(mut send: impl FnMut(&[u8]) + Send + 'static) -> anyhow::Result<Self> {
        let session_id = session_id()?.into();

        let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(CAPACITY);

        let (done, finished) = mpsc::channel();

        let stopped = Arc::new(AtomicBool::new(false));

        let stop = stopped.clone();

        let worker = thread::Builder::new()
            .name("analytics".into())
            .spawn(move || {
                while let Ok(body) = receiver.recv() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    send(&body);
                }

                let _ = done.send(());
            })?;

        Ok(Self {
            delivery: Some(Arc::new(Delivery {
                sender: Some(sender),
                stopped,
                finished: Mutex::new(finished),
                worker: Some(worker),
            })),
            session_id,
            user_id: "".into(),
        })
    }

    pub(crate) fn identify(&mut self, account: &str) {
        self.user_id = account.to_ascii_uppercase().into();
    }

    pub(crate) fn login(&mut self, account: &str) {
        if self.delivery.is_none() {
            return;
        }

        self.identify(account);

        self.event("gameStarted", json!({}));

        self.event(
            "clientDevice",
            json!({
                "deviceType": "PC", "operatingSystem": "WINDOWS",
                "operatingSystemVersion": true, "addressMAC": true, "addressIP": true,
                "timezoneOffset": "-03:00"
            }),
        );
    }

    pub(crate) fn ended(&self) {
        self.event("gameEnded", json!({}));
    }

    pub(crate) fn level_up(&self, character: &Entered) {
        let mut params = character_params(character);

        params["levelUpName"] = json!("LEVEL_UP");

        params["userLevel"] = json!(character.base_status.level as i16);

        self.event("levelUp", params);
    }

    pub(crate) fn message(&self, character: &Entered, message: &Message) {
        if message
            .recipient
            .split('\0')
            .next()
            .unwrap_or_default()
            .is_empty()
            || message
                .text
                .split('\0')
                .next()
                .unwrap_or_default()
                .is_empty()
        {
            return;
        }

        let mut params = character_params(character);

        params["communicationType"] = json!(message.kind);

        params["uniqueTracking"] = json!("uniqueTracking");

        params["message"] = json!(message.text);

        params["recipients"] = json!([{ "recipientID": message.recipient }]);

        self.event("messageSent", params);
    }

    pub(crate) fn item_actioned(&self, character: &Entered, id: i32, name: &str) {
        let mut params = character_params(character);

        params["action"] = json!("USE");

        params["itemID"] = json!(id.to_string());

        params["itemName"] = json!(name);

        params["itemType"] = json!("ITEM");

        self.event("itemActioned", params);
    }

    pub(crate) fn item_collected(&self, character: &Entered, name: &str, amount: i32) {
        let mut params = character_params(character);

        params["reward"] = json!({"rewardName": "COLLECT_ITEM", "rewardProducts": {
            "items": [{"item": {"itemType": "GOLD", "itemName": name, "itemAmount": amount}}]
        }});

        self.event("itemCollected", params);
    }

    fn event(&self, name: &str, mut params: Value) {
        let Some(delivery) = &self.delivery else {
            return;
        };

        if self.user_id.is_empty() {
            return;
        }

        params["platform"] = json!("GLOBAL_SERVER");

        let timestamp = local_timestamp();

        let mut body = styled(&json!({"eventList": [{
            "eventName": name, "userID": self.user_id.as_ref(), "sessionID": self.session_id.as_ref(),
            "eventTimestamp": timestamp, "eventParams": params
        }]}));

        if body.len() > MAX_BYTES {
            body.truncate(MAX_BYTES);
        }

        if let Some(sender) = &delivery.sender
            && sender.try_send(body).is_err()
        {
            tracing::warn!(
                event = name,
                "analytics queue full or unavailable; event dropped"
            );
        }
    }

    #[cfg(test)]

    pub(crate) fn recording() -> (Self, mpsc::Receiver<Value>) {
        let (sender, receiver) = mpsc::channel();

        let mut analytics = Self::worker(move |body| {
            let _ = sender.send(serde_json::from_slice(body).unwrap());
        })
        .unwrap();

        analytics.user_id = "TEST_ACCOUNT".into();

        (analytics, receiver)
    }
}

#[cfg(not(target_arch = "wasm32"))]

fn session_id() -> anyhow::Result<String> {
    #[cfg(windows)]
    {
        let id = unsafe { windows::Win32::System::Com::CoCreateGuid() }?;

        Ok(format!("{id:?}"))
    }

    #[cfg(all(not(windows), not(target_arch = "wasm32")))]
    {
        let mut bytes = [0u8; 16];

        getrandom::fill(&mut bytes)
            .map_err(|_| anyhow::anyhow!("could not generate analytics session ID"))?;

        bytes[6] = (bytes[6] & 0x0F) | 0x40;

        bytes[8] = (bytes[8] & 0x3F) | 0x80;

        let value = u128::from_be_bytes(bytes);

        Ok(format!(
            "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
            value >> 96,
            (value >> 80) & 0xFFFF,
            (value >> 64) & 0xFFFF,
            (value >> 48) & 0xFFFF,
            value & 0xFFFF_FFFF_FFFF,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Message {
    pub kind: &'static str,
    pub recipient: String,
    pub text: String,
}

fn character_params(character: &Entered) -> Value {
    let arch = (character.equipment.body().index as i16) % 10 > 5;

    json!({
        "characterName": character.name(),
        "characterClass": (["TK", "FM", "BM", "HT"].get(usize::from(character.class)).unwrap_or(&"TK")),
        "characterTrans": if arch { "Arch" } else { "Mortal" }
    })
}

fn styled(value: &Value) -> Vec<u8> {
    fn write(value: &Value, depth: usize, out: &mut String) {
        let indent = |n: usize| " ".repeat(n * 3);

        match value {
            Value::Object(map) => {
                out.push('{');

                for (i, (key, value)) in map.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }

                    out.push_str(&indent(depth + 1));

                    quote(key, out);

                    out.push_str(" : ");

                    write(value, depth + 1, out);
                }

                if !map.is_empty() {
                    out.push_str(&indent(depth));
                }

                out.push('}');
            }
            Value::Array(array) => {
                out.push('[');

                for (i, value) in array.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }

                    out.push_str(&indent(depth + 1));

                    write(value, depth + 1, out);
                }

                if !array.is_empty() {
                    out.push_str(&indent(depth));
                }

                out.push(']');
            }
            Value::String(value) => quote(value, out),
            _ => out.push_str(&serde_json::to_string(value).unwrap()),
        }
    }

    let mut out = String::new();

    write(value, 0, &mut out);

    out.into_bytes()
}

fn quote(value: &str, out: &mut String) {
    use std::fmt::Write;

    out.push('"');

    for ch in value.chars().take_while(|&ch| ch != '\0') {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < ' ' => {
                let _ = write!(out, "\\u{:04X}", ch as u32);
            }
            ch => out.push(ch),
        }
    }

    out.push('"');
}

fn local_timestamp() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();

        return format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.000",
            now.get_full_year(),
            now.get_month() + 1,
            now.get_date(),
            now.get_hours(),
            now.get_minutes(),
            now.get_seconds()
        );
    }

    #[cfg(unix)]
    {
        let seconds = time::OffsetDateTime::now_utc().unix_timestamp() as libc::time_t;

        let mut local: libc::tm = unsafe { std::mem::zeroed() };

        if !unsafe { libc::localtime_r(&seconds, &mut local) }.is_null() {
            return format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.000",
                local.tm_year + 1900,
                local.tm_mon + 1,
                local.tm_mday,
                local.tm_hour,
                local.tm_min,
                local.tm_sec
            );
        }
    }

    #[cfg(windows)]
    {
        let local = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };

        return format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.000",
            local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond
        );
    }

    #[cfg(all(not(windows), not(target_arch = "wasm32")))]
    {
        let now = time::OffsetDateTime::now_utc();

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.000",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }
}

#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]

pub fn configured(config: &Config) -> anyhow::Result<Analytics> {
    config.endpoint()?;

    Ok(Analytics::default())
}
