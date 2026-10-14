use super::*;
use std::sync::mpsc;
use web_time::{Duration, Instant};

type Key = ((u8, u8), Scale);

type Image = (Vec<Tile>, Texture);

#[derive(Debug)]
struct Worker {
    requests: mpsc::SyncSender<Key>,
    replies: mpsc::Receiver<(Key, Image)>,
}

#[derive(Debug)]
pub(super) struct Loader {
    game: GameDir,
    worker: Option<Worker>,
    pending: bool,
    retry: Instant,
}

impl Loader {
    pub fn new(game: GameDir) -> Self {
        Self {
            game,
            worker: None,
            pending: false,
            retry: Instant::now(),
        }
    }

    pub fn poll(&mut self, zone: (u8, u8), scale: Scale) -> Option<Image> {
        if self.worker.is_none() {
            if Instant::now() < self.retry {
                return None;
            }

            let game = self.game.clone();

            let (requests, work) = mpsc::sync_channel::<Key>(1);

            let (done, replies) = mpsc::sync_channel(1);

            if let Err(error) = crate::background::spawn("nwyd-minimap", work, done, move |key| {
                let image = compose(&game, key.0, key.1);

                (key, image)
            }) {
                tracing::warn!(% error, "minimap worker could not start; will retry");

                self.retry = Instant::now() + Duration::from_secs(1);

                return None;
            }

            self.worker = Some(Worker { requests, replies });
        }

        match self.worker.as_ref().unwrap().replies.try_recv() {
            Ok((key, image)) => {
                self.pending = false;

                if key == (zone, scale) {
                    return Some(image);
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker = None;

                self.pending = false;

                self.retry = Instant::now() + Duration::from_secs(1);

                return None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if !self.pending {
            if self
                .worker
                .as_ref()
                .unwrap()
                .requests
                .try_send((zone, scale))
                .is_ok()
            {
                self.pending = true;
            } else {
                self.worker = None;

                self.retry = Instant::now() + Duration::from_secs(1);
            }
        }

        None
    }
}
