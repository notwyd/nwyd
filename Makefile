.DEFAULT_GOAL := help
SHELL := /bin/sh

CARGO ?= cargo
FEATURES ?=
NO_DEFAULT_FEATURES ?= 0
ALL_FEATURES ?= 0
CARGO_FEATURE_ARGS = $(if $(filter 1,$(NO_DEFAULT_FEATURES)),--no-default-features) $(if $(filter 1,$(ALL_FEATURES)),--all-features) $(if $(strip $(FEATURES)),--features "$(strip $(FEATURES))")
RUSTFMT_TOOLCHAIN ?= nightly
TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
HOST_OS := $(shell uname -s)
HOST_TARGET := $(shell rustc -vV | sed -n 's/^host: //p')
WINDOWS_TARGET := x86_64-pc-windows-msvc
GAME_DIR ?= .
NR_SOURCE ?=
NR_RUNTIME ?=
NEURAL_TARGET_DIR = $(shell realpath -m -- "$(TARGET_DIR)")
NEURAL_RUNTIME_OUTPUT ?= $(NEURAL_TARGET_DIR)/$(WINDOWS_TARGET)/release
NEURAL_RUNTIME_ENABLED = $(if $(filter 1,$(ALL_FEATURES)),1,$(filter neural-rendering nwyd/neural-rendering bundled-neural-runtime nwyd/bundled-neural-runtime,$(subst $(comma), ,$(FEATURES))))
NEURAL_BUNDLE_ENABLED = $(if $(filter 1,$(ALL_FEATURES)),1,$(filter bundled-neural-runtime nwyd/bundled-neural-runtime,$(subst $(comma), ,$(FEATURES))))
DLSS_TARGET_DIR ?= $(TARGET_DIR)/dlss
DLSS_ZIP ?= $(DLSS_TARGET_DIR)/nwyd-dlss-$(WINDOWS_TARGET).zip
comma := ,

.PHONY: help check test fmt fmt-check release run release-linux release-macos release-windows release-dlss release-windows-dlss check-windows package-libs publish-check check-web release-web serve-web test-web release-relay release-relay-windows test-relay build-neural-runtime prepare-neural-bundle test-neural-bundle

help:
	@printf '%s\n' \
	  'make check: check the application and its runtime crates' \
	  'make test: run unit tests for all crates' \
	  'make release: build nwyd for the current Linux or macOS host' \
	  'make run GAME_DIR=/path/to/game: build and run with your game files' \
	  'make release-linux: native Linux release' \
	  'make release-macos: native macOS release' \
	  'make release-windows: cross-compile Windows x64 from Linux with cargo-xwin' \
	  'make release-dlss: build and ZIP the bundled Windows DLSS 5 client separately (alias: release-windows-dlss)' \
	  'make release-relay: build the native desktop relay' \
	  'make release-relay-windows: cross-compile the Windows relay' \
	  'make test-relay: test the relay and desktop UI' \
	  'make check-web: check the WebAssembly target' \
	  'make release-web: build WASM and bundle/minify browser assets in dist' \
	  'make serve-web: serve the browser bundle at http://localhost:8080/' \
	  'make test-web: install browser dependencies and test imports and caching' \
	  'make check-windows: check the Windows x64 cross-build from Linux' \
	  'make fmt / fmt-check: format Rust source / check formatting with nightly rustfmt' \
	  'make package-libs: package and verify all six libraries' \
	  'make publish-check: dry-run library publication to crates.io' \
	  'Feature options: FEATURES="feature1,feature2" NO_DEFAULT_FEATURES=1 ALL_FEATURES=1' \
	  'Experimental Windows DLSS 5: make release-windows FEATURES=neural-rendering NR_SOURCE=/path/to/DLSS5VKLayer' \
	  'Supply nvngx_dlssnr.dll beside nwyd.exe, or set NWYD_NEURAL_RUNTIME_DIR to its directory.' \
	  'Bundled DLSS 5: make release-dlss NR_SOURCE=/path/to/DLSS5VKLayer NR_RUNTIME=/path/to/runtime' \
	  'DLSS output: target/dlss/nwyd-dlss-x86_64-pc-windows-msvc.zip (override DLSS_TARGET_DIR / DLSS_ZIP); requires zip.' \
	  'make test-neural-bundle: test runtime staging, Windows build options and ZIP packaging without vendor binaries' \
	  'Windows prerequisites: cargo-xwin, clang, lld, llvm-rc, llvm-tools and the MSVC Rust target.' \
	  'Linux prerequisites: ALSA, GTK3 and WebKitGTK 4.1 development libraries.' \
	  'macOS prerequisites: Xcode command-line tools.'

check:
	$(CARGO) check --locked --workspace $(CARGO_FEATURE_ARGS)

test:
	$(CARGO) test --locked --workspace $(CARGO_FEATURE_ARGS)

fmt:
	$(CARGO) +$(RUSTFMT_TOOLCHAIN) fmt --all

fmt-check:
	$(CARGO) +$(RUSTFMT_TOOLCHAIN) fmt --all -- --check

release:
	@test "$(HOST_OS)" = Linux -o "$(HOST_OS)" = Darwin || { echo 'Build on Linux or macOS.' >&2; exit 2; }
	$(CARGO) build --locked --release --target "$(HOST_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)

