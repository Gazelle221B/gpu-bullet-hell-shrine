# 霊符演算録 〜 GPU Bullet Hell Shrine

A high-performance WebGPU-powered bullet hell (Danmaku) shoot 'em up game built with Rust, WebAssembly, and Vite.

This project leverages modern WebGPU compute shaders to simulate and render thousands of bullets simultaneously at 120 FPS, complete with spatial hashing for hyper-fast collision detection and gorgeous particle effects.

## ✨ Features
- **GPU-Driven Compute**: All bullet trajectories, spellcard logic, and spatial hashing are evaluated in parallel on the GPU using WGSL compute shaders.
- **Zero-Copy Rendering**: Computed state is passed directly to the render pipeline without CPU bottlenecks via WebGPU storage buffers.
- **6 Spellcard Patterns + 1 Final Spell**: Engaging Touhou-style boss attack patterns.
- **Responsive Web UI**: A glassmorphic heads-up display built with modern HTML/CSS that tracks true hardware telemetry.

## 🚀 Running Locally

### Prerequisites
- Node.js (for running the Vite dev server)
- Rust (for compiling WebAssembly via `wasm-pack`)
- A WebGPU-compatible browser (e.g., Google Chrome 113+, Microsoft Edge 113+)

### Build Instructions

1. **Install Frontend Dependencies:**
   ```bash
   npm install
   ```

2. **Compile Rust to WebAssembly:**
   Use the local build script to safely execute `wasm-pack` and build the WASM bundle:
   ```bash
   ./build.sh
   ```

3. **Launch the Dev Server:**
   ```bash
   npm run dev
   ```

4. **Play!**
   Open the local server URL (typically `http://localhost:5173`) in your browser.

## 🎮 Controls
- **WASD / Arrow Keys**: Move the player.
- **Shift**: Focus mode (slows down movement and reveals the player's hitbox).
- **Z / Space**: Fire needle shots.
- **X / K**: Unleash the **星封結界 (Spell Seal Barrier) Bomb** to clear all active bullets (costs 1 Bomb).

## 🛠️ Architecture
- **crates/app**: Rust to WASM bridge, exposing CPU game state, orchestration, and performance counters to JS.
- **crates/compute**: WGSL compute shaders managing Euler integration, bounding checks, and spatial grid hashing.
- **crates/render**: WebGPU drawing pipelines handling instanced geometries and additive blending for glowing shapes.
- **crates/game**: CPU-side bounds checking, timer tracking, and state management.
- **crates/shared**: Common standard data structures enforcing `std140` and `std430` layout compatibility between Rust and WGSL.
- **web**: Frontend TypeScript logic and DOM overlays binding to the WebAssembly engine.

## 📜 Attribution
Inspired by classic Japanese danmaku shooters. Not affiliated with Team Shanghai Alice or the Touhou Project.

## 📜 License
MIT License
