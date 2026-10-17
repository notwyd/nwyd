use crate::device::Gpu;
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const SLOT_COUNT: usize = 4;
const QUERY_COUNT: u32 = 4;
const QUERY_BYTES: u64 = QUERY_COUNT as u64 * 8;
const IDLE: u8 = 0;
const RECORDING: u8 = 1;
const RESOLVED: u8 = 2;
const PENDING: u8 = 3;
const READY: u8 = 4;
const FAILED: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GpuTimingPoint {
    SceneStart,
    SceneEnd,
    BlitStart,
    BlitEnd,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuFrameTiming {
    pub frame_id: u64,
    pub scene_ms: f64,
    pub blit_ms: f64,
}

struct Slot {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    frame_id: u64,
}

pub struct GpuTimings {
    slots: [Slot; SLOT_COUNT],
    states: [Arc<AtomicU8>; SLOT_COUNT],
    period_ns: f64,
}

pub struct GpuTimingFrame {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    slot: usize,
    state: Arc<AtomicU8>,
    points: Cell<u8>,
}

impl GpuTimings {
    pub fn new(gpu: &Gpu) -> Option<Self> {
        let required =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let period_ns = f64::from(gpu.queue.get_timestamp_period());
        if !gpu.device.features().contains(required) || !period_ns.is_finite() || period_ns <= 0.0 {
            return None;
        }
        Some(Self {
            slots: std::array::from_fn(|_| Slot {
                query_set: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: crate::gpu_label("frame timing"),
                    ty: wgpu::QueryType::Timestamp,
                    count: QUERY_COUNT,
                }),
                resolve: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: crate::gpu_label("frame timing resolve"),
                    size: QUERY_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: crate::gpu_label("frame timing readback"),
                    size: QUERY_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                frame_id: 0,
            }),
            states: std::array::from_fn(|_| Arc::new(AtomicU8::new(IDLE))),
            period_ns,
        })
    }

    pub fn begin_frame(&mut self, frame_id: u64) -> Option<GpuTimingFrame> {
        let slot = reserve_slot(&self.states)?;
        self.slots[slot].frame_id = frame_id;
        Some(GpuTimingFrame {
            query_set: self.slots[slot].query_set.clone(),
            resolve: self.slots[slot].resolve.clone(),
            readback: self.slots[slot].readback.clone(),
            slot,
            state: Arc::clone(&self.states[slot]),
            points: Cell::new(0),
        })
    }

    pub fn finish_frame(&mut self, frame: GpuTimingFrame) {
        if !Arc::ptr_eq(&frame.state, &self.states[frame.slot])
            || frame
                .state
                .compare_exchange(RESOLVED, PENDING, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }
        let state = Arc::clone(&frame.state);
        frame
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                state.store(
                    if result.is_ok() { READY } else { FAILED },
                    Ordering::Release,
                );
            });
    }

    pub fn has_pending(&self) -> bool {
        self.states
            .iter()
            .any(|state| state.load(Ordering::Acquire) == PENDING)
    }

    pub fn poll(&mut self, gpu: &Gpu) -> Vec<GpuFrameTiming> {
        if !self
            .states
            .iter()
            .any(|state| state.load(Ordering::Acquire) >= PENDING)
        {
            return Vec::new();
        }
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let mut completed = Vec::new();
        for (slot, state) in self.slots.iter().zip(&self.states) {
            match state.load(Ordering::Acquire) {
                READY => {
                    if let Ok(mapped) = slot.readback.slice(..).get_mapped_range() {
                        let timestamps = std::array::from_fn(|index| {
                            u64::from_ne_bytes(mapped[index * 8..index * 8 + 8].try_into().unwrap())
                        });
                        if let Some(sample) = sample(slot.frame_id, timestamps, self.period_ns) {
                            completed.push(sample);
                        }
                    }
                    slot.readback.unmap();
                    state.store(IDLE, Ordering::Release);
                }
                FAILED => {
                    slot.readback.unmap();
                    state.store(IDLE, Ordering::Release);
                }
                _ => {}
            }
        }
        completed.sort_by_key(|sample| sample.frame_id);
        completed
    }
}

impl GpuTimingFrame {
    pub fn write(&self, encoder: &mut wgpu::CommandEncoder, point: GpuTimingPoint) {
        if self.state.load(Ordering::Acquire) != RECORDING {
            return;
        }
        let index = point as u32;
        let bit = 1 << index;
        if self.points.get() & bit != 0 {
            return;
        }
        encoder.write_timestamp(&self.query_set, index);
        self.points.set(self.points.get() | bit);
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) -> bool {
        if self.points.get() != (1 << QUERY_COUNT) - 1
            || self.state.load(Ordering::Acquire) != RECORDING
        {
            return false;
        }
        encoder.resolve_query_set(&self.query_set, 0..QUERY_COUNT, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, QUERY_BYTES);
        self.state.store(RESOLVED, Ordering::Release);
        true
    }
}

impl Drop for GpuTimingFrame {
    fn drop(&mut self) {
        if matches!(self.state.load(Ordering::Acquire), RECORDING | RESOLVED) {
            self.state.store(IDLE, Ordering::Release);
        }
    }
}

fn reserve_slot(states: &[Arc<AtomicU8>]) -> Option<usize> {
    states.iter().position(|state| {
        state
            .compare_exchange(IDLE, RECORDING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    })
}

fn sample(frame_id: u64, timestamps: [u64; 4], period_ns: f64) -> Option<GpuFrameTiming> {
    if !period_ns.is_finite()
        || period_ns <= 0.0
        || timestamps.windows(2).any(|pair| pair[0] > pair[1])
    {
        return None;
    }
    Some(GpuFrameTiming {
        frame_id,
        scene_ms: (timestamps[1] - timestamps[0]) as f64 * period_ns / 1_000_000.0,
        blit_ms: (timestamps[3] - timestamps[2]) as f64 * period_ns / 1_000_000.0,
    })
}

#[cfg(test)]
mod tests;
