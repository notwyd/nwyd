#pragma once
#include <windows.h>
#include "ngx_abi.h"

#undef NVSDK_NGX_SUCCEED
#define NVSDK_NGX_SUCCEED(value) ((value) == NVSDK_NGX_Result_Success)

namespace nwyd_neural {
PVOID WINAPI register_exception_handler(ULONG first, PVECTORED_EXCEPTION_HANDLER handler);
}

#define AddVectoredExceptionHandler nwyd_neural::register_exception_handler
