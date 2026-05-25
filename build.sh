#!/bin/bash
set -e

echo "=== GPU Bullet Hell Shrine Sandboxed Wasm Build Script ==="

# Get absolute path of .bin
BIN_DIR="$(pwd)/.bin"

# Prepend our wrapper bin directory to PATH
export PATH="$BIN_DIR:$PATH"
export CARGO_HOME="$(pwd)/.cargo_home"
export RUSTUP_HOME="/Users/kairyon/.rustup"

echo "Using Cargo location: $(which cargo)"
echo "Using Rustc location: $(which rustc)"
echo "Using Wasm-Pack location: $(which wasm-pack)"

# Check versions
cargo --version
rustc --version
wasm-pack --version

echo "Building wasm package offline..."
wasm-pack build crates/app --target web --out-dir ../../web/pkg --offline



echo "=== Build Succeeded! Wasm outputs are ready in web/pkg ==="
