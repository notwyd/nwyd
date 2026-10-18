#include "nwyd_neural.h"
#include "mock_contract.h"
#include <windows.h>
#include <vulkan/vulkan.h>
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <limits>
#include <string>

static void require(bool condition, const char* what, const char* error = "") {
    if (!condition) { std::fprintf(stderr, "FAIL: %s: %s\n", what, error); ExitProcess(1); }
}

static void VKAPI_CALL properties(VkPhysicalDevice, VkPhysicalDeviceProperties* output) {
    *output = {};
    output->vendorID = 0x10de;
    output->limits.maxImageDimension2D = 16384;
}

static PFN_vkVoidFunction VKAPI_CALL get_instance_proc(VkInstance, const char* name) {
    return std::strcmp(name, "vkGetPhysicalDeviceProperties") ? nullptr : reinterpret_cast<PFN_vkVoidFunction>(&properties);
}

static LONG WINAPI after_unload(EXCEPTION_POINTERS* exception) {
    return exception->ExceptionRecord->ExceptionCode == 0x12345 ? EXCEPTION_CONTINUE_EXECUTION : EXCEPTION_CONTINUE_SEARCH;
}

int wmain(int argc, wchar_t** argv) {
    require(argc == 3, "adapter and mock runtime directories required");
    HMODULE library = LoadLibraryW(argv[1]);
    require(library != nullptr, "adapter loads");
    HMODULE mock = LoadLibraryW((std::wstring(argv[2]) + L"\\nvngx_dlssnr.dll").c_str());
    require(mock != nullptr, "mock loads");
    auto version = reinterpret_cast<decltype(&nwyd_nr_abi_version)>(GetProcAddress(library, "nwyd_nr_abi_version"));
    auto probe = reinterpret_cast<decltype(&nwyd_nr_probe)>(GetProcAddress(library, "nwyd_nr_probe"));
    auto create = reinterpret_cast<decltype(&nwyd_nr_create)>(GetProcAddress(library, "nwyd_nr_create"));
    auto evaluate = reinterpret_cast<decltype(&nwyd_nr_evaluate)>(GetProcAddress(library, "nwyd_nr_evaluate"));
    auto destroy = reinterpret_cast<decltype(&nwyd_nr_destroy)>(GetProcAddress(library, "nwyd_nr_destroy"));
    auto reset_mock = reinterpret_cast<NwydNrMockReset>(GetProcAddress(mock, "nwyd_mock_reset"));
    auto snapshot = reinterpret_cast<NwydNrMockSnapshot>(GetProcAddress(mock, "nwyd_mock_snapshot"));
    auto expect_reset = reinterpret_cast<NwydNrMockExpectReset>(GetProcAddress(mock, "nwyd_mock_expect_reset"));
    require(version && probe && create && evaluate && destroy && reset_mock && snapshot && expect_reset, "ABI exports exist");
    require(version() == NWYD_NR_ABI_VERSION, "ABI version matches");
    char error[512]{};
    NwydNrRequirements requirements{};
    requirements.struct_size = sizeof(requirements);
    requirements.abi_version = NWYD_NR_ABI_VERSION;
    const auto* directory = reinterpret_cast<const uint16_t*>(argv[2]);
    require(probe(nullptr, &requirements, error, sizeof(error)) == NWYD_NR_UNAVAILABLE, "missing runtime rejected");
    require(probe(directory, &requirements, error, sizeof(error)) == NWYD_NR_SUCCESS, "requirements probe", error);
    require(requirements.instance_extension_count == 1 && requirements.device_extension_count == 4, "extension counts");
    require(std::strcmp(requirements.device_extensions[2], "VK_KHR_buffer_device_address") == 0, "runtime extension names");
    NwydNrCreateInfo info{};
    info.struct_size = sizeof(info);
    info.abi_version = NWYD_NR_ABI_VERSION;
    info.runtime_directory = directory;
    info.instance = info.physical_device = info.device = info.recording_command_buffer = 1;
    info.get_instance_proc_addr = reinterpret_cast<uint64_t>(&get_instance_proc);
    info.get_device_proc_addr = 1;
    info.width = info.height = 64;
    const NwydNrTuning defaults{2.0f, 2.0f, 2.05f, -1.0f, 0.25f, 1, 2, 2};
    info.tuning = defaults;
    require(reset_mock(&defaults), "mock starts clean");
    NwydNrContext* context = nullptr;
    info.width = 1;
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT && !context, "tiny raster rejected");
    info.width = 64;
    info.tuning.intensity = std::numeric_limits<float>::quiet_NaN();
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT && !context, "nonfinite tuning rejected");
    info.tuning = defaults;
    info.tuning.passes = 0;
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT && !context, "empty chain rejected");
    info.tuning.passes = 3;
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT && !context, "oversized chain rejected");
    info.tuning = defaults;
    info.tuning.style = 3;
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT && !context, "unknown style rejected");
    info.tuning = defaults;
    SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", "create");
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_RUNTIME_FAILURE && !context, "failed first create cleaned up", error);
    SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", "create_second");
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_RUNTIME_FAILURE && !context, "failed second create rejects whole chain", error);
    NwydNrMockStats stats{};
    snapshot(&stats);
    require(stats.created == 1 && stats.released == 1 && !stats.live_features && !stats.invalid_calls, "partial chain releases first feature");
    SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", nullptr);
    require(reset_mock(&defaults), "failed creates leave no resources");
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_SUCCESS && context, "create tuning and caller identity", error);
    snapshot(&stats);
    require(stats.created == 2 && stats.live_features == 2 && stats.handles[0] && stats.handles[1] &&
            stats.handles[0] != stats.handles[1], "two independent temporal feature handles");
    NwydNrContext* second = nullptr;
    require(create(&info, &second, error, sizeof(error)) == NWYD_NR_BUSY && !second, "second context rejected");
    NwydNrFrame frame{};
    frame.struct_size = sizeof(frame);
    frame.abi_version = NWYD_NR_ABI_VERSION;
    frame.recording_command_buffer = 1;
    frame.color = {1, 1, VK_FORMAT_R8G8B8A8_UNORM, VK_IMAGE_ASPECT_COLOR_BIT, 64, 64};
    frame.depth = {2, 2, VK_FORMAT_R32_SFLOAT, VK_IMAGE_ASPECT_COLOR_BIT, 64, 64};
    frame.motion = {3, 3, VK_FORMAT_R16G16_SFLOAT, VK_IMAGE_ASPECT_COLOR_BIT, 64, 64};
    frame.output = {4, 4, VK_FORMAT_R16G16B16A16_UNORM, VK_IMAGE_ASPECT_COLOR_BIT, 64, 64};
    frame.reset = 1;
    NwydNrFrame pass_two = frame;
    pass_two.color = frame.output;
    pass_two.output.image = pass_two.output.image_view = 5;
    pass_two.pass_index = 1;
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "second pass cannot run first");
    frame.output.format = VK_FORMAT_R8G8B8A8_UNORM;
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "quantized intermediate rejected");
    frame.output.format = VK_FORMAT_R16G16B16A16_UNORM;
    frame.output.image = frame.color.image;
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "aliased output rejected");
    frame.output.image = 4;
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_SUCCESS, "first pass resources and sharpness", error);
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "duplicate first pass rejected");
    pass_two.reset = 0;
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "inconsistent chain reset rejected");
    pass_two.reset = 1;
    pass_two.depth.image = 6;
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "different second-pass depth rejected");
    pass_two.depth.image = 2;
    pass_two.color.image = 6;
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_INVALID_ARGUMENT, "unrelated second-pass input rejected");
    pass_two.color.image = 4;
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_SUCCESS, "second pass chain and sharpness", error);
    frame.reset = pass_two.reset = 0;
    expect_reset(0);
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_SUCCESS, "first history survives next frame", error);
    require(evaluate(context, &pass_two, error, sizeof(error)) == NWYD_NR_SUCCESS, "second history survives next frame", error);
    snapshot(&stats);
    require(stats.evaluations[0] == 2 && stats.evaluations[1] == 2 && !stats.reset[0] && !stats.reset[1] && !stats.invalid_calls,
            "both passes receive frame reset and run exactly once per frame");
    require(destroy(context, error, sizeof(error)) == NWYD_NR_SUCCESS, "two-pass teardown", error);
    snapshot(&stats);
    require(stats.releases[0] == 1 && stats.releases[1] == 1 && !stats.live_features && !stats.invalid_calls,
            "both independent histories are released exactly once");
    for (const char* failure : {"evaluate", "exception", "parameter"}) {
        require(reset_mock(&defaults), "failure test starts clean");
        if (!std::strcmp(failure, "parameter")) SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", failure);
        require(create(&info, &context, error, sizeof(error)) == NWYD_NR_SUCCESS, "recreate before failure", error);
        SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", nullptr);
        frame.reset = pass_two.reset = 1;
        const bool parameter = !std::strcmp(failure, "parameter");
        if (!parameter) require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_SUCCESS, "first pass before second-pass failure", error);
        SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", failure);
        int32_t result = evaluate(context, parameter ? &frame : &pass_two, error, sizeof(error));
        require(result == (!std::strcmp(failure, "evaluate") ? NWYD_NR_RUNTIME_FAILURE : NWYD_NR_EXCEPTION),
                "evaluation and parameter failures fail closed", error);
        SetEnvironmentVariableA("NWYD_NR_TEST_FAILURE", nullptr);
        require(evaluate(context, parameter ? &frame : &pass_two, error, sizeof(error)) == NWYD_NR_RUNTIME_FAILURE,
                "failure latches fallback for entire chain");
        require(destroy(context, error, sizeof(error)) == NWYD_NR_SUCCESS, "failed chain teardown", error);
        snapshot(&stats);
        require(stats.released == 2 && !stats.live_features && !stats.invalid_calls, "failed chain releases both histories");
    }
    info.tuning = {1.5f, 1.25f, 1.75f, 0.5f, 0.75f, 0, 1, 1};
    require(reset_mock(&info.tuning), "retuning starts clean");
    require(create(&info, &context, error, sizeof(error)) == NWYD_NR_SUCCESS, "new create-time tuning replaces previous values", error);
    frame.output.format = VK_FORMAT_R8G8B8A8_UNORM;
    require(evaluate(context, &frame, error, sizeof(error)) == NWYD_NR_SUCCESS, "single-pass format and sharpness", error);
    require(destroy(context, error, sizeof(error)) == NWYD_NR_SUCCESS, "single-pass teardown", error);
    snapshot(&stats);
    require(stats.created == 1 && stats.released == 1 && !stats.live_features && !stats.invalid_calls, "retuned single pass cleaned up");
    FreeLibrary(mock);
    FreeLibrary(library);
    PVOID handler = AddVectoredExceptionHandler(0, after_unload);
    RaiseException(0x12345, 0, 0, nullptr);
    RemoveVectoredExceptionHandler(handler);
    std::puts("Native adapter tests passed: ABI, creation tuning, independent passes, precision chain, reset, sharpness, cleanup, exceptions and unload");
    return 0;
}

static_assert(sizeof(NwydNrTuning) == 32);
static_assert(sizeof(NwydNrCreateInfo) == 104);
static_assert(offsetof(NwydNrCreateInfo, tuning) == 72);
static_assert(sizeof(NwydNrFrame) == 152);
static_assert(offsetof(NwydNrFrame, pass_index) == 148);
