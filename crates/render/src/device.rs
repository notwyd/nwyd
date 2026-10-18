use crate::error::{Error, Result};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

mod backend;

pub use backend::RendererBackend;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub breadcrumb: Arc<Breadcrumb>,
    lost: Arc<AtomicBool>,
    backend: RendererBackend,
    available_backends: Vec<RendererBackend>,
    #[cfg(all(windows, feature = "neural-rendering"))]
    neural_supported: Arc<std::sync::OnceLock<bool>>,
}

#[derive(Debug, Default)]
pub struct Breadcrumb {
    line: Mutex<String>,
    stamps: AtomicU64,
}

impl Breadcrumb {
    pub fn set(&self, line: impl Into<String>) {
        self.stamps.fetch_add(1, Ordering::Relaxed);

        match self.line.lock() {
            Ok(mut held) => *held = line.into(),
            Err(poison) => *poison.into_inner() = line.into(),
        }
    }

    pub fn read(&self) -> String {
        let stamps = self.stamps.load(Ordering::Relaxed);

        if stamps == 0 {
            return "nothing recorded yet — the failure is before the first frame".to_string();
        }

        let line = match self.line.lock() {
            Ok(held) => held.clone(),
            Err(poison) => poison.into_inner().clone(),
        };

        format!("{line} (stamp {stamps})")
    }
}

pub fn describe_uncaptured(err: &wgpu::Error, adapter: &str, census: &str) -> String {
    use std::error::Error as _;

    let family = match err {
        wgpu::Error::OutOfMemory { .. } => "out of memory",
        wgpu::Error::Validation { .. } => "validation",
        wgpu::Error::Internal { .. } => "internal",
    };

    let mut report = format!("nwyd: GPU {family} error on {adapter}\nnwyd:   {err}");

    let mut source = err.source();

    while let Some(cause) = source {
        report.push_str(&format!("\nnwyd:   caused by: {cause}"));

        source = cause.source();
    }

    report.push_str(&format!("\nnwyd:   last census: {census}"));

    report
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let info = self.adapter.get_info();

        f.debug_struct("Gpu")
            .field("backend", &info.backend)
            .field("name", &info.name)
            .field("device_type", &info.device_type)
            .finish()
    }
}

pub const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::TEXTURE_COMPRESSION_BC;

fn instance_descriptor() -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();

    if cfg!(target_arch = "wasm32") {
        descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
    } else {
        descriptor.backends = wgpu::Instance::enabled_backend_features() - wgpu::Backends::NOOP;
    }

    descriptor.with_env()
}

impl Gpu {
    pub async fn open(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Gpu> {
        Self::open_with_backend(instance, surface, None).await
    }

    pub async fn open_with_backend(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        preferred: Option<RendererBackend>,
    ) -> Result<Gpu> {
        #[cfg(target_arch = "wasm32")]
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| Error::NoAdapter(e.to_string()))?;

        #[cfg(target_arch = "wasm32")]
        let adapters = vec![adapter];

        #[cfg(not(target_arch = "wasm32"))]
        let adapters = instance
            .enumerate_adapters(instance_descriptor().backends)
            .await;

        let mut candidates = adapters
            .into_iter()
            .filter_map(|adapter| backend::Candidate::new(adapter, surface))
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| left.rank(preferred).cmp(&right.rank(preferred)));

        if candidates.is_empty() {
            return Err(Error::NoAdapter(
                "no renderer supports this display and the required graphics capabilities"
                    .to_string(),
            ));
        }

        let (selected, (device, queue)) = backend::first_available(&candidates, |candidate| {
            let adapter = candidate.adapter.clone();
            async move {
                let profiling_features = if cfg!(target_arch = "wasm32") {
                    wgpu::Features::empty()
                } else {
                    adapter.features()
                        & (wgpu::Features::TIMESTAMP_QUERY
                            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
                };

                let descriptor = wgpu::DeviceDescriptor {
                    label: crate::gpu_label("nwyd"),
                    required_features: REQUIRED_FEATURES | profiling_features,
                    required_limits: adapter.limits(),
                    experimental_features: wgpu::ExperimentalFeatures::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                };

                #[cfg(all(windows, feature = "neural-rendering"))]
                match crate::neural::request_device(&adapter, &descriptor) {
                    Ok(Some(device)) => return Ok(device),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(%error, "Neural rendering device setup unavailable")
                    }
                }

                adapter
                    .request_device(&descriptor)
                    .await
                    .map_err(|error| format!("{}: {error}", adapter.get_info().name))
            }
        })
        .await
        .map_err(|errors| Error::NoDevice(errors.join("; ")))?;

        let available_backends = backend::unique_backends(
            candidates[selected..]
                .iter()
                .map(|candidate| candidate.backend),
        );
        let selected = candidates.swap_remove(selected);
        let adapter = selected.adapter;
        let backend = selected.backend;

        let breadcrumb = Arc::new(Breadcrumb::default());

        {
            let name = adapter.get_info().name;

            let crumb = Arc::clone(&breadcrumb);

            device.on_uncaptured_error(Arc::new(move |err: wgpu::Error| {
                let report = describe_uncaptured(&err, &name, &crumb.read());

                tracing::info!("{report}");

                use std::io::Write as _;

                let _ = std::io::stderr().flush();

                panic!("{report}");
            }));
        }

        let lost = Arc::new(AtomicBool::new(false));

        let notified = Arc::clone(&lost);

        device.set_device_lost_callback(move |reason, message| {
            notified.store(true, Ordering::Release);

            if reason == wgpu::DeviceLostReason::Unknown {
                tracing::warn!(
                    ? reason,% message,
                    "nwyd: GPU device lost; rendering resources need recovery"
                );
            }
        });

        Ok(Gpu {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            breadcrumb,
            lost,
            backend,
            available_backends,
            #[cfg(all(windows, feature = "neural-rendering"))]
            neural_supported: Arc::new(std::sync::OnceLock::new()),
        })
    }

    pub fn instance() -> wgpu::Instance {
        wgpu::Instance::new(instance_descriptor())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn instance_with_display(
        display: Box<impl wgpu::rwh::HasDisplayHandle + std::fmt::Debug + Send + Sync + 'static>,
    ) -> wgpu::Instance {
        wgpu::Instance::new(instance_descriptor().with_display_handle(display))
    }

    pub fn backend(&self) -> RendererBackend {
        self.backend
    }

    pub fn supports_neural_rendering(&self) -> bool {
        #[cfg(all(windows, feature = "neural-rendering"))]
        {
            *self
                .neural_supported
                .get_or_init(|| crate::neural::probe(self))
        }
        #[cfg(not(all(windows, feature = "neural-rendering")))]
        {
            false
        }
    }

    pub fn available_backends(&self) -> &[RendererBackend] {
        &self.available_backends
    }

    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }
}
