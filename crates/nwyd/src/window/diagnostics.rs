#[cfg(any(not(target_arch = "wasm32"), test))]
use anyhow::{Context, Result};
use serde::Serialize;
#[cfg(any(not(target_arch = "wasm32"), test))]
use std::collections::BTreeMap;
use std::collections::VecDeque;
#[cfg(any(not(target_arch = "wasm32"), test))]
use std::io::Write;
#[cfg(any(not(target_arch = "wasm32"), test))]
use std::path::PathBuf;

const RECENT_SAMPLES: usize = 120;
#[cfg(any(not(target_arch = "wasm32"), test))]
const MAX_BENCHMARK_FRAMES: u64 = 100_000;
#[cfg(any(not(target_arch = "wasm32"), test))]
const CPU_STAGES: [&str; 8] = [
    "update_ms",
    "gap_ms",
    "acquire_ms",
    "prepare_ms",
    "submit_ms",
    "blit_ms",
    "present_ms",
    "total_ms",
];

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub(super) struct CpuFrameTiming {
    pub frame_id: u64,
    pub focused: bool,
    pub continuous: bool,
    pub surface: [u32; 2],
    pub update_ms: f64,
    pub gap_ms: f64,
    pub acquire_ms: f64,
    pub prepare_ms: f64,
    pub submit_ms: f64,
    pub blit_ms: f64,
    pub present_ms: f64,
    pub total_ms: f64,
}

