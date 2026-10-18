#include "nwyd_neural.h"
#include "ngx_snippet.h"
#include "guard.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <vector>

#undef AddVectoredExceptionHandler

struct NwydNrContext {
    dlssnr::NgxSnippet snippet;
    VkDevice device = VK_NULL_HANDLE;
    HMODULE system_nvapi = nullptr;
    uint32_t width = 0;
    uint32_t height = 0;
    NwydNrTuning tuning{};
    uint32_t next_pass = 0;
    uint32_t reset = 0;
    NwydNrImage chain{};
    NwydNrImage depth{};
    NwydNrImage motion{};
    bool failed = false;
    bool cleaned = false;
    ~NwydNrContext();
};

namespace {
std::mutex api_mutex;
NwydNrContext* active_context = nullptr;
PVOID exception_handler = nullptr;
ULONGLONG last_creation = 0;

int32_t error_result(char* out, uint32_t capacity, NwydNrResult result, const char* text) {
    if (out && capacity) std::snprintf(out, capacity, "%s", text);
    return result;
}

int32_t runtime_result(char* out, uint32_t capacity, const char* stage) {
    char text[256];
    std::snprintf(text, sizeof(text), "%s failed (Windows exception 0x%08lx)",
                  stage, static_cast<unsigned long>(dlssnr::g_guardCode));
    return error_result(out, capacity, dlssnr::g_guardCode ? NWYD_NR_EXCEPTION : NWYD_NR_RUNTIME_FAILURE, text);
}

bool ensure_guard() {
    if (!exception_handler) dlssnr::InstallGuard();
    return exception_handler != nullptr;
}

void deactivate_guard() {
    dlssnr::g_guardActive = false;
}

struct ProbeGuard {
    ~ProbeGuard() { if (!active_context) deactivate_guard(); }
};

struct ApiGuard {
    ApiGuard() { dlssnr::g_guardCode = 0; dlssnr::g_guardActive = false; }
    ~ApiGuard() { dlssnr::g_guardActive = false; }
};

struct Module {
    HMODULE value = nullptr;
    ~Module() { if (value) FreeLibrary(value); }
};

bool file_exists(const std::wstring& path) {
    DWORD attributes = GetFileAttributesW(path.c_str());
    return attributes != INVALID_FILE_ATTRIBUTES && !(attributes & FILE_ATTRIBUTE_DIRECTORY);
}

bool runtime_directory(const uint16_t* input, std::wstring& directory) {
    if (!input || !input[0]) return false;
    const auto* path = reinterpret_cast<const wchar_t*>(input);
    DWORD needed = GetFullPathNameW(path, 0, nullptr, nullptr);
    if (!needed || needed >= 32768) return false;
    std::vector<wchar_t> buffer(needed);
    DWORD written = GetFullPathNameW(path, needed, buffer.data(), nullptr);
    if (!written || written >= needed) return false;
    directory.assign(buffer.data(), written);
    return directory.size() < MAX_PATH - 32 && file_exists(directory + L"\\nvngx_dlssnr.dll") &&
           !file_exists(directory + L"\\nvngx.dll") &&
           !file_exists(directory + L"\\_nvngx.dll");
}

struct Environment {
    const wchar_t* name;
    std::wstring previous;
    bool existed;
    Environment(const wchar_t* key, const wchar_t* value) : name(key) {
        SetLastError(ERROR_SUCCESS);
        DWORD size = GetEnvironmentVariableW(name, nullptr, 0);
        existed = size != 0 || GetLastError() != ERROR_ENVVAR_NOT_FOUND;
        if (size) {
            std::vector<wchar_t> buffer(size);
            DWORD written = GetEnvironmentVariableW(name, buffer.data(), size);
            if (written < size) previous.assign(buffer.data(), written);
        }
        SetEnvironmentVariableW(name, value);
    }
    ~Environment() { SetEnvironmentVariableW(name, existed ? previous.c_str() : nullptr); }
};

struct HdrEnvironment {
    std::string previous;
    HdrEnvironment() {
        const char* value = std::getenv("DLSSNR_HDR");
        if (value) previous = value;
        _putenv_s("DLSSNR_HDR", "0");
    }
    ~HdrEnvironment() { _putenv_s("DLSSNR_HDR", previous.c_str()); }
};

bool exports_available(HMODULE module) {
    for (const char* name : {"NVSDK_NGX_VULKAN_Init_Ext", "NVSDK_NGX_VULKAN_CreateFeature",
                            "NVSDK_NGX_VULKAN_EvaluateFeature", "NVSDK_NGX_VULKAN_ReleaseFeature",
                            "NVSDK_NGX_VULKAN_Shutdown1", "NVSDK_NGX_VULKAN_GetFeatureInstanceExtensionRequirements",
                            "NVSDK_NGX_VULKAN_GetFeatureDeviceExtensionRequirements"}) {
        if (!GetProcAddress(module, name)) return false;
    }
    return true;
}

template<size_t N>
bool copy_extensions(char (&out)[N][NWYD_NR_EXTENSION_NAME_SIZE], uint32_t count, const VkExtensionProperties* properties) {
    if (count > N || (count && !properties)) return false;
    for (uint32_t i = 0; i < count; ++i) {
        size_t length = strnlen(properties[i].extensionName, NWYD_NR_EXTENSION_NAME_SIZE);
        if (!length || length == NWYD_NR_EXTENSION_NAME_SIZE) return false;
        std::memcpy(out[i], properties[i].extensionName, length + 1);
    }
    return true;
}

bool valid_image(const NwydNrImage& image, VkFormat format, const NwydNrContext& context) {
    return image.image && image.image_view && image.format == static_cast<uint32_t>(format) &&
           image.aspect_mask == VK_IMAGE_ASPECT_COLOR_BIT && image.width == context.width && image.height == context.height;
}

bool same_image(const NwydNrImage& left, const NwydNrImage& right) {
    return left.image == right.image && left.image_view == right.image_view;
}

bool valid_tuning(const NwydNrTuning& tuning) {
    auto in_range = [](float value, float minimum, float maximum) {
        return std::isfinite(value) && value >= minimum && value <= maximum;
    };
    return in_range(tuning.intensity, 0, 4) && in_range(tuning.tone, 0, 4) &&
           in_range(tuning.structure, 0, 4) && in_range(tuning.skin, -1, 4) &&
           in_range(tuning.sharpness, 0, 1) && tuning.auto_mask <= 1 && tuning.style <= 2 &&
           tuning.passes >= 1 && tuning.passes <= 2;
}

void throttle_creation() {
    const ULONGLONG now = GetTickCount64();
    if (last_creation && now - last_creation < 250) Sleep(static_cast<DWORD>(250 - (now - last_creation)));
    last_creation = GetTickCount64();
}

NVSDK_NGX_Resource_VK resource(const NwydNrImage& image, bool writable) {
    NVSDK_NGX_Resource_VK value{};
    value.Type = NVSDK_NGX_RESOURCE_VK_TYPE_VK_IMAGE_VIEW;
    value.ReadWrite = writable;
    value.Resource.ImageViewInfo.Image = reinterpret_cast<VkImage>(image.image);
    value.Resource.ImageViewInfo.ImageView = reinterpret_cast<VkImageView>(image.image_view);
    value.Resource.ImageViewInfo.SubresourceRange = {image.aspect_mask, 0, 1, 0, 1};
    value.Resource.ImageViewInfo.Format = static_cast<VkFormat>(image.format);
    value.Resource.ImageViewInfo.Width = image.width;
    value.Resource.ImageViewInfo.Height = image.height;
    return value;
}

void teardown(NwydNrContext& context) {
    if (context.cleaned) return;
    context.cleaned = true;
    HMODULE bundled_nvapi = context.snippet.nvapi;
    dlssnr::NgxTeardown(context.snippet, context.device);
    if (bundled_nvapi) FreeLibrary(bundled_nvapi);
    if (context.system_nvapi) FreeLibrary(context.system_nvapi);
    context.system_nvapi = nullptr;
    context.snippet.nvapi = nullptr;
}

int32_t probe(const uint16_t* input, NwydNrRequirements* output, char* error, uint32_t capacity) {
    if (!output || output->struct_size != sizeof(*output) || output->abi_version != NWYD_NR_ABI_VERSION)
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Neural runtime requirements ABI mismatch");
    *output = {};
    output->struct_size = sizeof(*output);
    output->abi_version = NWYD_NR_ABI_VERSION;
    output->minimum_width = output->minimum_height = 64;
    std::wstring directory;
    if (!runtime_directory(input, directory))
        return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "Runtime directory needs nvngx_dlssnr.dll and must not contain nvngx.dll or _nvngx.dll");
    if (!ensure_guard()) return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "Cannot register runtime exception handler");
    ProbeGuard handler;
    Module module;
    DWORD exception = 0;
    module.value = dlssnr::Guarded([&] {
        return LoadLibraryExW((directory + L"\\nvngx_dlssnr.dll").c_str(), nullptr,
                             LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    }, static_cast<HMODULE>(nullptr), &exception);
    if (!module.value) return runtime_result(error, capacity, "Loading neural runtime");
    if (!exports_available(module.value))
        return error_result(error, capacity, NWYD_NR_UNSUPPORTED, "Neural runtime Vulkan exports are incomplete");
    using InstanceRequirements = NVSDK_NGX_Result(__cdecl*)(const void*, uint32_t*, VkExtensionProperties**);
    using DeviceRequirements = NVSDK_NGX_Result(__cdecl*)(VkInstance, VkPhysicalDevice, const void*, uint32_t*, VkExtensionProperties**);
    auto instance_requirements = reinterpret_cast<InstanceRequirements>(GetProcAddress(module.value, "NVSDK_NGX_VULKAN_GetFeatureInstanceExtensionRequirements"));
    auto device_requirements = reinterpret_cast<DeviceRequirements>(GetProcAddress(module.value, "NVSDK_NGX_VULKAN_GetFeatureDeviceExtensionRequirements"));
    VkExtensionProperties* instance_properties = nullptr;
    VkExtensionProperties* device_properties = nullptr;
    bool ok = dlssnr::Guarded([&] {
        return instance_requirements(nullptr, &output->instance_extension_count, &instance_properties) == NVSDK_NGX_Result_Success &&
               device_requirements(VK_NULL_HANDLE, VK_NULL_HANDLE, nullptr, &output->device_extension_count, &device_properties) == NVSDK_NGX_Result_Success &&
               copy_extensions(output->instance_extensions, output->instance_extension_count, instance_properties) &&
               copy_extensions(output->device_extensions, output->device_extension_count, device_properties);
    }, false, &exception);
    if (!ok || dlssnr::g_guardCode) return runtime_result(error, capacity, "Reading neural runtime requirements");
    return error_result(error, capacity, NWYD_NR_SUCCESS, "");
}

