#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary=""
bundle_dir=""
output=""
while (($#)); do
    case "$1" in
        --binary) binary="${2:?--binary requires a file}"; shift 2 ;;
        --bundle) bundle_dir="${2:?--bundle requires a directory}"; shift 2 ;;
        --output) output="${2:?--output requires a ZIP path}"; shift 2 ;;
        *) echo "Usage: $0 --binary FILE --bundle DIRECTORY --output FILE.zip" >&2; exit 2 ;;
    esac
done

if [[ -z "$binary" || -z "$bundle_dir" || "$output" != *.zip ]]; then
    echo "Usage: $0 --binary FILE --bundle DIRECTORY --output FILE.zip" >&2
    exit 2
fi
if [[ -d "$output" ]]; then
    echo "The DLSS package output must be a file: $output" >&2
    exit 2
fi
command -v zip >/dev/null || { echo 'DLSS packaging requires zip.' >&2; exit 1; }

inputs=("$binary" "$repo_dir/LICENSE" "$repo_dir/NOTICE" "$repo_dir/crates/nwyd/assets/LiberationSans-LICENSE.txt")
names=(nwyd-dlss.exe LICENSE.txt NOTICE.txt LiberationSans-LICENSE.txt)
for name in LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt; do
    inputs+=("$bundle_dir/$name")
    names+=("$name")
done
if [[ -e "$bundle_dir/LICENSE-nvidia-runtime.txt" ]]; then
    inputs+=("$bundle_dir/LICENSE-nvidia-runtime.txt")
    names+=(LICENSE-nvidia-runtime.txt)
fi

output="$(realpath -m -- "$output")"
for input in "${inputs[@]}"; do
    if [[ ! -f "$input" || ! -s "$input" ]]; then
        echo "Missing or empty DLSS package input: $input" >&2
        exit 1
    fi
    if [[ "$(realpath -- "$input")" == "$output" ]]; then
        echo 'The DLSS package output must not overwrite an input file.' >&2
        exit 2
    fi
done

mkdir -p -- "$(dirname "$output")"
staging_dir="$(mktemp -d "${output}.stage.XXXXXX")"
trap 'rm -rf -- "$staging_dir"' EXIT
mkdir "$staging_dir/package"
for index in "${!inputs[@]}"; do
    cp -- "${inputs[$index]}" "$staging_dir/package/${names[$index]}"
done
(
    cd "$staging_dir/package"
    zip -0 -q -X "$staging_dir/archive.zip" "${names[@]}"
)
mv -fT -- "$staging_dir/archive.zip" "$output"
printf 'Packaged Windows DLSS build in %s\n' "$output"