impl CpuFrameTiming {
    fn values(self) -> [f64; 8] {
        [
            self.update_ms,
            self.gap_ms,
            self.acquire_ms,
            self.prepare_ms,
            self.submit_ms,
            self.blit_ms,
            self.present_ms,
            self.total_ms,
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
struct GpuSample {
    frame_id: u64,
    scene_ms: f64,
    blit_ms: f64,
}

#[derive(Debug)]
struct Benchmark {
    warmup: u64,
    frames: u64,
    gpu_timings: bool,
    #[cfg(any(not(target_arch = "wasm32"), test))]
    output: PathBuf,
}

#[derive(Default)]
pub(super) struct FrameDiagnostics {
    enabled: bool,
    benchmark: Option<Benchmark>,
    context: serde_json::Value,
    cpu: VecDeque<CpuFrameTiming>,
    gpu: VecDeque<GpuSample>,
    rejected_cpu: u64,
    rejected_gpu: u64,
}

impl FrameDiagnostics {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("NWYD_FRAME_TIMINGS").is_ok_and(|value| value == "1"),
            ..Self::default()
        }
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn benchmark(warmup: u64, frames: u64, output: PathBuf, gpu_timings: bool) -> Self {
        let frames = frames.clamp(1, MAX_BENCHMARK_FRAMES);

        Self {
            enabled: true,
            benchmark: Some(Benchmark {
                warmup: warmup.min(u64::MAX - frames),
                frames,
                gpu_timings,
                output,
            }),
            ..Self::default()
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn gpu_enabled(&self) -> bool {
        self.enabled
            && self
                .benchmark
                .as_ref()
                .is_none_or(|benchmark| benchmark.gpu_timings)
    }

    pub fn benchmark_time(&self, frame_id: u64) -> Option<u32> {
        self.benchmark.as_ref().map(|_| {
            10_000_u64
                .saturating_add(frame_id.saturating_mul(16))
                .min(u64::from(u32::MAX)) as u32
        })
    }

    pub fn done(&self, presented_frames: u64) -> bool {
        self.benchmark
            .as_ref()
            .is_some_and(|benchmark| presented_frames >= benchmark.warmup + benchmark.frames)
    }

    pub fn set_context(&mut self, context: serde_json::Value) {
        self.context = context;
    }

    pub fn set_gpu_timestamps_active(&mut self, active: bool) {
        self.context["gpu_timestamps"] = active.into();
    }

    fn accepts(&self, frame_id: u64) -> bool {
        self.enabled
            && self.benchmark.as_ref().is_none_or(|benchmark| {
                frame_id >= benchmark.warmup && frame_id - benchmark.warmup < benchmark.frames
            })
    }

    fn capacity(&self) -> usize {
        self.benchmark
            .as_ref()
            .map_or(RECENT_SAMPLES, |benchmark| benchmark.frames as usize)
    }

    pub fn record(&mut self, sample: CpuFrameTiming) {
        if !self.accepts(sample.frame_id) {
            return;
        }

        if !valid(sample.values()) {
            self.rejected_cpu += 1;

            return;
        }

        let capacity = self.capacity();

        retain(&mut self.cpu, sample, capacity);
    }

    pub fn record_gpu(&mut self, sample: render::timing::GpuFrameTiming) {
        if !self.gpu_enabled() || !self.accepts(sample.frame_id) {
            return;
        }

        if !valid([sample.scene_ms, sample.blit_ms]) {
            self.rejected_gpu += 1;

            return;
        }

        let sample = GpuSample {
            frame_id: sample.frame_id,
            scene_ms: sample.scene_ms,
            blit_ms: sample.blit_ms,
        };

        let capacity = self.capacity();

        retain(&mut self.gpu, sample, capacity);
    }

    pub fn lines(&self) -> Vec<crate::overlay::Line> {
        use crate::overlay::{BODY_COLOR, Line};

        if !self.enabled {
            return Vec::new();
        }

        let mut lines = Vec::new();

        if self.cpu.is_empty() {
            lines.push(Line::new("CPU: measuring frame stages", BODY_COLOR));
        } else {
            let averages: [f64; 8] = std::array::from_fn(|index| {
                mean(
                    self.cpu
                        .iter()
                        .rev()
                        .take(RECENT_SAMPLES)
                        .map(|sample| sample.values()[index]),
                )
            });

            lines.push(Line::new(
                format!(
                    "CPU ms: update {:.2}  gap {:.2}  acquire {:.2}  prepare {:.2}",
                    averages[0], averages[1], averages[2], averages[3]
                ),
                BODY_COLOR,
            ));

            lines.push(Line::new(
                format!(
                    "CPU ms: submit {:.2}  blit {:.2}  present {:.2}  total {:.2}",
                    averages[4], averages[5], averages[6], averages[7]
                ),
                BODY_COLOR,
            ));
        }

        if self.gpu.is_empty() {
            lines.push(Line::new("GPU: no timestamp samples", BODY_COLOR));
        } else {
            let scene = mean(
                self.gpu
                    .iter()
                    .rev()
                    .take(RECENT_SAMPLES)
                    .map(|sample| sample.scene_ms),
            );

            let blit = mean(
                self.gpu
                    .iter()
                    .rev()
                    .take(RECENT_SAMPLES)
                    .map(|sample| sample.blit_ms),
            );

            lines.push(Line::new(
                format!("GPU ms: scene {scene:.2}  blit {blit:.2}"),
                BODY_COLOR,
            ));
        }

        lines
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn write_report(&self) -> Result<()> {
        let Some(benchmark) = &self.benchmark else {
            return Ok(());
        };

        let mut cpu: Vec<_> = self.cpu.iter().collect();

        cpu.sort_unstable_by_key(|sample| sample.frame_id);

        let mut gpu: Vec<_> = self.gpu.iter().collect();

        gpu.sort_unstable_by_key(|sample| sample.frame_id);

        let cpu_summary: BTreeMap<_, _> = CPU_STAGES
            .iter()
            .enumerate()
            .map(|(index, &name)| {
                (
                    name,
                    statistics(cpu.iter().map(|sample| sample.values()[index])),
                )
            })
            .collect();

        let report = serde_json::json!({
            "schema_version": 1,
            "completed": cpu.len() as u64 == benchmark.frames,
            "missing_gpu_samples": (self.context["gpu_timestamps"] == true)
                .then(|| cpu.len().saturating_sub(gpu.len())),
            "context": self.context,
            "benchmark": {
                "warmup_frames": benchmark.warmup,
                "frames": benchmark.frames,
                "gpu_timings_requested": benchmark.gpu_timings,
            },
            "rejected_samples": { "cpu": self.rejected_cpu, "gpu": self.rejected_gpu },
            "cpu": cpu,
            "gpu": gpu,
            "summary": {
                "cpu": cpu_summary,
                "gpu": {
                    "scene_ms": statistics(gpu.iter().map(|sample| sample.scene_ms)),
                    "blit_ms": statistics(gpu.iter().map(|sample| sample.blit_ms)),
                },
            },
        });

        if let Some(parent) = benchmark
            .output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating frame report directory {}", parent.display()))?;
        }

        let file = std::fs::File::create(&benchmark.output)
            .with_context(|| format!("creating frame report {}", benchmark.output.display()))?;

        let mut writer = std::io::BufWriter::new(file);

        serde_json::to_writer_pretty(&mut writer, &report).context("serializing frame report")?;

        writer.write_all(b"\n")?;

        writer.flush().context("writing frame report")
    }
}

fn valid<const N: usize>(values: [f64; N]) -> bool {
    values
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
}

fn retain<T>(samples: &mut VecDeque<T>, sample: T, capacity: usize) {
    if samples.len() == capacity {
        samples.pop_front();
    }

    samples.push_back(sample);
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    values.enumerate().fold(0.0, |mean, (index, value)| {
        mean + (value - mean) / (index + 1) as f64
    })
}

#[cfg(any(not(target_arch = "wasm32"), test))]
#[derive(Debug, Serialize)]
struct Statistics {
    count: usize,
    mean: f64,
    median: f64,
    p95: f64,
    p99: f64,
    max: f64,
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn statistics(values: impl Iterator<Item = f64>) -> Option<Statistics> {
    let mut values: Vec<_> = values.collect();

    if values.is_empty() {
        return None;
    }

    values.sort_unstable_by(f64::total_cmp);

    let low = values[(values.len() - 1) / 2];

    let high = values[values.len() / 2];

    let percentile = |p: f64| values[(values.len() as f64 * p).ceil() as usize - 1];

    Some(Statistics {
        count: values.len(),
        mean: mean(values.iter().copied()),
        median: low + (high - low) * 0.5,
        p95: percentile(0.95),
        p99: percentile(0.99),
        max: *values.last().unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu(frame_id: u64) -> CpuFrameTiming {
        CpuFrameTiming {
            frame_id,
            focused: true,
            continuous: false,
            surface: [1280, 720],
            update_ms: 1.0,
            gap_ms: 2.0,
            acquire_ms: 3.0,
            prepare_ms: 4.0,
            submit_ms: 5.0,
            blit_ms: 6.0,
            present_ms: 7.0,
            total_ms: 28.0,
        }
    }

    fn gpu(frame_id: u64) -> render::timing::GpuFrameTiming {
        render::timing::GpuFrameTiming {
            frame_id,
            scene_ms: 4.0,
            blit_ms: 0.5,
        }
    }

    #[test]
    fn benchmark_filters_warmup_and_late_samples_using_frame_identity() {
        let mut diagnostics = FrameDiagnostics::benchmark(2, 3, PathBuf::new(), true);

        for frame_id in [0, 4, 1, 5, 3, 2, 6] {
            diagnostics.record(cpu(frame_id));

            diagnostics.record_gpu(gpu(frame_id));
        }

        assert_eq!(
            diagnostics
                .cpu
                .iter()
                .map(|sample| sample.frame_id)
                .collect::<Vec<_>>(),
            [4, 3, 2]
        );

        assert_eq!(
            diagnostics
                .gpu
                .iter()
                .map(|sample| sample.frame_id)
                .collect::<Vec<_>>(),
            [4, 3, 2]
        );

        assert!(!diagnostics.done(4));

        assert!(diagnostics.done(5));

        assert!(diagnostics.done(6));

        assert_eq!(diagnostics.benchmark_time(0), Some(10_000));

        assert_eq!(diagnostics.benchmark_time(4), Some(10_064));
    }

    #[test]
    fn benchmark_bounds_requested_frames_and_timing_arithmetic() {
        let zero = FrameDiagnostics::benchmark(0, 0, PathBuf::new(), true);

        assert!(!zero.done(0));

        assert!(zero.done(1));

        let large = FrameDiagnostics::benchmark(0, u64::MAX, PathBuf::new(), true);

        assert!(!large.done(MAX_BENCHMARK_FRAMES - 1));

        assert!(large.done(MAX_BENCHMARK_FRAMES));

        assert_eq!(large.benchmark_time(u64::MAX), Some(u32::MAX));

        let overflow = FrameDiagnostics::benchmark(u64::MAX, 1, PathBuf::new(), true);

        assert!(!overflow.done(u64::MAX - 1));

        assert!(overflow.done(u64::MAX));
    }

    #[test]
    fn regular_recording_is_bounded_and_disabled_recording_is_empty() {
        let mut diagnostics = FrameDiagnostics {
            enabled: true,
            ..Default::default()
        };

        let mut disabled = FrameDiagnostics::default();

        for frame_id in 0..150 {
            diagnostics.record(cpu(frame_id));

            diagnostics.record_gpu(gpu(frame_id));

            disabled.record(cpu(frame_id));

            disabled.record_gpu(gpu(frame_id));
        }

        assert_eq!(diagnostics.cpu.len(), RECENT_SAMPLES);

        assert_eq!(diagnostics.gpu.len(), RECENT_SAMPLES);

        assert_eq!(diagnostics.cpu.front().unwrap().frame_id, 30);

        assert_eq!(diagnostics.gpu.front().unwrap().frame_id, 30);

        assert!(!diagnostics.done(u64::MAX));

        assert_eq!(diagnostics.benchmark_time(1), None);

        assert_eq!(diagnostics.lines().len(), 3);

        assert!(!disabled.enabled());

        assert!(disabled.cpu.is_empty());

        assert!(disabled.gpu.is_empty());

        assert!(disabled.lines().is_empty());

        disabled.write_report().unwrap();
    }

    #[test]
    fn statistics_use_median_and_nearest_rank_percentiles() {
        let stats = statistics([4.0, 1.0, 3.0, 2.0].into_iter()).unwrap();

        assert_eq!(stats.count, 4);

        assert_eq!(stats.mean, 2.5);

        assert_eq!(stats.median, 2.5);

        assert_eq!(stats.p95, 4.0);

        assert_eq!(stats.p99, 4.0);

        assert_eq!(stats.max, 4.0);

        assert!(statistics(std::iter::empty()).is_none());
    }

    #[test]
    fn reports_preserve_cpu_samples_when_gpu_timings_are_disabled_or_unavailable() {
        let output =
            std::env::temp_dir().join(format!("nwyd-gpu-timing-modes-{}.json", std::process::id()));

        for (requested, active) in [(false, false), (true, false), (true, true)] {
            let mut diagnostics = FrameDiagnostics::benchmark(1, 2, output.clone(), requested);

            diagnostics.set_context(serde_json::json!({ "gpu_timestamps": active }));

            for frame_id in 0..3 {
                diagnostics.record(cpu(frame_id));

                if active || !requested {
                    diagnostics.record_gpu(gpu(frame_id));
                }
            }

            assert!(diagnostics.enabled());

            assert_eq!(diagnostics.gpu_enabled(), requested);

            assert_eq!(diagnostics.cpu.len(), 2);

            assert_eq!(diagnostics.gpu.len(), if active { 2 } else { 0 });

            diagnostics.write_report().unwrap();

            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();

            assert_eq!(report["completed"], true);

            assert_eq!(report["benchmark"]["gpu_timings_requested"], requested);

            assert_eq!(report["context"]["gpu_timestamps"], active);

            assert_eq!(report["summary"]["cpu"]["acquire_ms"]["max"], 3.0);

            if active {
                assert_eq!(report["missing_gpu_samples"], 0);

                assert_eq!(report["summary"]["gpu"]["scene_ms"]["count"], 2);
            } else {
                assert!(report["missing_gpu_samples"].is_null());

                assert!(report["summary"]["gpu"]["scene_ms"].is_null());
            }
        }

        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn interrupted_report_preserves_valid_samples_and_marks_completion_when_finished() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        let directory = std::env::temp_dir().join(format!(
            "nwyd-frame-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let output = directory.join("report.json");

        let mut diagnostics = FrameDiagnostics::benchmark(1, 3, output.clone(), true);

        diagnostics.set_context(serde_json::json!({ "backend": "vulkan", "pacing": "continuous" }));

        diagnostics.set_gpu_timestamps_active(true);

        diagnostics.record(cpu(2));

        diagnostics.record(cpu(1));

        diagnostics.record_gpu(gpu(1));

        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            let mut sample = cpu(3);

            sample.prepare_ms = invalid;

            diagnostics.record(sample);

            let mut sample = gpu(3);

            sample.scene_ms = invalid;

            diagnostics.record_gpu(sample);
        }

        diagnostics.write_report().unwrap();

        let bytes = std::fs::read(&output).unwrap();

        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(report["completed"], false);

        assert_eq!(report["missing_gpu_samples"], 1);

        assert_eq!(report["context"]["backend"], "vulkan");

        assert_eq!(report["context"]["pacing"], "continuous");

        assert_eq!(report["benchmark"]["warmup_frames"], 1);

        assert_eq!(report["cpu"].as_array().unwrap().len(), 2);

        assert_eq!(report["cpu"][0]["frame_id"], 1);

        assert_eq!(report["cpu"][0]["prepare_ms"], 4.0);

        assert_eq!(report["cpu"][0]["gap_ms"], 2.0);

        assert_eq!(report["gpu"][0]["scene_ms"], 4.0);

        assert_eq!(report["summary"]["cpu"]["total_ms"]["median"], 28.0);

        assert_eq!(report["rejected_samples"]["cpu"], 3);

        assert_eq!(report["rejected_samples"]["gpu"], 3);

        assert!(report["cpu"].as_array().unwrap().iter().all(|sample| {
            CPU_STAGES
                .iter()
                .all(|name| sample[*name].as_f64().is_some_and(f64::is_finite))
        }));

        diagnostics.record(cpu(3));

        diagnostics.write_report().unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();

        assert_eq!(report["completed"], true);

        assert_eq!(report["missing_gpu_samples"], 2);

        assert_eq!(report["cpu"].as_array().unwrap().len(), 3);

        assert_eq!(report["gpu"].as_array().unwrap().len(), 1);

        diagnostics.set_gpu_timestamps_active(false);

        diagnostics.write_report().unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();

        assert_eq!(report["context"]["gpu_timestamps"], false);

        assert!(report["missing_gpu_samples"].is_null());

        assert_eq!(report["gpu"].as_array().unwrap().len(), 1);

        std::fs::remove_dir_all(directory).unwrap();
    }
}
