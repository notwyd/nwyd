#pragma once
#include "nwyd_neural.h"

struct NwydNrMockStats {
    uint32_t create_attempts;
    uint32_t created;
    uint32_t released;
    uint32_t live_features;
    uint32_t evaluations[2];
    uint32_t reset[2];
    uint32_t releases[2];
    uint32_t shutdowns;
    uint32_t invalid_calls;
    uint64_t handles[2];
};

using NwydNrMockReset = bool (*)(const NwydNrTuning*);
using NwydNrMockSnapshot = void (*)(NwydNrMockStats*);
using NwydNrMockExpectReset = void (*)(uint32_t);
