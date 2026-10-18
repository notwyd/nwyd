#pragma once
#include <stdint.h>

#if defined(_WIN32)
#define NWYD_NR_EXPORT __declspec(dllexport)
#else
#define NWYD_NR_EXPORT
#endif

#define NWYD_NR_ABI_VERSION 2u
#define NWYD_NR_MAX_INSTANCE_EXTENSIONS 8u
#define NWYD_NR_MAX_DEVICE_EXTENSIONS 16u
#define NWYD_NR_EXTENSION_NAME_SIZE 256u

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NwydNrContext NwydNrContext;

typedef enum NwydNrResult {
    NWYD_NR_SUCCESS = 0,
    NWYD_NR_INVALID_ARGUMENT = 1,
    NWYD_NR_BUSY = 2,
    NWYD_NR_UNAVAILABLE = 3,
    NWYD_NR_UNSUPPORTED = 4,
    NWYD_NR_RUNTIME_FAILURE = 5,
    NWYD_NR_EXCEPTION = 6
} NwydNrResult;

typedef struct NwydNrRequirements {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t instance_extension_count;
    uint32_t device_extension_count;
    uint32_t minimum_width;
    uint32_t minimum_height;
    char instance_extensions[NWYD_NR_MAX_INSTANCE_EXTENSIONS][NWYD_NR_EXTENSION_NAME_SIZE];
    char device_extensions[NWYD_NR_MAX_DEVICE_EXTENSIONS][NWYD_NR_EXTENSION_NAME_SIZE];
} NwydNrRequirements;

typedef struct NwydNrTuning {
    float intensity;
    float tone;
    float structure;
    float skin;
    float sharpness;
    uint32_t auto_mask;
    uint32_t style;
    uint32_t passes;
} NwydNrTuning;

typedef struct NwydNrCreateInfo {
    uint32_t struct_size;
    uint32_t abi_version;
    const uint16_t* runtime_directory;
    uint64_t instance;
    uint64_t physical_device;
    uint64_t device;
    uint64_t recording_command_buffer;
    uint64_t get_instance_proc_addr;
    uint64_t get_device_proc_addr;
    uint32_t width;
    uint32_t height;
    NwydNrTuning tuning;
} NwydNrCreateInfo;

typedef struct NwydNrImage {
    uint64_t image;
    uint64_t image_view;
    uint32_t format;
    uint32_t aspect_mask;
    uint32_t width;
    uint32_t height;
} NwydNrImage;

typedef struct NwydNrFrame {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t recording_command_buffer;
    NwydNrImage color;
    NwydNrImage depth;
    NwydNrImage motion;
    NwydNrImage output;
    uint32_t reset;
    uint32_t pass_index;
} NwydNrFrame;

NWYD_NR_EXPORT uint32_t nwyd_nr_abi_version(void);
NWYD_NR_EXPORT int32_t nwyd_nr_probe(const uint16_t* runtime_directory, NwydNrRequirements* requirements,
                                   char* error, uint32_t error_capacity);
NWYD_NR_EXPORT int32_t nwyd_nr_create(const NwydNrCreateInfo* info, NwydNrContext** context,
                                    char* error, uint32_t error_capacity);
NWYD_NR_EXPORT int32_t nwyd_nr_evaluate(NwydNrContext* context, const NwydNrFrame* frame,
                                      char* error, uint32_t error_capacity);
NWYD_NR_EXPORT int32_t nwyd_nr_destroy(NwydNrContext* context, char* error, uint32_t error_capacity);

#ifdef __cplusplus
}
#endif
