use crate::mixer::{CHANNELS, Mixer};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const IDLE_WAIT: Duration = Duration::from_millis(1);

fn block_frames(rate: u32) -> usize {
    (rate.div_ceil(500) as usize).clamp(1, 256)
}

fn capacity(rate: u32, callback_frames: usize) -> usize {
    (rate.div_ceil(50) as usize).max(callback_frames.saturating_add(block_frames(rate)))
}

struct Ring {
    frames: Box<[AtomicU64]>,
    read: AtomicUsize,
    write: AtomicUsize,
}

impl Ring {
    fn new(capacity: usize) -> Self {
        Self {
            frames: (0..capacity.max(1) + 1)
                .map(|_| AtomicU64::new(0))
                .collect(),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    fn remaining(&self) -> usize {
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Relaxed);
        if write >= read {
            self.frames.len() - 1 - (write - read)
        } else {
            read - write - 1
        }
    }

    fn push(&self, frame: [f32; CHANNELS]) -> bool {
        let write = self.write.load(Ordering::Relaxed);
        let next = (write + 1) % self.frames.len();
        if next == self.read.load(Ordering::Acquire) {
            return false;
        }

        let packed = u64::from(frame[0].to_bits()) | (u64::from(frame[1].to_bits()) << 32);
        self.frames[write].store(packed, Ordering::Relaxed);
        self.write.store(next, Ordering::Release);
        true
    }

    fn pop(&self) -> Option<[f32; CHANNELS]> {
        let read = self.read.load(Ordering::Relaxed);
        if read == self.write.load(Ordering::Acquire) {
            return None;
        }

        let packed = self.frames[read].load(Ordering::Relaxed);
        self.read
            .store((read + 1) % self.frames.len(), Ordering::Release);
        Some([
            f32::from_bits(packed as u32),
            f32::from_bits((packed >> 32) as u32),
        ])
    }
}

pub(super) struct Consumer {
    ring: Arc<Ring>,
}

impl Consumer {
    pub(super) fn fill<T: Copy + Default>(
        &self,
        output: &mut [T],
        channels: usize,
        convert: impl Fn(f32) -> T,
    ) {
        output.fill(T::default());
        if channels == 0 {
            return;
        }

        for frame in output.chunks_exact_mut(channels) {
            let Some([left, right]) = self.ring.pop() else {
                break;
            };
            if channels == 1 {
                frame[0] = convert((left + right) * 0.5);
            } else {
                frame[0] = convert(left);
                frame[1] = convert(right);
            }
        }
    }
}

fn mix_block(
    mixer: &Mutex<Mixer>,
    output: &mut [f32],
    mut should_stop: impl FnMut() -> bool,
) -> bool {
    'prepare: loop {
        if should_stop() {
            return false;
        }
        let mut buffered = match mixer.try_lock() {
            Ok(mut mixer) => match mixer.take_music_buffer(output.len() / CHANNELS) {
                Some(buffered) => buffered,
                None => {
                    mixer.fill(output);
                    return true;
                }
            },
            Err(TryLockError::Poisoned(_)) => {
                output.fill(0.0);
                return true;
            }
            Err(TryLockError::WouldBlock) => {
                thread::park_timeout(IDLE_WAIT);
                continue;
            }
        };

        if !buffered.buffer(&mut should_stop) {
            continue;
        }

        loop {
            if should_stop() {
                return false;
            }
            match mixer.try_lock() {
                Ok(mut mixer) => match mixer.restore_music_buffer(buffered) {
                    None => {
                        mixer.fill(output);
                        return true;
                    }
                    Some(stale) => {
                        drop(mixer);
                        drop(stale);
                        continue 'prepare;
                    }
                },
                Err(TryLockError::Poisoned(_)) => {
                    output.fill(0.0);
                    return true;
                }
                Err(TryLockError::WouldBlock) => thread::park_timeout(IDLE_WAIT),
            }
        }
    }
}

pub(super) struct Producer {
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Producer {
    pub(super) fn spawn(
        mixer: Arc<Mutex<Mixer>>,
        rate: u32,
        callback_frames: usize,
    ) -> std::io::Result<(Self, Consumer)> {
        let ring = Arc::new(Ring::new(capacity(rate, callback_frames)));
        let stopped = Arc::new(AtomicBool::new(false));
        let producer_ring = Arc::clone(&ring);
        let producer_stopped = Arc::clone(&stopped);
        let block_frames = block_frames(rate);
        let worker = thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || {
                let mut scratch = vec![0.0; block_frames * CHANNELS];

                while !producer_stopped.load(Ordering::Acquire) {
                    let frames = producer_ring.remaining().min(block_frames);
                    if frames == 0 {
                        thread::park_timeout(IDLE_WAIT);
                        continue;
                    }

                    let output = &mut scratch[..frames * CHANNELS];
                    if !mix_block(&mixer, output, || producer_stopped.load(Ordering::Acquire)) {
                        return;
                    }

                    for frame in output.chunks_exact(CHANNELS) {
                        if !producer_ring.push([frame[0], frame[1]]) {
                            break;
                        }
                    }
                }
            })?;

        Ok((
            Self {
                stopped,
                worker: Some(worker),
            },
            Consumer { ring },
        ))
    }

    pub(super) fn stop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
