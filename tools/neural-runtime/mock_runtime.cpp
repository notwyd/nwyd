#include "ngx_abi.h"
#include "mock_contract.h"
#include <cstring>
#include <cwchar>

static NwydNrMockStats stats{};
static NwydNrTuning expected{2.0f, 2.0f, 2.05f, -1.0f, 0.25f, 1, 2, 2};
static NVSDK_NGX_Handle* handles[2]{};
static uint32_t next_pass = 0;
static uint32_t expected_reset = 1;
static uintptr_t next_handle = 0x1000;
static VkImage chain = VK_NULL_HANDLE;
static VkImage depth_image = VK_NULL_HANDLE;
static VkImage motion_image = VK_NULL_HANDLE;

static bool requested(const char* name) {
    char value[32]{};
    return GetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", value, sizeof(value)) && !std::strcmp(value, name);
}

static NVSDK_NGX_Result invalid() {
    ++stats.invalid_calls;
    return NVSDK_NGX_Result_FAIL_InvalidParameter;
}

static void __cdecl parameter_fault(void*, const char*, void*) {
    RaiseException(EXCEPTION_ACCESS_VIOLATION, 0, 0, nullptr);
}

extern "C" __declspec(dllexport) bool nwyd_mock_reset(const NwydNrTuning* tuning) {
    if (stats.live_features || !tuning) return false;
    stats = {};
    expected = *tuning;
    next_pass = 0;
    expected_reset = 1;
    chain = depth_image = motion_image = VK_NULL_HANDLE;
    return true;
}

