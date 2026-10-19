use anyhow::{Context, Result, bail};
use render::device::Gpu;

pub(super) fn configure(
    surface: &wgpu::Surface<'_>,
    gpu: &Gpu,
    config: &wgpu::SurfaceConfiguration,
) -> Result<()> {
    configure_with(
        config,
        || gpu.is_lost(),
        || surface.get_capabilities(&gpu.adapter),
        || {
            #[cfg(not(target_arch = "wasm32"))]
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);

            surface.configure(&gpu.device, config);

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(error) = pollster::block_on(scope.pop()) {
                return Err(error.into());
            }

            Ok(())
        },
    )
    .with_context(|| format!("configuring the window surface on {}", gpu.info().name))
}

fn configure_with(
    config: &wgpu::SurfaceConfiguration,
    is_lost: impl Fn() -> bool,
    capabilities: impl FnOnce() -> wgpu::SurfaceCapabilities,
    configure: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if config.width == 0 || config.height == 0 || is_lost() {
        return Ok(());
    }

    let result = (|| {
        let capabilities = capabilities();

        if capabilities.formats.is_empty() || capabilities.present_modes.is_empty() {
            bail!(
                "window surface capabilities are unavailable; the display connection may have been lost"
            );
        }

        if !capabilities.formats.contains(&config.format) {
            bail!("window surface no longer supports {:?}", config.format);
        }

        if is_lost() {
            return Ok(());
        }

        configure()
    })();

    if is_lost() { Ok(()) } else { result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn config() -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1600,
            height: 900,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        }
    }

    fn capabilities() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Bgra8Unorm],
            present_modes: vec![wgpu::PresentMode::Fifo],
            ..Default::default()
        }
    }

    #[test]
    fn unavailable_surface_never_reaches_gpu_configuration() {
        for available in [
            wgpu::SurfaceCapabilities::default(),
            wgpu::SurfaceCapabilities {
                present_modes: vec![],
                ..capabilities()
            },
        ] {
            let error = configure_with(
                &config(),
                || false,
                || available,
                || panic!("an unavailable display must not be configured"),
            )
            .unwrap_err();

            assert!(error.to_string().contains("display connection"));
        }
    }

    #[test]
    fn changed_surface_format_never_reaches_gpu_configuration() {
        let error = configure_with(
            &config(),
            || false,
            || wgpu::SurfaceCapabilities {
                formats: vec![wgpu::TextureFormat::Rgba8Unorm],
                ..capabilities()
            },
            || panic!("an unsupported format must not be configured"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("Bgra8Unorm"));
    }

    #[test]
    fn zero_size_and_lost_devices_skip_surface_queries() {
        for (width, height, lost) in [(0, 900, false), (1600, 0, false), (1600, 900, true)] {
            let config = wgpu::SurfaceConfiguration {
                width,
                height,
                ..config()
            };

            configure_with(
                &config,
                || lost,
                || panic!("inactive surfaces must not be queried"),
                || panic!("inactive surfaces must not be configured"),
            )
            .unwrap();
        }
    }

    #[test]
    fn supported_surface_is_configured_once() {
        let configured = Cell::new(0);

        configure_with(
            &config(),
            || false,
            capabilities,
            || {
                configured.set(configured.get() + 1);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(configured.get(), 1);
    }

    #[test]
    fn configuration_failure_after_successful_query_is_reported() {
        let error = configure_with(
            &config(),
            || false,
            capabilities,
            || bail!("display disconnected during configure"),
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "display disconnected during configure");
    }

    #[test]
    fn device_loss_during_capability_query_defers_to_device_recovery() {
        for available in [capabilities(), wgpu::SurfaceCapabilities::default()] {
            let lost = Cell::new(false);

            configure_with(
                &config(),
                || lost.get(),
                || {
                    lost.set(true);
                    available
                },
                || panic!("a lost device must not be configured"),
            )
            .unwrap();
        }
    }

    #[test]
    fn device_loss_during_configuration_defers_to_device_recovery() {
        let lost = Cell::new(false);

        configure_with(
            &config(),
            || lost.get(),
            capabilities,
            || {
                lost.set(true);
                bail!("device lost during configure")
            },
        )
        .unwrap();
    }
}