int32_t create(const NwydNrCreateInfo* info, NwydNrContext** output, char* error, uint32_t capacity) {
    if (output) *output = nullptr;
    if (!info || !output || info->struct_size != sizeof(*info) || info->abi_version != NWYD_NR_ABI_VERSION ||
        !info->instance || !info->physical_device || !info->device || !info->recording_command_buffer ||
        !info->get_instance_proc_addr || !info->get_device_proc_addr || info->width < 64 || info->height < 64 ||
        info->width > 16384 || info->height > 16384 || !valid_tuning(info->tuning))
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Invalid neural runtime creation arguments");
    if (active_context) return error_result(error, capacity, NWYD_NR_BUSY, "Only one neural runtime context can be active");
    std::wstring directory;
    if (!runtime_directory(info->runtime_directory, directory))
        return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "Runtime directory needs nvngx_dlssnr.dll and must not contain nvngx.dll or _nvngx.dll");
    auto get_instance_proc = reinterpret_cast<PFN_vkGetInstanceProcAddr>(info->get_instance_proc_addr);
    auto instance = reinterpret_cast<VkInstance>(info->instance);
    auto physical = reinterpret_cast<VkPhysicalDevice>(info->physical_device);
    auto properties_fn = reinterpret_cast<PFN_vkGetPhysicalDeviceProperties>(get_instance_proc(instance, "vkGetPhysicalDeviceProperties"));
    if (!properties_fn) return error_result(error, capacity, NWYD_NR_UNSUPPORTED, "Vulkan physical-device query is unavailable");
    VkPhysicalDeviceProperties properties{};
    properties_fn(physical, &properties);
    if (properties.vendorID != 0x10de || properties.limits.maxImageDimension2D < std::max(info->width, info->height))
        return error_result(error, capacity, NWYD_NR_UNSUPPORTED, "Neural runtime requires a compatible NVIDIA Vulkan device");
    if (!ensure_guard()) return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "Cannot register runtime exception handler");
    ProbeGuard handler;
    HMODULE self = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                           reinterpret_cast<LPCWSTR>(&nwyd_nr_create), &self))
        return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "Cannot locate neural adapter module");
    dlssnr::g_layerModule = self;
    auto context = std::make_unique<NwydNrContext>();
    context->device = reinterpret_cast<VkDevice>(info->device);
    context->width = info->width;
    context->height = info->height;
    context->tuning = info->tuning;
    bool wine = GetProcAddress(GetModuleHandleW(L"ntdll.dll"), "wine_get_version") != nullptr;
    if (!wine) {
        if (file_exists(directory + L"\\nvapi64.dll") || file_exists(directory + L"\\dxgi.dll"))
            return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Native Windows must use system NVAPI and DXGI");
        DWORD exception = 0;
        context->system_nvapi = dlssnr::Guarded([&] {
            return LoadLibraryExW(L"nvapi64.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
        }, static_cast<HMODULE>(nullptr), &exception);
        if (!context->system_nvapi) return error_result(error, capacity, NWYD_NR_UNAVAILABLE, "System NVIDIA NVAPI is unavailable");
    }
    Environment binary_path(L"DLSSNR_BIN_DIR", directory.c_str());
    Environment skip_nvapi(L"DLSSNR_SKIP_NVAPI", wine ? L"0" : L"1");
    HdrEnvironment hdr;
    dlssnr::NgxTuning tuning{};
    tuning.intensity = info->tuning.intensity;
    tuning.localTone = info->tuning.tone;
    tuning.localStructure = info->tuning.structure;
    tuning.skinStructure = info->tuning.skin;
    tuning.autoMask = info->tuning.auto_mask;
    tuning.style = info->tuning.style;
    throttle_creation();
    bool ok = dlssnr::NgxLoadAndInit(context->snippet, instance, physical, context->device, info->width, info->height,
                                   reinterpret_cast<VkCommandBuffer>(info->recording_command_buffer), tuning);
    for (uint32_t pass = 1; ok && !dlssnr::g_guardCode && pass < info->tuning.passes; ++pass) {
        throttle_creation();
        dlssnr::NgxSetCreateTuning(context->snippet, tuning);
        ok = !dlssnr::g_guardCode && dlssnr::NgxCreatePass(context->snippet, pass, info->width, info->height,
                                                        reinterpret_cast<VkCommandBuffer>(info->recording_command_buffer));
    }
    if (!ok || dlssnr::g_guardCode) {
        const DWORD exception = dlssnr::g_guardCode;
        teardown(*context);
        dlssnr::g_guardCode = exception;
        return runtime_result(error, capacity, "Initializing neural rendering");
    }
    active_context = context.release();
    *output = active_context;
    return error_result(error, capacity, NWYD_NR_SUCCESS, "");
}

