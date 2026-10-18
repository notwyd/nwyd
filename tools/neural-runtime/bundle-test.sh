#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT
adapter_dir="$scratch_dir/adapter files"
runtime_dir="$scratch_dir/runtime files"
bundle_dir="$scratch_dir/bundle files"
mkdir -p "$adapter_dir" "$runtime_dir/wine" "$bundle_dir/wine"

for name in nwyd_neural.dll LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt; do
    printf '%s\n' "$name" > "$adapter_dir/$name"
done
printf 'nvidia runtime\n' > "$runtime_dir/nvngx_dlssnr.dll"
printf 'nvidia license\n' > "$runtime_dir/LICENSE-nvidia-runtime.txt"
for name in nvapi64.dll dxgi.dll LICENSE-nvapi.txt LICENSE-dxgi.txt; do
    printf '%s\n' "$name" > "$runtime_dir/$name"
    printf '%s\n' "$name" > "$runtime_dir/wine/$name"
    printf 'stale %s\n' "$name" > "$bundle_dir/$name"
    printf 'stale %s\n' "$name" > "$bundle_dir/wine/$name"
done
printf 'extra input DLL\n' > "$adapter_dir/extra.dll"
printf 'extra input DLL\n' > "$runtime_dir/extra.dll"
printf 'stale helper DLL\n' > "$bundle_dir/wine/other.dll"
stage=("$repo_dir/tools/prepare-neural-bundle.sh" --adapter "$adapter_dir" --runtime "$runtime_dir" --output "$bundle_dir")

"${stage[@]}"
for name in nwyd_neural.dll LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt; do
    cmp "$adapter_dir/$name" "$bundle_dir/$name"
done
cmp "$runtime_dir/nvngx_dlssnr.dll" "$bundle_dir/nvngx_dlssnr.dll"
cmp "$runtime_dir/LICENSE-nvidia-runtime.txt" "$bundle_dir/LICENSE-nvidia-runtime.txt"
for name in nvapi64.dll dxgi.dll LICENSE-nvapi.txt LICENSE-dxgi.txt; do
    test ! -e "$bundle_dir/$name"
done
test ! -e "$bundle_dir/wine"
test ! -e "$bundle_dir/extra.dll"
test -s "$runtime_dir/wine/nvapi64.dll"

expect_failure() {
    local expected="$1"
    shift
    if "$@" > "$scratch_dir/error" 2>&1; then
        echo "Expected bundle staging to fail: $expected" >&2
        exit 1
    fi
    grep -F -- "$expected" "$scratch_dir/error" > /dev/null
}

cp "$bundle_dir/nvngx_dlssnr.dll" "$scratch_dir/original-runtime"
printf 'next runtime generation\n' > "$runtime_dir/nvngx_dlssnr.dll"
expect_failure 'Usage:' "${stage[@]}" --wine-runtime "$runtime_dir"
cmp "$scratch_dir/original-runtime" "$bundle_dir/nvngx_dlssnr.dll"
expect_failure 'must not contain or equal' "$repo_dir/tools/prepare-neural-bundle.sh" --adapter "$adapter_dir" --runtime "$runtime_dir" --output "$runtime_dir"
expect_failure 'must not contain or equal' "$repo_dir/tools/prepare-neural-bundle.sh" --adapter "$adapter_dir" --runtime "$runtime_dir" --output "$scratch_dir"
test -s "$runtime_dir/wine/nvapi64.dll"
mv "$runtime_dir/nvngx_dlssnr.dll" "$runtime_dir/saved.dll"
expect_failure 'nvngx_dlssnr.dll' "${stage[@]}"
cmp "$scratch_dir/original-runtime" "$bundle_dir/nvngx_dlssnr.dll"
mv "$runtime_dir/saved.dll" "$runtime_dir/nvngx_dlssnr.dll"
mv "$adapter_dir/NOTICE-neural-runtime.txt" "$adapter_dir/saved.txt"
expect_failure 'NOTICE-neural-runtime.txt' "${stage[@]}"
mv "$adapter_dir/saved.txt" "$adapter_dir/NOTICE-neural-runtime.txt"
expect_failure '--runtime' "$repo_dir/tools/prepare-neural-bundle.sh" --adapter "$adapter_dir" --output "$bundle_dir"
printf '' > "$runtime_dir/nvngx_dlssnr.dll"
expect_failure 'nvngx_dlssnr.dll' "${stage[@]}"
printf 'updated runtime\n' > "$runtime_dir/nvngx_dlssnr.dll"
rm "$runtime_dir/LICENSE-nvidia-runtime.txt"
"${stage[@]}"
cmp "$runtime_dir/nvngx_dlssnr.dll" "$bundle_dir/nvngx_dlssnr.dll"
test ! -e "$bundle_dir/LICENSE-nvidia-runtime.txt"
test ! -e "$bundle_dir/wine"
expect_failure 'NR_RUNTIME' make -C "$repo_dir" prepare-neural-bundle NR_RUNTIME=
expect_failure 'nvngx_dlssnr.dll' make -C "$repo_dir" prepare-neural-bundle NR_RUNTIME="$scratch_dir/absent-runtime"
expect_failure 'NR_SOURCE' make -C "$repo_dir" prepare-neural-bundle NR_RUNTIME="$runtime_dir" NR_SOURCE=

