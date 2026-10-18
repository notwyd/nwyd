use crate::Gpu;

#[cfg(all(test, feature = "bundled-neural-runtime"))]
#[path = "../../build_support/neural_bundle.rs"]
mod bundle_build;

#[cfg(all(feature = "bundled-neural-runtime", any(windows, test)))]
mod bundle;

#[cfg(any(windows, test))]
mod passes;
#[cfg(any(windows, test))]
mod tuning;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{probe, request_device};

#[cfg(all(windows, test))]
mod tests;

pub struct NeuralRenderer {
    #[cfg(windows)]
    state: Option<(windows::Session, passes::Passes)>,
    size: [u32; 2],
}

impl NeuralRenderer {
    pub fn new(gpu: &Gpu, size: [u32; 2]) -> Result<Self, String> {
        if !gpu.supports_neural_rendering() {
            return Err("DLSS 5 Neural Rendering is unavailable on this device".into());
        }
        #[cfg(windows)]
        {
            let passes = passes::Passes::new(gpu, size)?;
            let session = windows::Session::new(gpu, size)?;
            Ok(Self {
                state: Some((session, passes)),
                size,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = size;
            Err("DLSS 5 Neural Rendering requires Windows and Vulkan".into())
        }
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    pub fn resize(&mut self, gpu: &Gpu, size: [u32; 2]) -> Result<(), String> {
        #[cfg(windows)]
        if !self
            .state
            .as_ref()
            .is_some_and(|(session, _)| session.matches_device(gpu))
        {
            return Err(
                "Neural rendering session belongs to a different or unavailable device".into(),
            );
        }
        if self.size == size {
            return Ok(());
        }
        #[cfg(windows)]
        {
            self.state = None;
        }
        *self = Self::new(gpu, size)?;
        Ok(())
    }

    pub fn evaluate(
        &mut self,
        gpu: &Gpu,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        bias: &wgpu::TextureView,
        target: &wgpu::TextureView,
        reset: bool,
    ) -> Result<(), String> {
        #[cfg(windows)]
        {
            let (session, passes) = self
                .state
                .as_mut()
                .ok_or("Neural rendering was disabled after a failure")?;
            if !session.matches_device(gpu) {
                return Err("Neural rendering session belongs to a different device".into());
            }
            passes.validate_inputs(color, depth, motion, bias, target)?;
            let barriers = passes.prepare(gpu, color, depth, motion, bias);
            let first_pass = match session.record(
                gpu,
                passes.color(),
                &passes.depth,
                passes.motion(),
                &passes.intermediate,
                reset,
                0,
            ) {
                Ok(commands) => commands,
                Err(error) => {
                    self.state = None;
                    return Err(error);
                }
            };
            let intermediate_barrier = passes.between_passes(gpu);
            let second_pass = match session.record(
                gpu,
                &passes.intermediate,
                &passes.depth,
                passes.motion(),
                &passes.output,
                reset,
                1,
            ) {
                Ok(commands) => commands,
                Err(error) => {
                    self.state = None;
                    return Err(error);
                }
            };
            let composition = passes.compose(gpu, target);
            gpu.queue.submit([
                barriers,
                first_pass,
                intermediate_barrier,
                second_pass,
                composition,
            ]);
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = (gpu, color, depth, motion, bias, target, reset);
            Err("DLSS 5 Neural Rendering requires Windows and Vulkan".into())
        }
    }
}
