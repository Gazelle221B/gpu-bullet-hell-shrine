import init, { WasmGame } from "./pkg/app.js";

function codeToLegacyKey(code: string): number {
  const map: Record<string, number> = {
    ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40,
    KeyW: 87, KeyA: 65, KeyS: 83, KeyD: 68,
    KeyZ: 90, KeyX: 88, KeyK: 75, KeyR: 82, KeyP: 80,
    ShiftLeft: 16, ShiftRight: 16,
    Space: 32,
  };
  return map[code] ?? 0;
}

const SCROLL_PREVENT_CODES = new Set([
  "ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown", "Space",
]);

async function run() {
  console.log("Loading WASM module...");
  // Initialize wasm bundle
  await init();

  console.log("WASM loaded. Booting game engine...");
  const game = await WasmGame.new("game-canvas");

  // Track key events
  window.addEventListener("keydown", (e) => {
    const code = codeToLegacyKey(e.code);
    game.handle_key_down(code);

    // Prevent default scrolling for arrows and space
    if (SCROLL_PREVENT_CODES.has(e.code)) {
      e.preventDefault();
    }

    // Trigger bomb (X or K)
    if (e.code === "KeyX" || e.code === "KeyK") {
      game.trigger_bomb();
    }

    // Restart key (R)
    if (e.code === "KeyR") {
      location.reload();
    }
  });

  window.addEventListener("keyup", (e) => {
    const code = codeToLegacyKey(e.code);
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
  const debugDrawCalls = document.getElementById("debug-draw-calls")!;
  const debugMaxBucket = document.getElementById("debug-max-bucket")!;
  const debugAvgBucket = document.getElementById("debug-avg-bucket")!;
  const debugUpload = document.getElementById("debug-upload")!;
  const debugMode = document.getElementById("debug-mode")!;

  const finalSpellCountdown = document.getElementById("final-spell-countdown") as HTMLElement | null;
  const finalSpellSec = document.getElementById("final-spell-sec") as HTMLElement | null;

  const gameOverlay = document.getElementById("game-overlay")!;
  const overlayTitle = document.getElementById("overlay-title")!;
  const overlayMsg = document.getElementById("overlay-msg")!;
  const restartBtn = document.getElementById("restart-btn")!;

  restartBtn.addEventListener("click", () => {
    location.reload();
  });

  let frameCount = 0;

  function gameLoop(timestamp: number) {
    game.update(timestamp);
    game.render();

    frameCount++;

    // Throttle DOM updates to once every 3 frames for better performance
    if (frameCount % 3 === 0) {
      // 1. Update Game HUD
      const score = game.get_score().toLocaleString("en-US", { minimumIntegerDigits: 9, useGrouping: false });
      scoreVal.innerText = score;
      grazeVal.innerText = game.get_graze().toString();

      const lives = game.get_lives();
      let heartsHtml = "";
      for (let i = 0; i < 3; i++) {
        heartsHtml += `<span class="heart ${i < lives ? "active" : ""}">★</span>`;
      }
      livesContainer.innerHTML = heartsHtml;

      const bombs = game.get_bombs();
      let bombsHtml = "";
      for (let i = 0; i < bombs; i++) {
        bombsHtml += `<span class="bomb-badge">星</span>`;
      }
      bombsContainer.innerHTML = bombsHtml;

      const hpPercent = game.get_boss_hp_percent() * 100;
      bossHpBar.style.width = `${Math.max(0, hpPercent)}%`;

      bossPhaseName.innerText = game.get_phase_display_name();

      if (finalSpellCountdown && finalSpellSec) {
        if (game.is_final_spell_active()) {
          finalSpellCountdown.style.display = "";
          finalSpellSec.innerText = game.get_final_spell_timer().toFixed(1);
        } else {
          finalSpellCountdown.style.display = "none";
        }
      }

      // 2. Update WebGPU Debug statistics
      const counters = game.get_debug_counters_js() as any;
      const prefix = counters.timing_is_approximate ? "~" : "";

      debugFps.innerText = counters.fps.toFixed(1);
      debugFrameTime.innerText = `${counters.frame_ms.toFixed(1)} ms`;
      debugRenderTime.innerText = `${prefix}${counters.render_ms.toFixed(3)} ms`;
      debugComputeTime.innerText = `${prefix}${counters.compute_ms.toFixed(3)} ms`;
      debugBullets.innerText = counters.active_bullets.toLocaleString();
      debugParticles.innerText = counters.active_particles.toLocaleString();
      debugDrawCalls.innerText = counters.draw_calls.toString();
      debugMaxBucket.innerText = counters.grid_max_bucket.toString();
      debugAvgBucket.innerText = counters.grid_avg_bucket.toFixed(1);
      debugUpload.innerText = `${(counters.buffer_upload_bytes / 1024).toFixed(0)} KB/frame`;
      debugMode.innerText = counters.timing_is_approximate ? "ArrayBuffer (~)" : "ArrayBuffer";


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
