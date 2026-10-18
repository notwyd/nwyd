use super::passes::DEVICE_FEATURES;
use super::tuning::Tuning;
use crate::Gpu;
use ash::vk::{self, Handle};
use std::ffi::{CStr, c_char, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr::NonNull;
use std::sync::{Arc, OnceLock};
use wgpu::hal::api::Vulkan;

const ABI_VERSION: u32 = 2;
const ERROR_CAPACITY: usize = 1024;

#[repr(C)]
struct Requirements {
    struct_size: u32,
    abi_version: u32,
    instance_extension_count: u32,
    device_extension_count: u32,
    minimum_width: u32,
    minimum_height: u32,
    instance_extensions: [[u8; 256]; 8],
    device_extensions: [[u8; 256]; 16],
}

#[repr(C)]
struct CreateInfo {
    struct_size: u32,
    abi_version: u32,
    runtime_directory: *const u16,
    instance: u64,
    physical_device: u64,
    device: u64,
    recording_command_buffer: u64,
    get_instance_proc_addr: u64,
    get_device_proc_addr: u64,
    width: u32,
    height: u32,
    tuning: Tuning,
}

#[repr(C)]
struct Image {
    image: u64,
    image_view: u64,
    format: u32,
    aspect_mask: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
struct Frame {
    struct_size: u32,
    abi_version: u32,
    recording_command_buffer: u64,
    color: Image,
    depth: Image,
    motion: Image,
    output: Image,
    reset: u32,
    pass_index: u32,
}

type Probe = unsafe extern "C" fn(*const u16, *mut Requirements, *mut c_char, u32) -> i32;
type Create = unsafe extern "C" fn(*const CreateInfo, *mut *mut c_void, *mut c_char, u32) -> i32;
type Evaluate = unsafe extern "C" fn(*mut c_void, *const Frame, *mut c_char, u32) -> i32;
type Destroy = unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> i32;

struct Runtime {
    _library: libloading::Library,
    directory: Vec<u16>,
    requirements: Requirements,
    create: Create,
    evaluate: Evaluate,
    destroy: Destroy,
}

impl Runtime {
    fn get() -> Result<&'static Self, String> {
        static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();
        RUNTIME
            .get_or_init(Self::load)
            .as_ref()
            .map_err(Clone::clone)
    }

    fn load() -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let executable_directory = executable.parent().ok_or("No executable directory")?;
        let external = std::env::var_os("NWYD_NEURAL_RUNTIME_DIR").map(std::path::PathBuf::from);
        #[cfg(feature = "bundled-neural-runtime")]
        let bundled = if external.as_ref().is_some_and(|directory| {
            directory.join("nwyd_neural.dll").is_file()
                || executable_directory.join("nwyd_neural.dll").is_file()
        }) {
            None
        } else {
            super::bundle::directory()?
        };
        #[cfg(not(feature = "bundled-neural-runtime"))]
        let bundled: Option<std::path::PathBuf> = None;
        let directory = match external.as_ref().or(bundled.as_ref()) {
            Some(path) => path.clone(),
            None => executable_directory.to_owned(),
        };
        let directory = directory
            .canonicalize()
            .map_err(|error| format!("Neural runtime directory: {error}"))?;
        let path = if external.is_some() && directory.join("nwyd_neural.dll").is_file() {
            directory.join("nwyd_neural.dll")
        } else if external.is_some() && executable_directory.join("nwyd_neural.dll").is_file() {
            executable_directory.join("nwyd_neural.dll")
        } else {
            bundled
                .as_deref()
                .unwrap_or(executable_directory)
                .join("nwyd_neural.dll")
        };
        let directory = directory
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let library: libloading::Library = unsafe {
            libloading::os::windows::Library::load_with_flags(&path, 0x00000100 | 0x00001000)
                .map_err(|error| format!("Loading {}: {error}", path.display()))?
                .into()
        };
        let mut requirements = Requirements {
            struct_size: std::mem::size_of::<Requirements>() as u32,
            abi_version: ABI_VERSION,
            instance_extension_count: 0,
            device_extension_count: 0,
            minimum_width: 0,
            minimum_height: 0,
            instance_extensions: [[0; 256]; 8],
            device_extensions: [[0; 256]; 16],
        };
        let mut error = [0; ERROR_CAPACITY];
        unsafe {
            let version = library
                .get::<unsafe extern "C" fn() -> u32>(b"nwyd_nr_abi_version\0")
                .map_err(|error| error.to_string())?;
            if version() != ABI_VERSION {
                return Err("Unsupported neural runtime adapter ABI".into());
            }
            let probe = library
                .get::<Probe>(b"nwyd_nr_probe\0")
                .map_err(|error| error.to_string())?;
            status(
                probe(
                    directory.as_ptr(),
                    &mut requirements,
                    error.as_mut_ptr(),
                    ERROR_CAPACITY as u32,
                ),
                &error,
            )?;
            if requirements.struct_size != std::mem::size_of::<Requirements>() as u32
                || requirements.abi_version != ABI_VERSION
                || requirements.instance_extension_count > 8
                || requirements.device_extension_count > 16
            {
                return Err("Invalid neural runtime requirements".into());
            }
            for name in requirements
                .instance_extensions
                .iter()
                .take(requirements.instance_extension_count as usize)
                .chain(
                    requirements
                        .device_extensions
                        .iter()
                        .take(requirements.device_extension_count as usize),
                )
            {
                CStr::from_bytes_until_nul(name).map_err(|_| "Invalid neural extension name")?;
            }
            Ok(Self {
                create: *library
                    .get::<Create>(b"nwyd_nr_create\0")
                    .map_err(|error| error.to_string())?,
                evaluate: *library
                    .get::<Evaluate>(b"nwyd_nr_evaluate\0")
                    .map_err(|error| error.to_string())?,
                destroy: *library
                    .get::<Destroy>(b"nwyd_nr_destroy\0")
                    .map_err(|error| error.to_string())?,
                _library: library,
                directory,
                requirements,
            })
        }
    }

    fn instance_extensions(&'static self) -> impl Iterator<Item = &'static CStr> {
        self.requirements
            .instance_extensions
            .iter()
            .take(self.requirements.instance_extension_count as usize)
            .map(|name| CStr::from_bytes_until_nul(name).unwrap())
    }

    fn device_extensions(&'static self) -> impl Iterator<Item = &'static CStr> {
        self.requirements
            .device_extensions
            .iter()
            .take(self.requirements.device_extension_count as usize)
            .map(|name| CStr::from_bytes_until_nul(name).unwrap())
    }
}

pub(crate) fn request_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<Option<(wgpu::Device, wgpu::Queue)>, String> {
    if adapter.get_info().vendor != 0x10de || adapter.get_info().backend != wgpu::Backend::Vulkan {
        return Ok(None);
    }
    let Ok(runtime) = Runtime::get() else {
        return Ok(None);
    };
    if !adapter.features().contains(DEVICE_FEATURES) {
        return Ok(None);
    }
    let mut descriptor = descriptor.clone();
    descriptor.required_features |= DEVICE_FEATURES;
    unsafe {
        let raw = adapter.as_hal::<Vulkan>().ok_or("No Vulkan adapter")?;
        if !runtime
            .instance_extensions()
            .all(|extension| raw.shared_instance().extensions().contains(&extension))
            || !runtime.device_extensions().all(|extension| {
                raw.physical_device_capabilities()
                    .supports_extension(extension)
            })
        {
            return Ok(None);
        }
        let mut address = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
        let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut address);
        raw.shared_instance()
            .raw_instance()
            .get_physical_device_features2(raw.raw_physical_device(), &mut features);
        if address.buffer_device_address == vk::FALSE {
            return Ok(None);
        }
        address =
            vk::PhysicalDeviceBufferDeviceAddressFeatures::default().buffer_device_address(true);
        let opened = raw
            .open_with_callback(
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                Some(Box::new(|args| {
                    for extension in runtime.device_extensions() {
                        if !args.extensions.contains(&extension) {
                            args.extensions.push(extension);
                        }
                    }
                    *args.create_info = args.create_info.push_next(&mut address);
                })),
            )
            .map_err(|error| error.to_string())?;
        adapter
            .create_device_from_hal::<Vulkan>(opened, &descriptor)
            .map(Some)
            .map_err(|error| error.to_string())
    }
}

fn eligible(gpu: &Gpu, runtime: &'static Runtime, size: [u32; 2]) -> bool {
    if gpu.info().vendor != 0x10de
        || gpu.info().backend != wgpu::Backend::Vulkan
        || gpu.device.limits().max_sampled_textures_per_shader_stage < 3
        || !gpu.device.features().contains(DEVICE_FEATURES)
        || !crate::temporal::supported(gpu, wgpu::TextureFormat::Rgba8Unorm, (size[0], size[1]))
    {
        return false;
    }
    for (format, required) in [
        (
            wgpu::TextureFormat::R32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        ),
        (
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        ),
        (
            wgpu::TextureFormat::Rgba16Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        ),
        (
            wgpu::TextureFormat::Rg16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        ),
    ] {
        if !gpu
            .adapter
            .get_texture_format_features(format)
            .allowed_usages
            .contains(required)
        {
            return false;
        }
    }
    unsafe {
        gpu.device.as_hal::<Vulkan>().is_some_and(|device| {
            runtime
                .device_extensions()
                .all(|extension| device.enabled_device_extensions().contains(&extension))
        })
    }
}

pub(crate) fn probe(gpu: &Gpu) -> bool {
    match Session::new(gpu, [64, 64]) {
        Ok(session) => {
            drop(session);
            true
        }
        Err(error) => {
            tracing::info!(%error, "DLSS 5 Neural Rendering unavailable");
            false
        }
    }
}

pub(super) struct Session {
    runtime: &'static Runtime,
    context: NonNull<c_void>,
    device: Arc<wgpu::Device>,
}

unsafe impl Send for Session {}

impl Session {
    pub fn matches_device(&self, gpu: &Gpu) -> bool {
        Arc::ptr_eq(&self.device, &gpu.device)
    }

    pub fn new(gpu: &Gpu, size: [u32; 2]) -> Result<Self, String> {
        let runtime = Runtime::get()?;
        if !eligible(gpu, runtime, size) {
            return Err("Neural rendering requires a supported NVIDIA Vulkan device".into());
        }
        if size[0] < runtime.requirements.minimum_width
            || size[1] < runtime.requirements.minimum_height
        {
            return Err("Neural rendering resolution is too small".into());
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: crate::gpu_label("create neural rendering feature"),
            });
        let mut context = std::ptr::null_mut();
        let mut error = [0; ERROR_CAPACITY];
        let result = unsafe {
            let raw_device = gpu.device.as_hal::<Vulkan>().ok_or("No Vulkan device")?;
            let instance = raw_device.shared_instance();
            encoder.as_hal_mut::<Vulkan, _, _>(|command| {
                let command = command.ok_or("No Vulkan command encoder")?;
                let info = CreateInfo {
                    struct_size: std::mem::size_of::<CreateInfo>() as u32,
                    abi_version: ABI_VERSION,
                    runtime_directory: runtime.directory.as_ptr(),
                    instance: instance.raw_instance().handle().as_raw(),
                    physical_device: raw_device.raw_physical_device().as_raw(),
                    device: raw_device.raw_device().handle().as_raw(),
                    recording_command_buffer: command.raw_handle().as_raw(),
                    get_instance_proc_addr: instance.entry().static_fn().get_instance_proc_addr
                        as usize as u64,
                    get_device_proc_addr: instance.raw_instance().fp_v1_0().get_device_proc_addr
                        as usize as u64,
                    width: size[0],
                    height: size[1],
                    tuning: Tuning::default(),
                };
                status(
                    (runtime.create)(
                        &info,
                        &mut context,
                        error.as_mut_ptr(),
                        ERROR_CAPACITY as u32,
                    ),
                    &error,
                )
            })
        };
        result?;
        let session = Self {
            runtime,
            context: NonNull::new(context).ok_or("Neural runtime returned no session")?,
            device: Arc::clone(&gpu.device),
        };
        let submission = gpu.queue.submit([encoder.finish()]);
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .map_err(|error| format!("Neural feature creation: {error}"))?;
        Ok(session)
    }

    pub fn record(
        &mut self,
        gpu: &Gpu,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        output: &wgpu::TextureView,
        reset: bool,
        pass_index: u32,
    ) -> Result<wgpu::CommandBuffer, String> {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: crate::gpu_label("evaluate neural rendering"),
            });
        let color = image(color)?;
        let depth = image(depth)?;
        let motion = image(motion)?;
        let output = image(output)?;
        let mut error = [0; ERROR_CAPACITY];
        unsafe {
            encoder.as_hal_mut::<Vulkan, _, _>(|command| {
                let command = command.ok_or("No Vulkan command encoder")?;
                let frame = Frame {
                    struct_size: std::mem::size_of::<Frame>() as u32,
                    abi_version: ABI_VERSION,
                    recording_command_buffer: command.raw_handle().as_raw(),
                    color,
                    depth,
                    motion,
                    output,
                    reset: u32::from(reset),
                    pass_index,
                };
                status(
                    (self.runtime.evaluate)(
                        self.context.as_ptr(),
                        &frame,
                        error.as_mut_ptr(),
                        ERROR_CAPACITY as u32,
                    ),
                    &error,
                )
            })?;
        }
        Ok(encoder.finish())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        }) {
            tracing::warn!(%error, "Retaining neural runtime resources while the GPU is unavailable");
            std::mem::forget(Arc::clone(&self.device));
            return;
        }
        let mut error = [0; ERROR_CAPACITY];
        let result = unsafe {
            (self.runtime.destroy)(
                self.context.as_ptr(),
                error.as_mut_ptr(),
                ERROR_CAPACITY as u32,
            )
        };
        if let Err(error) = status(result, &error) {
            tracing::warn!(%error, "Neural rendering cleanup failed");
        }
    }
}

