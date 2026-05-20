import init, { WasmGame } from "./pkg/app.js";

async function run() {
  console.log("Loading WASM module...");
  // Initialize wasm bundle
  await init();

  console.log("WASM loaded. Booting game engine...");
  const game = await WasmGame.new("game-canvas");

  // Track key events
  window.addEventListener("keydown", (e) => {
    // Map keys to codes
    let code = e.keyCode;
    game.handle_key_down(code);

    // Prevent default scrolling for arrows and space
    if ([32, 37, 38, 39, 40].indexOf(e.keyCode) > -1) {
      e.preventDefault();
    }

    // Trigger bomb (X or K)
    if (e.key.toLowerCase() === "x" || e.key.toLowerCase() === "k") {
      game.trigger_bomb();
    }

    // Restart key (R)
    if (e.key.toLowerCase() === "r") {
      location.reload();
    }
  });

  window.addEventListener("keyup", (e) => {
    let code = e.keyCode;
    game.handle_key_up(code);
  });

  // DOM stats references
  const scoreVal = document.getElementById("score-val")!;
  const grazeVal = document.getElementById("graze-val")!;
  const bossHpBar = document.getElementById("boss-hp-bar")!;
  const bossPhaseName = document.getElementById("boss-phase-name")!;
  const livesContainer = document.getElementById("lives-container")!;
  const bombsContainer = document.getElementById("bombs-container")!;

  const debugFps = document.getElementById("debug-fps")!;
  const debugFrameTime = document.getElementById("debug-frame-time")!;
  const debugRenderTime = document.getElementById("debug-render-time")!;
  const debugComputeTime = document.getElementById("debug-compute-time")!;
  const debugBullets = document.getElementById("debug-bullets")!;
  const debugParticles = document.getElementById("debug-particles")!;

  const gameOverlay = document.getElementById("game-overlay")!;
  const overlayTitle = document.getElementById("overlay-title")!;
  const overlayMsg = document.getElementById("overlay-msg")!;
  const restartBtn = document.getElementById("restart-btn")!;

  restartBtn.addEventListener("click", () => {
    location.reload();
  });

  // Patterns reference in Japanese matching GDD
  const phasePatterns = [
    "Phase 1: 星降りの円環 (Starry Rings)",
    "Phase 2: 二重螺旋の霊札 (Double Helix)",
    "Phase 3: 月蝕の格子雨 (Lunar Lattice Rain)",
    "Phase 4: 蝶の迷路 (Maze of Butterflies)",
    "Phase 5: 時計盤レーザー (Clockwork Lasers)",
    "Phase 6: 星屑反転 (Stardust Inversion)",
    "Final Spell: 天球演算「星守ノ夜」 (Celestial Stress Test)",
  ];

  let frameCount = 0;
  let lastTime = performance.now();

  function gameLoop(timestamp: number) {
    const start = performance.now();

    // Update and render in Rust / GPU
    game.update(timestamp);
    game.render();

    const end = performance.now();
    frameCount++;

    // Throttle DOM updates to once every 3 frames for better performance
    if (frameCount % 3 === 0) {
      // 1. Update Game HUD
      const score = game.get_score().toLocaleString("en-US", { minimumIntegerDigits: 9, useGrouping: false });
      scoreVal.innerText = score;
      grazeVal.innerText = game.get_graze().toString();

      // Update Lives (hearts)
      const lives = game.get_lives();
      let heartsHtml = "";
      for (let i = 0; i < 3; i++) {
        heartsHtml += `<span class="heart ${i < lives ? "active" : ""}">★</span>`;
      }
      livesContainer.innerHTML = heartsHtml;

      // Update Bombs
      const bombs = game.get_bombs();
      let bombsHtml = "";
      for (let i = 0; i < bombs; i++) {
        bombsHtml += `<span class="bomb-badge">星</span>`;
      }
      bombsContainer.innerHTML = bombsHtml;

      // Update Boss HP
      const hpPercent = game.get_boss_hp_percent() * 100;
      bossHpBar.style.width = `${Math.max(0, hpPercent)}%`;

      const phaseIdx = Math.min(game.get_boss_phase(), 6);
      bossPhaseName.innerText = phasePatterns[phaseIdx];

      // 2. Update WebGPU Debug statistics
      const fps = game.get_fps();
      debugFps.innerText = fps.toFixed(1);
      
      const frameMs = end - start;
      debugFrameTime.innerText = `${frameMs.toFixed(1)} ms`;

      // Real high-precision hardware timings directly from WebGPU passes!
      debugRenderTime.innerText = `${game.get_gpu_render_ms().toFixed(3)} ms`;
      debugComputeTime.innerText = `${game.get_gpu_compute_ms().toFixed(3)} ms`;

      debugBullets.innerText = game.get_bullet_count().toLocaleString();
      debugParticles.innerText = (game.get_bullet_count() > 0 ? "Active" : "Idle");


      // 3. Handle Game Over / Victory
      if (game.is_game_over()) {
        overlayTitle.innerText = "演算終了 (GAME OVER)";
        overlayTitle.style.color = "var(--color-magenta)";
        overlayMsg.innerText = "篠宮 澪火の霊符結界が崩壊しました";
        gameOverlay.classList.remove("hidden");
        return; // Stop loop
      }

      if (game.is_victory()) {
        overlayTitle.innerText = "異変解決 (VICTORY)";
        overlayTitle.style.color = "var(--color-green)";
        overlayMsg.innerText = "星の演算式の暴走を鎮め、結界を修復しました！";
        gameOverlay.classList.remove("hidden");
        return; // Stop loop
      }
    }

    requestAnimationFrame(gameLoop);
  }

  requestAnimationFrame(gameLoop);
}

run().catch((err) => {
  console.error("Critical error in engine initialization: ", err);
});
