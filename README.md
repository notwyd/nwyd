# NWYD

An open source game client for Linux, macOS, and Windows.

## Requirements

- Current stable Rust with Cargo, Make, and a native C/C++ toolchain.
- Nightly Rust with rustfmt for formatting: `rustup toolchain install nightly --profile minimal --component rustfmt`.
- Linux: pkg-config and development libraries for ALSA, GTK 3, and WebKitGTK 4.1.
- Linux dialogs: an XDG Desktop Portal backend with a file chooser and Zenity.
- macOS: Xcode command-line tools.
- Windows cross-builds require Linux, cargo-xwin, clang, lld, llvm-rc, the Rust llvm-tools component, and the `x86_64-pc-windows-msvc` Rust target.

## Build and run

Build a native Linux or macOS release:

```sh
make release
```

Cross-compile a Windows release from Linux:

```sh
make release-windows
```

Binaries are written to `target/<target-triple>/release/`. To build and run a debug version:

```sh
make run
```

On startup, a native folder selector asks for your game files on every platform. Choose the folder containing `ItemList.bin` and the `UI` and `Env` directories. If the files cannot be loaded, you can choose another folder; canceling exits.

Open Settings with Ctrl+Shift+S or the Settings entry in the server list. The Interface tab lists the installed languages from `Lang`; Portuguese is the default. Language changes apply immediately and are remembered for future launches.

User settings are stored separately from the game files:

- Linux: `$XDG_CONFIG_HOME/nwyd` or `~/.config/nwyd`.
- macOS: `~/Library/Application Support/nwyd`.
- Windows: `%APPDATA%\nwyd`.

Existing NWYD preferences are imported when no user settings exist. Quick-slot and guild-mark preferences use separate profiles for each game folder.

## Checks and tests

```sh
make check
make fmt
make fmt-check
cargo test --locked --workspace
```

## License

Project code is licensed under Apache-2.0. Bundled third-party assets retain their own licenses.
