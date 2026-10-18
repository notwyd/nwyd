#!/usr/bin/env bash
set -euo pipefail

adapter_dir=""
runtime_dir=""
output_dir=""
while (($#)); do
    case "$1" in
        --adapter) adapter_dir="${2:?--adapter requires a directory}"; shift 2 ;;
        --runtime) runtime_dir="${2:?--runtime requires a directory}"; shift 2 ;;
        --output) output_dir="${2:?--output requires a directory}"; shift 2 ;;
        *) echo "Usage: $0 --adapter DIRECTORY --runtime DIRECTORY --output DIRECTORY" >&2; exit 2 ;;
    esac
done

if [[ -z "$adapter_dir" || -z "$runtime_dir" || -z "$output_dir" ]]; then
    echo "Usage: $0 --adapter DIRECTORY --runtime DIRECTORY --output DIRECTORY" >&2
    exit 2
fi

require_file() {
    if [[ ! -f "$1" || ! -s "$1" ]]; then
        echo "Missing or empty neural bundle input: $1" >&2
        exit 1
    fi
}

adapter_files=(nwyd_neural.dll LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt)
for name in "${adapter_files[@]}"; do
    require_file "$adapter_dir/$name"
done
require_file "$runtime_dir/nvngx_dlssnr.dll"

output_dir="$(realpath -m -- "$output_dir")"
for input_dir in "$adapter_dir" "$runtime_dir"; do
    input_dir="$(realpath -m -- "$input_dir")"
    if [[ "$input_dir" == "$output_dir" || "$input_dir" == "$output_dir/"* ]]; then
        echo "The bundle output must not contain or equal its input directories" >&2
        exit 2
    fi
done
mkdir -p -- "$(dirname "$output_dir")"
staging_dir="$(mktemp -d "${output_dir}.stage.XXXXXX")"
trap 'rm -rf "$staging_dir"' EXIT
for name in "${adapter_files[@]}"; do
    cp -- "$adapter_dir/$name" "$staging_dir/$name"
done
cp -- "$runtime_dir/nvngx_dlssnr.dll" "$staging_dir/nvngx_dlssnr.dll"
if [[ -f "$runtime_dir/LICENSE-nvidia-runtime.txt" ]]; then
    require_file "$runtime_dir/LICENSE-nvidia-runtime.txt"
    cp -- "$runtime_dir/LICENSE-nvidia-runtime.txt" "$staging_dir/LICENSE-nvidia-runtime.txt"
fi

mkdir -p -- "$output_dir"
for name in "${adapter_files[@]}" nvngx_dlssnr.dll LICENSE-nvidia-runtime.txt; do
    if [[ -f "$staging_dir/$name" ]]; then
        mv -f -- "$staging_dir/$name" "$output_dir/$name"
    else
        rm -f -- "$output_dir/$name"
    fi
done
rm -rf -- "$output_dir/wine"
rm -f -- "$output_dir/nvapi64.dll" "$output_dir/dxgi.dll" "$output_dir/LICENSE-nvapi.txt" "$output_dir/LICENSE-dxgi.txt"
printf 'Prepared neural bundle in %s\n' "$output_dir"
