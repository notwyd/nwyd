.DEFAULT_GOAL := help
SHELL := /bin/sh

CARGO ?= cargo
RUSTFMT_TOOLCHAIN ?= nightly
TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
HOST_OS := $(shell uname -s)
HOST_TARGET := $(shell rustc -vV | sed -n 's/^host: //p')
WINDOWS_TARGET := x86_64-pc-windows-msvc
GAME_DIR ?= .

.PHONY: help check test fmt fmt-check release run release-linux release-macos release-windows check-windows package-libs publish-check

help:
	@printf '%s\n' \
	  'make check: check the application and its runtime crates' \
	  'make test: run unit tests for all crates' \
	  'make release: build nwyd for the current Linux or macOS host' \
	  'make run GAME_DIR=/path/to/game: build and run with your game files' \
	  'make release-linux: native Linux release' \
	  'make release-macos: native macOS release' \
	  'make release-windows: cross-compile Windows x64 from Linux with cargo-xwin' \
	  'make check-windows: check the Windows x64 cross-build from Linux' \
	  'make fmt / fmt-check: format Rust source / check formatting with nightly rustfmt' \
	  'make package-libs: package and verify all six libraries' \
	  'make publish-check: dry-run library publication to crates.io' \
	  'Windows prerequisites: cargo-xwin, clang, lld, llvm-rc, llvm-tools and the MSVC Rust target.' \
	  'Linux prerequisites: ALSA, GTK3 and WebKitGTK 4.1 development libraries.' \
	  'macOS prerequisites: Xcode command-line tools.'

check:
	$(CARGO) check --locked --workspace

test:
	$(CARGO) test --locked --workspace

fmt:
	$(CARGO) +$(RUSTFMT_TOOLCHAIN) fmt --all

fmt-check:
	$(CARGO) +$(RUSTFMT_TOOLCHAIN) fmt --all -- --check

release:
	@test "$(HOST_OS)" = Linux -o "$(HOST_OS)" = Darwin || { echo 'Build on Linux or macOS.' >&2; exit 2; }
	$(CARGO) build --locked --release --target "$(HOST_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd

run:
	$(CARGO) build --locked --target "$(HOST_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd
	cd "$(GAME_DIR)" && "$(abspath $(TARGET_DIR))/$(HOST_TARGET)/debug/nwyd"

release-linux:
	@test "$(HOST_OS)" = Linux || { echo 'Run this target on Linux.' >&2; exit 2; }
	$(MAKE) release

release-macos:
	@test "$(HOST_OS)" = Darwin || { echo 'Run this target on macOS.' >&2; exit 2; }
	$(MAKE) release

release-windows:
	@test "$(HOST_OS)" = Linux || { echo 'Windows builds run on Linux using cargo-xwin.' >&2; exit 2; }
	$(CARGO) xwin build --locked --release --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd

check-windows:
	@test "$(HOST_OS)" = Linux || { echo 'Windows builds run on Linux using cargo-xwin.' >&2; exit 2; }
	$(CARGO) xwin check --locked --target "$(WINDOWS_TARGET)" --target-dir "$(TARGET_DIR)" -p nwyd

package-libs:
	$(CARGO) package --locked --workspace --exclude nwyd

publish-check:
	$(CARGO) publish --dry-run --locked --workspace --exclude nwyd