for features in bundled-neural-runtime nwyd/bundled-neural-runtime neural-rendering,bundled-neural-runtime; do
    make -C "$repo_dir" -n release-windows FEATURES="$features" NR_SOURCE="$scratch_dir/source" NR_RUNTIME="$runtime_dir" TARGET_DIR="$scratch_dir/target files" > "$scratch_dir/make"
    grep -F 'prepare-neural-bundle.sh' "$scratch_dir/make" > /dev/null
    grep -F 'NWYD_NEURAL_BUNDLE_DIR=' "$scratch_dir/make" > /dev/null
    grep -F 'neural-adapter' "$scratch_dir/make" > /dev/null
    if grep -F -- '--wine-runtime' "$scratch_dir/make"; then
        echo 'Native Windows build unexpectedly stages Wine compatibility files' >&2
        exit 1
    fi
done
make -C "$repo_dir" -n release-windows ALL_FEATURES=1 NR_SOURCE="$scratch_dir/source" NR_RUNTIME="$runtime_dir" > "$scratch_dir/make"
grep -F 'NWYD_NEURAL_BUNDLE_DIR=' "$scratch_dir/make" > /dev/null
make -C "$repo_dir" -n release-windows FEATURES=neural-rendering NR_SOURCE="$scratch_dir/source" > "$scratch_dir/make"
grep -F 'build-neural-runtime.sh' "$scratch_dir/make" > /dev/null
if grep -E 'prepare-neural-bundle.sh|NWYD_NEURAL_BUNDLE_DIR=' "$scratch_dir/make"; then
    echo 'Unbundled build unexpectedly stages an embedded runtime' >&2
    exit 1
fi
make -C "$repo_dir" -n release-windows > "$scratch_dir/make"
if grep -E 'build-neural-runtime.sh|prepare-neural-bundle.sh|NWYD_NEURAL_BUNDLE_DIR=' "$scratch_dir/make"; then
    echo 'Default build unexpectedly includes a neural runtime' >&2
    exit 1
fi

for target in release-dlss release-windows-dlss; do
    make -C "$repo_dir" -n "$target" FEATURES=analytics NO_DEFAULT_FEATURES=1 NR_SOURCE="$scratch_dir/source" NR_RUNTIME="$runtime_dir" TARGET_DIR="$scratch_dir/target files" DLSS_ZIP="$scratch_dir/package files/dlss.zip" > "$scratch_dir/make"
    grep -F -- '--target-dir "'"$scratch_dir/target files/dlss"'"' "$scratch_dir/make" > /dev/null
    grep -F -- '--no-default-features' "$scratch_dir/make" > /dev/null
    grep -F -- '--features "analytics bundled-neural-runtime"' "$scratch_dir/make" > /dev/null
    grep -F 'NWYD_NEURAL_BUNDLE_DIR="'"$scratch_dir/target files/dlss/neural-bundle"'"' "$scratch_dir/make" > /dev/null
    grep -F -- '--binary "'"$scratch_dir/target files/dlss/x86_64-pc-windows-msvc/release/nwyd.exe"'"' "$scratch_dir/make" > /dev/null
    grep -F -- '--output "'"$scratch_dir/package files/dlss.zip"'"' "$scratch_dir/make" > /dev/null
    if grep -F -- '--target-dir "'"$scratch_dir/target files"'"' "$scratch_dir/make"; then
        echo 'DLSS build unexpectedly uses the regular build directory' >&2
        exit 1
    fi