int32_t evaluate(NwydNrContext* context, const NwydNrFrame* frame, char* error, uint32_t capacity) {
    if (!context || context != active_context || !frame || frame->struct_size != sizeof(*frame) || frame->abi_version != NWYD_NR_ABI_VERSION ||
        !frame->recording_command_buffer || frame->pass_index >= context->tuning.passes || frame->reset > 1)
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Invalid neural frame arguments");
    if (context->failed) return error_result(error, capacity, NWYD_NR_RUNTIME_FAILURE, "Neural runtime is disabled after an earlier failure");
    if (frame->pass_index != context->next_pass || (frame->pass_index &&
        (!same_image(frame->color, context->chain) || !same_image(frame->depth, context->depth) ||
         !same_image(frame->motion, context->motion) || frame->reset != context->reset)))
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Neural passes must run in order with the previous output and unchanged guides/reset");
    VkFormat color_format = frame->pass_index ? VK_FORMAT_R16G16B16A16_UNORM : VK_FORMAT_R8G8B8A8_UNORM;
    VkFormat output_format = context->tuning.passes == 2 ? VK_FORMAT_R16G16B16A16_UNORM : VK_FORMAT_R8G8B8A8_UNORM;
    if (!valid_image(frame->color, color_format, *context) ||
        !valid_image(frame->output, output_format, *context) ||
        !valid_image(frame->depth, VK_FORMAT_R32_SFLOAT, *context) ||
        !valid_image(frame->motion, VK_FORMAT_R16G16_SFLOAT, *context) ||
        frame->output.image == frame->color.image || frame->output.image == frame->depth.image || frame->output.image == frame->motion.image)
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Neural images must match the full-resolution pass formats and output must not alias input");
    dlssnr::NgxSetResources(context->snippet, resource(frame->color, false), resource(frame->output, true),
                           resource(frame->motion, false), resource(frame->depth, false), context->width, context->height);
    DWORD exception = 0;
    bool parameters_ok = dlssnr::Guarded([&] {
        context->snippet.params->Set("DLSSNR.DepthInverted", 0u);
        context->snippet.params->Set("DLSSNR.MVecScaleX", 1.0f);
        context->snippet.params->Set("DLSSNR.MVecScaleY", 1.0f);
        return true;
    }, false, &exception);
    dlssnr::NgxSetReset(context->snippet, frame->reset != 0, false);
    dlssnr::NgxSetSharpness(context->snippet, context->tuning.sharpness);
    bool ok = parameters_ok && !dlssnr::g_guardCode && dlssnr::NgxEvaluatePass(context->snippet, frame->pass_index,
                                                reinterpret_cast<VkCommandBuffer>(frame->recording_command_buffer));
    if (!ok || dlssnr::g_guardCode) {
        context->failed = true;
        return runtime_result(error, capacity, "Evaluating neural rendering");
    }
    context->chain = frame->output;
    context->depth = frame->depth;
    context->motion = frame->motion;
    context->reset = frame->reset;
    context->next_pass = (frame->pass_index + 1) % context->tuning.passes;
    return error_result(error, capacity, NWYD_NR_SUCCESS, "");
}
}

