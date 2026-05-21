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

echo "Patching generated app.js to handle deprecated maxInterStageShaderComponents limit..."
node -e '
  const fs = require("fs");
  const filePath = "web/pkg/app.js";
  if (!fs.existsSync(filePath)) {
    console.error("app.js does not exist!");
    process.exit(1);
  }
  let content = fs.readFileSync(filePath, "utf8");
  
  const injectionMarker = "__patchedRequestDevice";
  if (content.includes(injectionMarker)) {
    console.log("app.js is already patched, skipping.");
    process.exit(0);
  }
  
  // Stable post-processing step: monkey-patch GPUAdapter to remove deprecated limit
  const stablePatch = `
if (typeof GPUAdapter !== "undefined" && GPUAdapter.prototype.requestDevice && !GPUAdapter.prototype.__patchedRequestDevice) {
    GPUAdapter.prototype.__patchedRequestDevice = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = function(descriptor) {
        if (descriptor && descriptor.requiredLimits) {
            delete descriptor.requiredLimits.maxInterStageShaderComponents;
        }
        return this.__patchedRequestDevice(descriptor);
    };
}
`;

  content = content + "\n" + stablePatch;
  
  fs.writeFileSync(filePath, content, "utf8");
  console.log("Successfully patched app.js with a stable GPUAdapter wrapper!");
'

echo "=== Build Succeeded! Wasm outputs are ready in web/pkg ==="