run:
	$(CARGO) build --locked --target "$(HOST_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)
	cd "$(GAME_DIR)" && "$(abspath $(TARGET_DIR))/$(HOST_TARGET)/debug/nwyd"

release-linux:
	@test "$(HOST_OS)" = Linux || { echo 'Run this target on Linux.' >&2; exit 2; }
	$(MAKE) release

release-macos:
	@test "$(HOST_OS)" = Darwin || { echo 'Run this target on macOS.' >&2; exit 2; }
	$(MAKE) release

release-windows:
	@test "$(HOST_OS)" = Linux || { echo 'Windows builds run on Linux using cargo-xwin.' >&2; exit 2; }
ifneq ($(strip $(NEURAL_BUNDLE_ENABLED)),)
	$(MAKE) prepare-neural-bundle
	@if [ "$${CARGO_ENCODED_RUSTFLAGS+x}" ]; then \
	  CARGO_ENCODED_RUSTFLAGS="$${CARGO_ENCODED_RUSTFLAGS:+$${CARGO_ENCODED_RUSTFLAGS}$$(printf '\037')}-Ctarget-feature=+crt-static"; export CARGO_ENCODED_RUSTFLAGS; \
	else \
	  RUSTFLAGS="$${RUSTFLAGS-$${CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS:-}} -C target-feature=+crt-static"; export RUSTFLAGS; \
	fi; \
	NWYD_NEURAL_BUNDLE_DIR="$(NEURAL_TARGET_DIR)/neural-bundle" $(CARGO) xwin build --locked --release --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)
else
	$(if $(NEURAL_RUNTIME_ENABLED),$(MAKE) build-neural-runtime,@:)
	$(CARGO) xwin build --locked --release --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)
endif

release-windows-dlss: release-dlss

release-dlss:
	@test "$(HOST_OS)" = Linux || { echo 'Windows builds run on Linux using cargo-xwin.' >&2; exit 2; }
	@command -v zip >/dev/null || { echo 'Install zip to package the DLSS build.' >&2; exit 2; }
	$(MAKE) release-windows TARGET_DIR="$(DLSS_TARGET_DIR)" FEATURES="$(strip $(FEATURES) bundled-neural-runtime)"
	./tools/package-dlss.sh --binary "$(DLSS_TARGET_DIR)/$(WINDOWS_TARGET)/release/nwyd.exe" --bundle "$(DLSS_TARGET_DIR)/neural-bundle" --output "$(DLSS_ZIP)"

build-neural-runtime:
	@test "$(HOST_OS)" = Linux || { echo 'The experimental Windows runtime builds on Linux using MinGW.' >&2; exit 2; }
	@test -n "$(NR_SOURCE)" || { echo 'Set NR_SOURCE to the pinned DLSS5VKLayer source checkout.' >&2; exit 2; }
	./tools/build-neural-runtime.sh --source "$(NR_SOURCE)" --output "$(NEURAL_RUNTIME_OUTPUT)"

prepare-neural-bundle:
	@test -n "$(NR_RUNTIME)" || { echo 'Set NR_RUNTIME to a directory containing nvngx_dlssnr.dll.' >&2; exit 2; }
	@test -s "$(NR_RUNTIME)/nvngx_dlssnr.dll" || { echo 'NR_RUNTIME must contain nvngx_dlssnr.dll.' >&2; exit 2; }
	$(MAKE) build-neural-runtime NEURAL_RUNTIME_OUTPUT="$(NEURAL_TARGET_DIR)/neural-adapter"
	./tools/prepare-neural-bundle.sh --adapter "$(NEURAL_TARGET_DIR)/neural-adapter" --runtime "$(NR_RUNTIME)" --output "$(NEURAL_TARGET_DIR)/neural-bundle"

test-neural-bundle:
	./tools/neural-runtime/bundle-test.sh
	./tools/neural-runtime/package-test.sh

check-windows:
	@test "$(HOST_OS)" = Linux || { echo 'Windows builds run on Linux using cargo-xwin.' >&2; exit 2; }
	$(CARGO) xwin check --locked --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)

package-libs:
	$(CARGO) package --locked --workspace --exclude nwyd --exclude nwyd-relay $(CARGO_FEATURE_ARGS)

publish-check:
	$(CARGO) publish --dry-run --locked --workspace --exclude nwyd --exclude nwyd-relay $(CARGO_FEATURE_ARGS)

release-relay:
	$(CARGO) build --locked --release --target "$(HOST_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd-relay $(CARGO_FEATURE_ARGS)

release-relay-windows:
	CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-C target-feature=+crt-static" $(CARGO) xwin build --locked --release --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd-relay $(CARGO_FEATURE_ARGS)

test-relay:
	$(CARGO) test --locked -p nwyd-relay --all-targets $(CARGO_FEATURE_ARGS)

check-web:
	$(CARGO) check --locked --target wasm32-unknown-unknown --target-dir "$(TARGET_DIR)" -p nwyd $(CARGO_FEATURE_ARGS)

release-web:
	CARGO="$(CARGO)" CARGO_TARGET_DIR="$(TARGET_DIR)" ./tools/build-web.sh $(CARGO_FEATURE_ARGS)

serve-web: release-web
	python3 -m http.server 8080 --bind 127.0.0.1 --directory dist

test-web: check-web
	npm --prefix web test