NwydNrContext::~NwydNrContext() { teardown(*this); }

namespace nwyd_neural {
PVOID WINAPI register_exception_handler(ULONG first, PVECTORED_EXCEPTION_HANDLER handler) {
    if (!exception_handler) {
        HMODULE pinned = nullptr;
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                               reinterpret_cast<LPCWSTR>(&register_exception_handler), &pinned)) return nullptr;
        exception_handler = AddVectoredExceptionHandler(first, handler);
    }
    return exception_handler;
}
}

extern "C" uint32_t nwyd_nr_abi_version(void) { return NWYD_NR_ABI_VERSION; }

extern "C" int32_t nwyd_nr_probe(const uint16_t* directory, NwydNrRequirements* requirements, char* error, uint32_t capacity) {
    std::lock_guard<std::mutex> lock(api_mutex);
    ApiGuard guard;
    try { return probe(directory, requirements, error, capacity); }
    catch (...) { return error_result(error, capacity, NWYD_NR_RUNTIME_FAILURE, "Neural probe allocation failed"); }
}

extern "C" int32_t nwyd_nr_create(const NwydNrCreateInfo* info, NwydNrContext** context, char* error, uint32_t capacity) {
    std::lock_guard<std::mutex> lock(api_mutex);
    ApiGuard guard;
    try { return create(info, context, error, capacity); }
    catch (...) { return error_result(error, capacity, NWYD_NR_RUNTIME_FAILURE, "Neural context allocation failed"); }
}