done
make -C "$repo_dir" -n release-dlss ALL_FEATURES=1 DLSS_TARGET_DIR="$scratch_dir/custom dlss" > "$scratch_dir/make"
grep -F -- '--target-dir "'"$scratch_dir/custom dlss"'"' "$scratch_dir/make" > /dev/null
grep -F -- '--output "'"$scratch_dir/custom dlss/nwyd-dlss-x86_64-pc-windows-msvc.zip"'"' "$scratch_dir/make" > /dev/null
grep -F -- '--all-features' "$scratch_dir/make" > /dev/null
expect_failure 'Windows builds run on Linux' make -C "$repo_dir" release-dlss HOST_OS=Darwin

cat > "$scratch_dir/cargo-stub" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s' "${RUSTFLAGS-}" > "$STUB_LOG.rustflags"
printf '%s' "${CARGO_ENCODED_RUSTFLAGS-}" > "$STUB_LOG.encoded"
printf '%s' "${NWYD_NEURAL_BUNDLE_DIR-}" > "$STUB_LOG.bundle"
printf '%s\n' "$@" > "$STUB_LOG.args"
STUB
chmod +x "$scratch_dir/cargo-stub"
export STUB_LOG="$scratch_dir/cargo"
make -C "$repo_dir" release-windows MAKE=true CARGO="$scratch_dir/cargo-stub" FEATURES=bundled-neural-runtime TARGET_DIR="$scratch_dir/target files" RUSTFLAGS='--cfg bundle_test' > "$scratch_dir/make"
test "$(cat "$STUB_LOG.rustflags")" = '--cfg bundle_test -C target-feature=+crt-static'
test "$(cat "$STUB_LOG.bundle")" = "$scratch_dir/target files/neural-bundle"
test "$(cat "$STUB_LOG.encoded")" = ''
CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS='--cfg target_bundle_test' make -C "$repo_dir" release-windows MAKE=true CARGO="$scratch_dir/cargo-stub" FEATURES=bundled-neural-runtime > "$scratch_dir/make"
test "$(cat "$STUB_LOG.rustflags")" = '--cfg target_bundle_test -C target-feature=+crt-static'
CARGO_ENCODED_RUSTFLAGS=$'--cfg\037bundle_test' make -C "$repo_dir" release-windows MAKE=true CARGO="$scratch_dir/cargo-stub" FEATURES=bundled-neural-runtime RUSTFLAGS='caller flags' > "$scratch_dir/make"
test "$(cat "$STUB_LOG.encoded")" = $'--cfg\037bundle_test\037-Ctarget-feature=+crt-static'
test "$(cat "$STUB_LOG.rustflags")" = 'caller flags'
CARGO_ENCODED_RUSTFLAGS='' make -C "$repo_dir" release-windows MAKE=true CARGO="$scratch_dir/cargo-stub" FEATURES=bundled-neural-runtime > "$scratch_dir/make"
test "$(cat "$STUB_LOG.encoded")" = '-Ctarget-feature=+crt-static'
make -C "$repo_dir" release-windows MAKE=true CARGO="$scratch_dir/cargo-stub" FEATURES=neural-rendering RUSTFLAGS='caller flags' > "$scratch_dir/make"
test "$(cat "$STUB_LOG.rustflags")" = 'caller flags'
test "$(cat "$STUB_LOG.bundle")" = ''
printf 'Neural bundle staging and Makefile tests passed.\n'
