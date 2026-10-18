#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch_dir="$(mktemp -d)"
trap 'rm -rf -- "$scratch_dir"' EXIT
binary="$scratch_dir/build files/nwyd.exe"
bundle_dir="$scratch_dir/bundle files"
output="$scratch_dir/release files/nwyd dlss.zip"
mkdir -p "$(dirname "$binary")" "$bundle_dir/wine"
printf 'synthetic bundled executable\n' > "$binary"
for name in LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt LICENSE-nvidia-runtime.txt; do
    printf '%s\n' "$name" > "$bundle_dir/$name"
done
for name in nwyd_neural.dll nvngx_dlssnr.dll nvapi64.dll dxgi.dll private.txt; do
    printf 'unpackaged input\n' > "$bundle_dir/$name"
    printf 'unpackaged input\n' > "$bundle_dir/wine/$name"
done
package=("$repo_dir/tools/package-dlss.sh" --binary "$binary" --bundle "$bundle_dir" --output "$output")
"${package[@]}"
unzip -tq "$output"
unzip -Z1 "$output" | LC_ALL=C sort > "$scratch_dir/actual"
printf '%s\n' nwyd-dlss.exe LICENSE.txt NOTICE.txt LiberationSans-LICENSE.txt LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt LICENSE-nvidia-runtime.txt | LC_ALL=C sort > "$scratch_dir/expected"
cmp "$scratch_dir/expected" "$scratch_dir/actual"
unzip -p "$output" nwyd-dlss.exe | cmp "$binary" -
unzip -p "$output" LICENSE.txt | cmp "$repo_dir/LICENSE" -
unzip -p "$output" NOTICE.txt | cmp "$repo_dir/NOTICE" -
unzip -p "$output" LiberationSans-LICENSE.txt | cmp "$repo_dir/crates/nwyd/assets/LiberationSans-LICENSE.txt" -
for name in LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt LICENSE-nvidia-runtime.txt; do
    unzip -p "$output" "$name" | cmp "$bundle_dir/$name" -
done

printf 'stale entry\n' > "$scratch_dir/stale.dll"
zip -q -j "$output" "$scratch_dir/stale.dll"
rm "$bundle_dir/LICENSE-nvidia-runtime.txt"
printf 'updated executable\n' > "$binary"
"${package[@]}"
unzip -p "$output" nwyd-dlss.exe | cmp "$binary" -
unzip -Z1 "$output" | LC_ALL=C sort > "$scratch_dir/actual"
printf '%s\n' nwyd-dlss.exe LICENSE.txt NOTICE.txt LiberationSans-LICENSE.txt LICENSE-neural-runtime.txt NOTICE-neural-runtime.txt ATTRIBUTION-DLSS5VKLayer.txt | LC_ALL=C sort > "$scratch_dir/expected"
cmp "$scratch_dir/expected" "$scratch_dir/actual"
cp "$output" "$scratch_dir/original.zip"

expect_failure() {
    local expected="$1"
    shift
    if "$@" > "$scratch_dir/error" 2>&1; then
        echo "Expected DLSS packaging to fail: $expected" >&2
        exit 1
    fi
    grep -F -- "$expected" "$scratch_dir/error" >/dev/null
    cmp "$scratch_dir/original.zip" "$output"
}

mv "$binary" "$binary.saved"
expect_failure 'nwyd.exe' "${package[@]}"
mv "$binary.saved" "$binary"
mv "$bundle_dir/NOTICE-neural-runtime.txt" "$bundle_dir/saved.txt"
expect_failure 'NOTICE-neural-runtime.txt' "${package[@]}"
mv "$bundle_dir/saved.txt" "$bundle_dir/NOTICE-neural-runtime.txt"
: > "$binary"
expect_failure 'nwyd.exe' "${package[@]}"
printf 'synthetic bundled executable\n' > "$binary"
: > "$bundle_dir/LICENSE-nvidia-runtime.txt"
expect_failure 'LICENSE-nvidia-runtime.txt' "${package[@]}"
rm "$bundle_dir/LICENSE-nvidia-runtime.txt"
expect_failure 'Usage:' "${package[@]}" --unexpected
expect_failure 'requires a ZIP path' "${package[@]}" --output
expect_failure 'must not overwrite an input' "$repo_dir/tools/package-dlss.sh" --binary "$output" --bundle "$bundle_dir" --output "$output"
mkdir "$scratch_dir/directory.zip"
expect_failure 'must be a file' "${package[@]}" --output "$scratch_dir/directory.zip"

mkdir "$scratch_dir/bin"
cat > "$scratch_dir/bin/zip" <<'STUB'
#!/usr/bin/env bash
echo 'simulated zip failure' >&2
exit 1
STUB
chmod +x "$scratch_dir/bin/zip"
expect_failure 'simulated zip failure' env PATH="$scratch_dir/bin:$PATH" "${package[@]}"
shopt -s nullglob
staging_directories=("$output".stage.*)
test "${#staging_directories[@]}" -eq 0
printf 'DLSS package tests passed.\n'
