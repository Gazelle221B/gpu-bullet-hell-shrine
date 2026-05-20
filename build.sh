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

echo "Patching generated app.js to remove deprecated maxInterStageShaderComponents limit..."
node -e '
  const fs = require("fs");
  const filePath = "web/pkg/app.js";
  if (!fs.existsSync(filePath)) {
    console.error("app.js does not exist!");
    process.exit(1);
  }
  let content = fs.readFileSync(filePath, "utf8");
  
  // Replace requestDevice callsite to delete maxInterStageShaderComponents limit
  let patched = false;
  content = content.replace(/const\s+ret\s*=\s*([\w]+)\.requestDevice\(([\w]+)\);/g, (match, adapter, desc) => {
    patched = true;
    return `if (${desc} && ${desc}.requiredLimits) { delete ${desc}.requiredLimits.maxInterStageShaderComponents; }\n            ${match}`;
  });
  
  // Build-time validation check to ensure the offending limit field deletion was actually injected
  if (patched && content.includes("maxInterStageShaderComponents")) {
    fs.writeFileSync(filePath, content, "utf8");
    console.log("Successfully patched app.js dynamically!");
  } else {
    console.error("Failed to patch app.js! requestDevice callsite not found or validation failed.");
    process.exit(1);
  }
'

echo "=== Build Succeeded! Wasm outputs are ready in web/pkg ==="
