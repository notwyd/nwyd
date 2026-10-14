use super::{Got, Want};
use std::{cell::RefCell, collections::VecDeque, sync::mpsc, time::Duration};

struct Work {
    want: Want,
    timeout: Duration,
    done: mpsc::Sender<Got>,
}

#[derive(Default)]
struct Pool {
    jobs: Vec<Work>,
    completed: VecDeque<(mpsc::Sender<Got>, Got)>,
    running: usize,
}

thread_local! {
    static POOL: RefCell<Pool> = RefCell::new(Pool::default());
}

pub(super) struct Sender {
    timeout: Duration,
    done: mpsc::Sender<Got>,
}

impl Sender {
    pub(super) fn send(&self, want: Want) -> Result<(), Want> {
        POOL.with(|pool| {
            let mut pool = pool.borrow_mut();

            if pool.jobs.len() >= 128 {
                return Err(want);
            }

            pool.jobs.push(Work {
                want,
                timeout: self.timeout,
                done: self.done.clone(),
            });

            Ok(())
        })
    }
}

pub(super) fn sender(timeout: Duration, done: mpsc::Sender<Got>) -> Sender {
    Sender { timeout, done }
}

pub(super) fn pump() {
    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();

        for _ in 0..8 {
            let Some((done, got)) = pool.completed.pop_front() else {
                break;
            };

            let _ = done.send(got);
        }

        while pool.running < 3 {
            let Some(Work {
                want,
                timeout,
                done,
            }) = pool.jobs.pop()
            else {
                break;
            };

            pool.running += 1;

            wasm_bindgen_futures::spawn_local(async move {
                let pixels = match super::fetch_async(&want.url, timeout).await {
                    Ok(bytes) => super::decode(&bytes),
                    Err(error) => {
                        tracing::debug!(url = %want.url, ?error, "guildmark download failed");

                        None
                    }
                };

                let mut got = Got {
                    slot: want.slot,
                    key: want.key,
                    pixels,
                };

                POOL.with(|pool| {
                    let mut pool = pool.borrow_mut();

                    pool.running -= 1;

                    if pool.completed.len() >= 64 {
                        got.pixels = None;

                        let _ = done.send(got);
                    } else {
                        pool.completed.push_back((done, got));
                    }
                });
            });
        }
    });
}
