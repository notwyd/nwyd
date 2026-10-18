#!/usr/bin/env bash
set -euo pipefail
source_dir="${1:?Pass the pinned DLSS5VKLayer source directory}"
output_dir="${2:?Pass a test output directory}"
adapter_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$adapter_dir/../build-neural-runtime.sh" --source "$source_dir" --output "$output_dir"
mkdir -p "$output_dir/mock"
compiler="${NR_CXX:-x86_64-w64-mingw32-g++}"
"$compiler" -std=c++17 -O2 -shared -static \
    -I "$source_dir/core" -I "$source_dir/standalone_runner/third_party" \
    "$adapter_dir/mock_runtime.cpp" -o "$output_dir/mock/nvngx_dlssnr.dll"
"$compiler" -std=c++17 -O2 -static -municode \
    -I "$source_dir/standalone_runner/third_party" \
    "$adapter_dir/self_test.cpp" -o "$output_dir/self_test.exe"
echo "Run self_test.exe with the absolute Windows paths to nwyd_neural.dll and the mock directory."
