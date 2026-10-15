# NWYD

## Prerequisites

- Stable Rust and Cargo
- Make

## Linux (Debian/Ubuntu)

```sh
sudo apt-get install build-essential pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev
make release
```

## macOS

```sh
xcode-select --install
make release
```

## Windows (cross-build from Linux)

```sh
sudo apt-get install build-essential pkg-config clang lld llvm cmake ninja-build
rustup target add x86_64-pc-windows-msvc
rustup component add llvm-tools
cargo install --locked cargo-xwin --version 0.23.1
make release-windows
```

## WebAssembly

- Node.js 22.12+ and npm

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.126
make release-web
```

## Relay

```sh
make release-relay
```

### Windows (cross-build from Linux)

```sh
make release-relay-windows
```
