#!/usr/bin/env bash
set -euo pipefail

source_dir=""
output_dir=""
while (($#)); do
    case "$1" in
        --source) source_dir="${2:?--source requires a directory}"; shift 2 ;;
        --output) output_dir="${2:?--output requires a directory}"; shift 2 ;;
        *) echo "Usage: $0 --source DLSS5VKLayer --output DIRECTORY" >&2; exit 2 ;;
    esac
done

if [[ -z "$source_dir" || -z "$output_dir" ]]; then
    echo "Usage: $0 --source DLSS5VKLayer --output DIRECTORY" >&2
    exit 2
fi

expected_commit=117c9530834d37ef8dc7b371e36cc3fb507e6b6b
if [[ "$(git -C "$source_dir" rev-parse HEAD)" != "$expected_commit" ]]; then
    echo "DLSS5VKLayer must be checked out at $expected_commit" >&2
    exit 1
fi
if ! git -C "$source_dir" diff --quiet HEAD -- core common/shm_protocol.h standalone_runner/third_party/vulkan; then
    echo "DLSS5VKLayer input files have uncommitted changes" >&2
    exit 1
fi

source_dir="$(cd "$source_dir" && pwd)"
adapter_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/neural-runtime" && pwd)"
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
compiler="${NR_CXX:-x86_64-w64-mingw32-g++}"
if [[ "$("$compiler" -dumpmachine)" != x86_64-w64-mingw32 ]]; then
    echo "NR_CXX must use the x86_64 MinGW GNU C++ ABI" >&2
    exit 1
fi

"$compiler" -std=c++17 -O2 -shared -static -pthread \
    -I "$source_dir/core" -I "$source_dir/standalone_runner/third_party" \
    -I "$adapter_dir" -include "$adapter_dir/overlay.h" \
    "$adapter_dir/adapter.cpp" "$source_dir/core/ngx_snippet.cpp" "$source_dir/core/guard.cpp" \
    -Wl,--out-implib,"$output_dir/libnwyd_neural.a" \
    -o "$output_dir/nwyd_neural.dll"

cp "$adapter_dir/LICENSE" "$output_dir/LICENSE-neural-runtime.txt"
cp "$adapter_dir/NOTICE" "$output_dir/NOTICE-neural-runtime.txt"
cp "$source_dir/ATTRIBUTION.md" "$output_dir/ATTRIBUTION-DLSS5VKLayer.txt"
cp "$adapter_dir/nwyd_neural.h" "$output_dir/nwyd_neural.h"
echo "Built $output_dir/nwyd_neural.dll"