extern "C" __declspec(dllexport) void nwyd_mock_snapshot(NwydNrMockStats* output) { *output = stats; }
extern "C" __declspec(dllexport) void nwyd_mock_expect_reset(uint32_t reset) { expected_reset = reset; }

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_Init_Ext(
    unsigned long long, const wchar_t*, VkInstance, VkPhysicalDevice, VkDevice, unsigned, const void*) {
    HMODULE caller = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                      reinterpret_cast<LPCWSTR>(__builtin_return_address(0)), &caller);
    wchar_t name[MAX_PATH]{};
    GetModuleFileNameW(caller, name, MAX_PATH);
    return std::wcscmp(name, L"nvngx.dll") == 0 ? NVSDK_NGX_Result_Success : NVSDK_NGX_Result_FAIL_PlatformError;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_CreateFeature(
    VkCommandBuffer, int feature, NVSDK_NGX_Parameter* parameters, NVSDK_NGX_Handle** handle) {
    ++stats.create_attempts;
    uint32_t pass = handles[0] ? 1 : 0;
    if (requested("create") || (pass == 1 && requested("create_second"))) return NVSDK_NGX_Result_FAIL_Failure;
    if (feature != 18 || handles[pass] || pass >= expected.passes) return invalid();
    float intensity = 0, tone = 0, structure = 0, skin = 0;
    unsigned style = 0, auto_mask = 0, preset = 1;
    if (parameters->Get("DLSSNR.Intensity", &intensity) != NVSDK_NGX_Result_Success || intensity != expected.intensity ||
        parameters->Get("DLSSNR.LocalToneStrength", &tone) != NVSDK_NGX_Result_Success || tone != expected.tone ||
        parameters->Get("DLSSNR.LocalStructureStrength", &structure) != NVSDK_NGX_Result_Success || structure != expected.structure ||
        parameters->Get("DLSSNR.SkinStructureStrength", &skin) != NVSDK_NGX_Result_Success || skin != expected.skin ||
        parameters->Get("DLSSNR.Style", &style) != NVSDK_NGX_Result_Success || style != expected.style ||
        parameters->Get("DLSSNR.UseAutoMask", &auto_mask) != NVSDK_NGX_Result_Success || auto_mask != expected.auto_mask ||
        parameters->Get("DLSSNR.Hint.Render.Preset", &preset) != NVSDK_NGX_Result_Success || preset != 0)
        return invalid();
    if (requested("parameter")) {
        static void* fault_table[16];
        auto table = reinterpret_cast<void***>(parameters);
        std::memcpy(fault_table, *table, sizeof(fault_table));
        fault_table[0] = reinterpret_cast<void*>(&parameter_fault);
        *table = fault_table;
    }
    handles[pass] = reinterpret_cast<NVSDK_NGX_Handle*>(next_handle += 0x100);
    stats.handles[pass] = reinterpret_cast<uint64_t>(handles[pass]);
    *handle = handles[pass];
    ++stats.created;
    ++stats.live_features;
    return NVSDK_NGX_Result_Success;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_EvaluateFeature(
    VkCommandBuffer, const NVSDK_NGX_Handle* handle, const NVSDK_NGX_Parameter* parameters, void*) {
    if (requested("exception")) RaiseException(EXCEPTION_ACCESS_VIOLATION, 0, 0, nullptr);
    if (requested("evaluate")) return NVSDK_NGX_Result_FAIL_Failure;
    uint32_t pass = handle == handles[0] ? 0 : 1;
    if (handle != handles[pass] || pass != next_pass) return invalid();
    unsigned inverted = 1, reset = 0, reset_alias = 0;
    float scale_x = 0, scale_y = 0, sharpness = 0;
    const NVSDK_NGX_Resource_VK *color = nullptr, *output = nullptr, *depth = nullptr, *motion = nullptr;
    if (parameters->Get("DLSSNR.DepthInverted", &inverted) != NVSDK_NGX_Result_Success || inverted != 0 ||
        parameters->Get("DLSSNR.MVecScaleX", &scale_x) != NVSDK_NGX_Result_Success || scale_x != 1 ||
        parameters->Get("DLSSNR.MVecScaleY", &scale_y) != NVSDK_NGX_Result_Success || scale_y != 1 ||
        parameters->Get("Sharpness", &sharpness) != NVSDK_NGX_Result_Success || sharpness != expected.sharpness ||
        parameters->Get("DLSSNR.Reset", &reset) != NVSDK_NGX_Result_Success || reset != expected_reset ||
        parameters->Get("Reset", &reset_alias) != NVSDK_NGX_Result_Success || reset_alias != reset ||
        parameters->Get("DLSSNR.Color", &color) != NVSDK_NGX_Result_Success || !color || color->ReadWrite ||
        parameters->Get("DLSSNR.Output", &output) != NVSDK_NGX_Result_Success || !output || !output->ReadWrite ||
        parameters->Get("DLSSNR.Depth", &depth) != NVSDK_NGX_Result_Success || !depth || depth->ReadWrite ||
        parameters->Get("DLSSNR.MVec", &motion) != NVSDK_NGX_Result_Success || !motion || motion->ReadWrite)
        return invalid();
    VkFormat input_format = pass ? VK_FORMAT_R16G16B16A16_UNORM : VK_FORMAT_R8G8B8A8_UNORM;
    VkFormat output_format = expected.passes == 2 ? VK_FORMAT_R16G16B16A16_UNORM : VK_FORMAT_R8G8B8A8_UNORM;
    if (color->Resource.ImageViewInfo.Format != input_format || output->Resource.ImageViewInfo.Format != output_format ||
        depth->Resource.ImageViewInfo.Format != VK_FORMAT_R32_SFLOAT ||
        motion->Resource.ImageViewInfo.Format != VK_FORMAT_R16G16_SFLOAT) return invalid();
    if (pass && (color->Resource.ImageViewInfo.Image != chain || depth->Resource.ImageViewInfo.Image != depth_image ||
                 motion->Resource.ImageViewInfo.Image != motion_image)) return invalid();
    chain = output->Resource.ImageViewInfo.Image;
    depth_image = depth->Resource.ImageViewInfo.Image;
    motion_image = motion->Resource.ImageViewInfo.Image;
    ++stats.evaluations[pass];
    stats.reset[pass] = reset;
    next_pass = (pass + 1) % expected.passes;
    return NVSDK_NGX_Result_Success;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_ReleaseFeature(NVSDK_NGX_Handle* handle) {
    uint32_t pass = handle == handles[0] ? 0 : 1;
    if (!handle || handles[pass] != handle) return invalid();
    handles[pass] = nullptr;
    ++stats.released;
    ++stats.releases[pass];
    --stats.live_features;
    return NVSDK_NGX_Result_Success;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_Shutdown1(VkDevice) {
    ++stats.shutdowns;
    next_pass = 0;
    return stats.live_features ? invalid() : NVSDK_NGX_Result_Success;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_GetFeatureInstanceExtensionRequirements(
    const void*, unsigned* count, VkExtensionProperties** output) {
    static VkExtensionProperties properties[] = {{"VK_KHR_get_physical_device_properties2", 2}};
    *count = 1;
    *output = properties;
    return NVSDK_NGX_Result_Success;
}

extern "C" __declspec(dllexport) NVSDK_NGX_Result NVSDK_NGX_VULKAN_GetFeatureDeviceExtensionRequirements(
    VkInstance, VkPhysicalDevice, const void*, unsigned* count, VkExtensionProperties** output) {
    static VkExtensionProperties properties[] = {{"VK_NVX_binary_import", 1}, {"VK_NVX_image_view_handle", 2},
                                                {"VK_KHR_buffer_device_address", 1}, {"VK_KHR_push_descriptor", 2}};
    *count = 4;
    *output = properties;
    return NVSDK_NGX_Result_Success;
}