fn image(view: &wgpu::TextureView) -> Result<Image, String> {
    let format = match view.texture().format() {
        wgpu::TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        wgpu::TextureFormat::Rgba16Unorm => vk::Format::R16G16B16A16_UNORM,
        wgpu::TextureFormat::R32Float => vk::Format::R32_SFLOAT,
        wgpu::TextureFormat::Rg16Float => vk::Format::R16G16_SFLOAT,
        _ => return Err("Unsupported neural rendering image format".into()),
    };
    unsafe {
        let texture = view.texture();
        let image = texture
            .as_hal::<Vulkan>()
            .ok_or("Neural input is not a Vulkan image")?
            .raw_handle()
            .as_raw();
        let image_view = view
            .as_hal::<Vulkan>()
            .ok_or("Neural input is not a Vulkan view")?
            .raw_handle()
            .as_raw();
        Ok(Image {
            image,
            image_view,
            format: format.as_raw() as u32,
            aspect_mask: vk::ImageAspectFlags::COLOR.as_raw(),
            width: texture.width(),
            height: texture.height(),
        })
    }
}

fn status(result: i32, error: &[c_char]) -> Result<(), String> {
    if result == 0 {
        return Ok(());
    }
    let bytes = error
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect::<Vec<_>>();
    let message = String::from_utf8_lossy(&bytes);
    Err(format!("Neural runtime error {result}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_abi_layouts_match_the_version_two_contract() {
        assert_eq!(std::mem::size_of::<Requirements>(), 6168);
        assert_eq!(std::mem::size_of::<CreateInfo>(), 104);
        assert_eq!(std::mem::size_of::<Image>(), 32);
        assert_eq!(std::mem::size_of::<Frame>(), 152);
        assert_eq!(std::mem::offset_of!(CreateInfo, instance), 16);
        assert_eq!(std::mem::offset_of!(CreateInfo, tuning), 72);
        assert_eq!(std::mem::offset_of!(Frame, color), 16);
        assert_eq!(std::mem::offset_of!(Frame, reset), 144);
        assert_eq!(std::mem::offset_of!(Frame, pass_index), 148);
    }

    #[test]
    fn all_nonzero_runtime_results_are_failures() {
        assert!(status(0, &[]).is_ok());
        for code in [1, 2, 3, 4, 5, 6, -1, i32::MIN] {
            assert!(status(code, &[]).is_err());
        }
    }
}