extern "C" int32_t nwyd_nr_evaluate(NwydNrContext* context, const NwydNrFrame* frame, char* error, uint32_t capacity) {
    std::lock_guard<std::mutex> lock(api_mutex);
    ApiGuard guard;
    try { return evaluate(context, frame, error, capacity); }
    catch (...) {
        if (context == active_context && context) context->failed = true;
        return error_result(error, capacity, NWYD_NR_RUNTIME_FAILURE, "Neural evaluation allocation failed");
    }
}

extern "C" int32_t nwyd_nr_destroy(NwydNrContext* context, char* error, uint32_t capacity) {
    std::lock_guard<std::mutex> lock(api_mutex);
    ApiGuard guard;
    if (!context || context != active_context)
        return error_result(error, capacity, NWYD_NR_INVALID_ARGUMENT, "Invalid neural context");
    try { teardown(*context); }
    catch (...) {
        context->failed = true;
        return error_result(error, capacity, NWYD_NR_RUNTIME_FAILURE, "Neural context teardown failed");
    }
    delete context;
    active_context = nullptr;
    deactivate_guard();
    if (dlssnr::g_guardCode) return runtime_result(error, capacity, "Destroying neural context");
    return error_result(error, capacity, NWYD_NR_SUCCESS, "");
}

static_assert(sizeof(void*) == 8);
static_assert(sizeof(NwydNrTuning) == 32);
static_assert(sizeof(NwydNrCreateInfo) == 104);
static_assert(sizeof(NwydNrImage) == 32);
static_assert(sizeof(NwydNrFrame) == 152);
static_assert(sizeof(NwydNrRequirements) == 6168);
