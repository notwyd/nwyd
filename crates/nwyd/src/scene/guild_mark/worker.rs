use super::{Got, Want};
use std::{
    sync::{Arc, Condvar, Mutex, OnceLock, mpsc},
    time::Duration,
};

#[derive(Default)]

struct Transport {
    #[cfg(not(windows))]
    agent: Option<ureq::Agent>,
}

struct Stack<T> {
    values: Mutex<Vec<T>>,
    ready: Condvar,
    limit: usize,
}

impl<T> Stack<T> {
    fn new(limit: usize) -> Self {
        Self {
            values: Mutex::new(Vec::new()),
            ready: Condvar::new(),
            limit,
        }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let mut values = self.values.lock().unwrap_or_else(|p| p.into_inner());

        if values.len() == self.limit {
            return Err(value);
        }

        values.push(value);

        self.ready.notify_one();

        Ok(())
    }

    fn pop(&self) -> T {
        let mut values = self.values.lock().unwrap_or_else(|p| p.into_inner());

        loop {
            if let Some(value) = values.pop() {
                return value;
            }

            values = self.ready.wait(values).unwrap_or_else(|p| p.into_inner());
        }
    }

    fn try_pop(&self) -> Option<T> {
        self.values.lock().unwrap_or_else(|p| p.into_inner()).pop()
    }
}

struct Work {
    want: Want,
    timeout: Duration,
    done: mpsc::Sender<Got>,
}

struct Completed {
    got: Got,
    done: mpsc::Sender<Got>,
}

struct Pool {
    jobs: Arc<Stack<Work>>,
    completed: Arc<Stack<Completed>>,
}

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();

    POOL.get_or_init(|| {
        let jobs = Arc::new(Stack::<Work>::new(128));

        let completed = Arc::new(Stack::<Completed>::new(64));

        for _ in 0..3 {
            let jobs = jobs.clone();

            let completed = completed.clone();

            std::thread::spawn(move || {
                let mut agent = Transport::default();

                loop {
                    let Work {
                        want,
                        timeout,
                        done,
                    } = jobs.pop();

                    let pixels = match fetch(&want.url, timeout, &mut agent) {
                        Ok(bytes) => super::decode(&bytes),
                        Err(error) => {
                            tracing::debug!(
                                url = % want.url, ? error, "guildmark download failed"
                            );

                            None
                        }
                    };

                    let got = Got {
                        slot: want.slot,
                        key: want.key,
                        pixels,
                    };

                    if let Err(mut overflow) = completed.push(Completed { got, done }) {
                        overflow.got.pixels = None;

                        let _ = overflow.done.send(overflow.got);
                    }
                }
            });
        }

        Pool { jobs, completed }
    })
}

fn fetch(url: &str, timeout: Duration, retained: &mut Transport) -> Result<Vec<u8>, super::Error> {
    #[cfg(windows)]
    {
        let _ = (timeout, retained);

        return super::fetch_native(url);
    }

    #[cfg(not(windows))]
    {
        let agent = retained
            .agent
            .get_or_insert_with(|| super::badge_agent(ureq::tls::TlsConfig::default()));

        super::fetch_with_agent(url, timeout, agent)
    }
}

pub(super) struct Sender {
    timeout: Duration,
    done: mpsc::Sender<Got>,
}

impl Sender {
    pub(super) fn send(&self, want: Want) -> Result<(), Want> {
        pool()
            .jobs
            .push(Work {
                want,
                timeout: self.timeout,
                done: self.done.clone(),
            })
            .map_err(|work| work.want)
    }
}

pub(super) fn sender(timeout: Duration, done: mpsc::Sender<Got>) -> Sender {
    pool();

    Sender { timeout, done }
}

pub(super) fn pump() {
    drain_frame(&pool().completed, |completed| {
        let _ = completed.done.send(completed.got);
    });
}

fn drain_frame<T>(stack: &Stack<T>, mut consume: impl FnMut(T)) {
    for _ in 0..8 {
        let Some(value) = stack.try_pop() else {
            break;
        };

        consume(value);
    }
}
