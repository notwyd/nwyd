use super::Key;
use formats::texture::Texture;
use render::texture::PreparedMaterial;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use web_time::{Duration, Instant};

#[cfg(test)]
mod tests;

struct Request {
    key: Key,
    generation: u64,
    source: Arc<Texture>,
}

type Reply = (Key, u64, Arc<PreparedMaterial>);

struct Worker {
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Reply>,
}

impl Worker {
    fn new() -> std::io::Result<Self> {
        let (requests, work) = mpsc::sync_channel::<Request>(1);
        let (done, replies) = mpsc::sync_channel(1);

        crate::background::spawn("nwyd-material-preparation", work, done, |request| {
            let start = Instant::now();
            let prepared = Arc::new(PreparedMaterial::from_shared(request.source, request.key.3));

            tracing::debug!(
                elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                "material background preparation completed"
            );

            (request.key, request.generation, prepared)
        })?;

        Ok(Self { requests, replies })
    }
}

pub(super) struct Preparation {
    worker: Option<Worker>,
    pending: Option<(Key, u64)>,
    ready: HashMap<Key, Arc<PreparedMaterial>>,
    generation: u64,
    restart: Instant,
}

impl Default for Preparation {
    fn default() -> Self {
        Self {
            worker: None,
            pending: None,
            ready: HashMap::new(),
            generation: 0,
            restart: Instant::now(),
        }
    }
}

impl Preparation {
    pub(super) fn poll(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };

        match worker.replies.try_recv() {
            Ok((key, generation, prepared)) => {
                if self.pending.as_ref() != Some(&(key.clone(), generation)) {
                    return;
                }

                self.pending = None;

                if generation == self.generation && prepared.policy() == key.3 {
                    self.ready.insert(key, prepared);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.disconnected(),
        }
    }

    pub(super) fn get(&self, key: &Key) -> Option<Arc<PreparedMaterial>> {
        self.ready.get(key).map(Arc::clone)
    }

    #[cfg(test)]
    pub(super) fn take(&mut self, key: &Key) -> Option<Arc<PreparedMaterial>> {
        self.ready.remove(key)
    }

    pub(super) fn request(&mut self, key: &Key, source: &Arc<Texture>) -> bool {
        if self.ready.contains_key(key) {
            return true;
        }

        if let Some((pending, generation)) = &self.pending {
            return pending == key && *generation == self.generation;
        }

        if !self.ready.is_empty() {
            return false;
        }

        if self.worker.is_none() {
            if Instant::now() < self.restart {
                return false;
            }

            match Worker::new() {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.restart = Instant::now() + Duration::from_secs(1);

                    tracing::warn!(%error, "material worker could not start; will retry");

                    return false;
                }
            }
        }

        self.generation = self.generation.wrapping_add(1);

        let request = Request {
            key: key.clone(),
            generation: self.generation,
            source: Arc::clone(source),
        };

        match self.worker.as_ref().unwrap().requests.try_send(request) {
            Ok(()) => {
                self.pending = Some((key.clone(), self.generation));

                true
            }
            Err(mpsc::TrySendError::Full(_)) => false,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.disconnected();

                false
            }
        }
    }

    pub(super) fn invalidate(&mut self, key: &Key) {
        if self
            .pending
            .as_ref()
            .is_some_and(|(pending, _)| pending == key)
        {
            self.generation = self.generation.wrapping_add(1);
        }

        self.ready.remove(key);
    }

    pub(super) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);

        self.worker = None;
        self.pending = None;
        self.ready.clear();
        self.restart = Instant::now();
    }

    fn disconnected(&mut self) {
        self.worker = None;
        self.pending = None;
        self.restart = Instant::now() + Duration::from_secs(1);
    }
}
