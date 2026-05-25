use wasm_bindgen::prelude::*;
use game::GameState;
use render::RenderContext;
use compute::ComputeContext;
use shared::{BulletInit, Particle, DebugCounters, MAX_BULLETS, MAX_PARTICLES};

const BULLET_META_SIZE: usize = 16; // radius(f32) + age(f32) + lifetime(f32) + packed_flags(u32)

struct PlayerBullet {
    position: [f32; 2],
    active: bool,
}

#[wasm_bindgen]
pub struct WasmGame {
    state: GameState,
    render: RenderContext,
    compute: ComputeContext,
    keys: [bool; 256],
    shift_pressed: bool,
    last_frame_time: f64,
    fps: f32,
    frame_count: u32,
    fps_timer: f32,

    // CPU-side state tracking
    ticks: u32,
    bullet_write_idx: usize,
    particle_write_idx: usize,
    player_bullets: Vec<PlayerBullet>,
    
    gpu_compute_ms: f32,
    gpu_render_ms: f32,
    timing_is_approximate: u32,
    debug_counters: DebugCounters,
    active_particles: u32,
    particles_spawned_this_frame: u32,
    
    pos_staging: Vec<u8>,
    vel_staging: Vec<u8>,
    accel_staging: Vec<u8>,
    meta_staging: Vec<u8>,
    typeinfo_staging: Vec<u8>,
    seed_staging: Vec<u8>,
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen]
    pub async fn new(canvas_id: &str) -> Result<WasmGame, JsValue> {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Debug);

        log::info!("Initializing GPU Bullet Hell Shrine Wasm Engine...");

        let window = web_sys::window().ok_or("No global window found")?;
        let document = window.document().ok_or("No document found")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("Canvas element '{}' not found", canvas_id))?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| "Element is not a canvas")?;

        // Initialize Render & Compute contexts (sharing zero-copy WebGPU buffers)
        let render = RenderContext::new(canvas).await;
        let compute = ComputeContext::new(
            render.device.clone(),
            render.queue.clone(),
            &render.bullet_pos_buf,
            &render.bullet_vel_buf,
            &render.bullet_accel_buf,
            &render.bullet_meta_buf,
            &render.bullet_typeinfo_buf,
            &render.bullet_seed_buf,
            &render.particle_buf,
            &render.uniform_buffer,
        );

        Ok(WasmGame {
            state: {
                let mut s = GameState::new();
                s.bullet_count = MAX_BULLETS as u32;
                s
            },
            render,
            compute,
            keys: [false; 256],
            shift_pressed: false,
            last_frame_time: 0.0,
            fps: 60.0,
            frame_count: 0,
            fps_timer: 0.0,
            ticks: 0,
            bullet_write_idx: 0,
            particle_write_idx: 0,
            player_bullets: Vec::with_capacity(128),
            gpu_compute_ms: 0.1,
            gpu_render_ms: 0.5,
            timing_is_approximate: 1,
            debug_counters: DebugCounters {
                fps: 60.0, frame_ms: 0.0, compute_ms: 0.1, render_ms: 0.5,
                active_bullets: 0, active_particles: 0, draw_calls: 0,
                buffer_upload_bytes: 0, grid_max_bucket: 0, grid_avg_bucket: 0.0,
                collision_hits: 0, collision_grazes: 0,
                timing_is_approximate: 1, _pad_counters: [0; 3],
            },
            active_particles: 0,
            particles_spawned_this_frame: 0,
            pos_staging: Vec::new(),
            vel_staging: Vec::new(),
            accel_staging: Vec::new(),
            meta_staging: Vec::new(),
            typeinfo_staging: Vec::new(),
            seed_staging: Vec::new(),
        })
    }

    #[wasm_bindgen]
    pub fn handle_key_down(&mut self, key_code: u32) {
        if key_code < 256 {
            self.keys[key_code as usize] = true;
        }
        if key_code == 16 { // Shift key
            self.shift_pressed = true;
        }
    }

    #[wasm_bindgen]
    pub fn handle_key_up(&mut self, key_code: u32) {
        if key_code < 256 {
            self.keys[key_code as usize] = false;
        }
        if key_code == 16 { // Shift key
            self.shift_pressed = false;
        }
    }

    #[wasm_bindgen]
    pub fn trigger_bomb(&mut self) {
        if self.state.trigger_bomb() {
            log::info!("Bomb Triggered: 星封結界 (Spell Seal Barrier)!");
            
            let zero_meta = vec![0u8; MAX_BULLETS * BULLET_META_SIZE];
            self.render.queue.write_buffer(&self.render.bullet_meta_buf, 0, &zero_meta);
            self.bullet_write_idx = 0;
            
            // Spawn gorgeous circular violet bomb shockwave particles
            let player_pos = self.state.player.position;
            for k in 0..120 {
                let angle = (k as f32) * (2.0 * std::f32::consts::PI / 120.0);
                let speed = 450.0 + ((k % 4) as f32 * 50.0);
                self.spawn_particle(Particle {
                    position: player_pos,
                    velocity: [angle.cos() * speed, angle.sin() * speed],
                    color: [0.6, 0.1, 1.0, 1.0], // Violet glows
                    size: 14.0,
                    age: 0.0,
                    lifetime: 1.6,
                    flags: 1,
                });
            }
        }
    }

    #[wasm_bindgen]
    pub fn update(&mut self, timestamp: f64) {
        if self.last_frame_time == 0.0 {
            self.last_frame_time = timestamp;
            return;
        }

        let dt = ((timestamp - self.last_frame_time) / 1000.0) as f32;
        self.last_frame_time = timestamp;

        // Cap dt to prevent massive jumps when tab is inactive
        let dt = dt.min(0.05);

        self.ticks += 1;
        self.particles_spawned_this_frame = 0;

        // Track FPS
        self.frame_count += 1;
        self.fps_timer += dt;
        if self.fps_timer >= 1.0 {
            self.fps = self.frame_count as f32 / self.fps_timer;
            self.frame_count = 0;
            self.fps_timer = 0.0;
        }

        // 1. Run core CPU-side State Update
        self.state.update(dt, &self.keys, self.shift_pressed);

        if self.state.is_game_over || self.state.is_victory {
            return;
        }

        // 2. Aim & Fire player shots (when holding Space or Z key)
        // Key 90 = Z, Key 32 = Space
        if (self.keys[90] || self.keys[32]) && self.ticks % 4 == 0 {
            let player_pos = self.state.player.position;
            self.player_bullets.push(PlayerBullet {
                position: [player_pos[0] - 12.0, player_pos[1] - 15.0],
                active: true,
            });
            self.player_bullets.push(PlayerBullet {
                position: [player_pos[0] + 12.0, player_pos[1] - 15.0],
                active: true,
            });

            // Sparking cyan particles from shot emitters
            self.spawn_particle(Particle {
                position: [player_pos[0], player_pos[1] - 10.0],
                velocity: [0.0, -180.0],
                color: [0.0, 0.9, 1.0, 1.0],
                size: 4.5,
                age: 0.0,
                lifetime: 0.3,
                flags: 1,
            });
        }

        // 3. Update Player CPU-side Bullets Flight & Collision with Boss
        let boss_pos = self.state.boss.position;
        let mut hit_boss = false;
        for b in &mut self.player_bullets {
            if !b.active { continue; }
            b.position[1] -= 1200.0 * dt; // Fast upward shots
            
            if b.position[1] < 30.0 {
                b.active = false;
                continue;
            }

            // Check hitbox collision against Boss (32px hitbox)
            let dx = b.position[0] - boss_pos[0];
            let dy = b.position[1] - boss_pos[1];
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < 36.0 * 36.0 {
                b.active = false;
                self.state.boss.hp = (self.state.boss.hp - 1.2).max(0.0);
                self.state.player.score += 80;
                hit_boss = true;
            }
        }
        self.player_bullets.retain(|b| b.active);

        // Spawn gold hit effects on boss
        if hit_boss {
            for k in 0..3 {
                let speed = 90.0 + (k as f32 * 40.0);
                let angle = (k as f32) * (2.0 * std::f32::consts::PI / 3.0) + (self.ticks as f32 * 0.1);
                self.spawn_particle(Particle {
                    position: boss_pos,
                    velocity: [angle.cos() * speed, angle.sin() * speed],
                    color: [1.0, 0.8, 0.0, 1.0], // Glowing gold sparks
                    size: 6.0,
                    age: 0.0,
                    lifetime: 0.4,
                    flags: 1,
                });
            }
        }

        // 4. Procedural Spellcard Spawns from Boss (Emitting Bullets) — batched
        let new_bullets = self.state.emit_pattern(self.ticks);
        self.flush_bullets(&new_bullets);

        // 5. Run WebGPU Compute Passes (Updating physics, spatial hash, and collisions)
        self.compute.execute_compute_pass(self.state.bullet_count);
        self.gpu_compute_ms = self.compute.last_frame_compute_ms;
        
        // 6. Handle Collision Result Readback non-blocking
        self.compute.sample_collisions(web_sys::window().unwrap().performance().unwrap().now());
        while let Some(col_result) = self.compute.take_collision_result() {
            let hits = col_result.hit_count;
            let grazes = col_result.graze_count;
            
            self.state.handle_collision_results(&col_result);

            if hits > 0 && !self.state.player.is_invincible {
                let player_pos = self.state.player.position;
                for k in 0..80 {
                    let angle = (k as f32) * (2.0 * std::f32::consts::PI / 80.0);
                    let speed = 120.0 + (k as f32 % 4.0) * 80.0;
                    self.spawn_particle(Particle {
                        position: player_pos,
                        velocity: [angle.cos() * speed, angle.sin() * speed],
                        color: [1.0, 0.1, 0.4, 1.0],
                        size: 7.0,
                        age: 0.0,
                        lifetime: 1.1,
                        flags: 1,
                    });
                }
            }

            if grazes > 0 {
                let player_pos = self.state.player.position;
                for k in 0..grazes.min(8) {
                    let angle = (k as f32) * (2.0 * std::f32::consts::PI / 8.0);
                    self.spawn_particle(Particle {
                        position: player_pos,
                        velocity: [angle.cos() * 70.0, angle.sin() * 70.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                        size: 3.5,
                        age: 0.0,
                        lifetime: 0.25,
                        flags: 1,
                    });
                }
            }

            self.compute.last_frame_collision_hits = hits;
            self.compute.last_frame_collision_grazes = grazes;
        }

        self.active_particles = self.particles_spawned_this_frame;
        self.compute.sample_grid_stats();
        self.compute.sample_active_count();

        self.debug_counters = DebugCounters {
            fps: self.fps,
            frame_ms: self.gpu_compute_ms + self.gpu_render_ms,
            compute_ms: self.gpu_compute_ms,
            render_ms: self.gpu_render_ms,
            active_bullets: self.compute.last_frame_active_bullets,
            active_particles: self.active_particles,
            draw_calls: self.render.last_frame_draw_calls,
            buffer_upload_bytes: self.render.last_frame_upload_bytes,
            grid_max_bucket: self.compute.last_frame_grid_max_bucket,
            grid_avg_bucket: self.compute.last_frame_grid_avg_bucket,
            collision_hits: self.compute.last_frame_collision_hits,
            collision_grazes: self.compute.last_frame_collision_grazes,
            timing_is_approximate: self.timing_is_approximate,
            _pad_counters: [0; 3],
        };
    }

    #[wasm_bindgen]
    pub fn render(&mut self) {
        let render_start = web_sys::window().unwrap().performance().unwrap().now();
        
        // Construct uniform values
        let uniforms = self.state.fill_uniforms(
            self.render.size.0 as f32,
            self.render.size.1 as f32,
        );

        // Run additive render pipelines
        self.render.render_frame(&uniforms);

        let render_end = web_sys::window().unwrap().performance().unwrap().now();
        self.gpu_render_ms = (render_end - render_start) as f32;
    }

    // Dynamic bullet spawning helper (single bullet, legacy path for backward compat)
    #[allow(dead_code)]
    fn spawn_bullet(&mut self, init: BulletInit) {
        let idx = self.bullet_write_idx;
        
        let mut meta_bytes = [0u8; 16];
        meta_bytes[0..4].copy_from_slice(&init.radius.to_ne_bytes());
        meta_bytes[4..8].copy_from_slice(&0.0f32.to_ne_bytes()); // age = 0.0
        meta_bytes[8..12].copy_from_slice(&init.lifetime.to_ne_bytes());
        meta_bytes[12..16].copy_from_slice(&(init.flags | 1u32).to_ne_bytes());

        self.render.write_bullet_buffer(render::BulletBufferType::Pos, idx, bytemuck::bytes_of(&init.position), 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Vel, idx, bytemuck::bytes_of(&init.velocity), 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Accel, idx, bytemuck::bytes_of(&init.acceleration), 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Meta, idx, &meta_bytes, 16);
        
        let typeinfo = init.bullet_type | (init.pattern_id << 8) | (init.color_id << 16) | (init.flags << 24);
        self.render.write_bullet_buffer(render::BulletBufferType::TypeInfo, idx, bytemuck::bytes_of(&typeinfo), 4);
        self.render.write_bullet_buffer(render::BulletBufferType::Seed, idx, bytemuck::bytes_of(&init.seed), 4);

        self.bullet_write_idx = (self.bullet_write_idx + 1) % MAX_BULLETS;
    }

    fn flush_bullets(&mut self, inits: &[BulletInit]) {
        let n = inits.len();
        if n == 0 { return; }

        let idx = self.bullet_write_idx;

        self.pos_staging.clear();
        self.vel_staging.clear();
        self.accel_staging.clear();
        self.meta_staging.clear();
        self.typeinfo_staging.clear();
        self.seed_staging.clear();

        for init in inits {
            self.pos_staging.extend_from_slice(bytemuck::bytes_of(&init.position));
            self.vel_staging.extend_from_slice(bytemuck::bytes_of(&init.velocity));
            self.accel_staging.extend_from_slice(bytemuck::bytes_of(&init.acceleration));
            let mut meta = [0u8; 16];
            meta[0..4].copy_from_slice(&init.radius.to_ne_bytes());
            meta[8..12].copy_from_slice(&init.lifetime.to_ne_bytes());
            meta[12..16].copy_from_slice(&(init.flags | 1u32).to_ne_bytes());
            self.meta_staging.extend_from_slice(&meta);
            let ti = init.bullet_type | (init.pattern_id << 8) | (init.color_id << 16) | (init.flags << 24);
            self.typeinfo_staging.extend_from_slice(bytemuck::bytes_of(&ti));
            self.seed_staging.extend_from_slice(bytemuck::bytes_of(&init.seed));
        }

        self.render.write_bullet_buffer(render::BulletBufferType::Pos, idx, &self.pos_staging, 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Vel, idx, &self.vel_staging, 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Accel, idx, &self.accel_staging, 8);
        self.render.write_bullet_buffer(render::BulletBufferType::Meta, idx, &self.meta_staging, 16);
        self.render.write_bullet_buffer(render::BulletBufferType::TypeInfo, idx, &self.typeinfo_staging, 4);
        self.render.write_bullet_buffer(render::BulletBufferType::Seed, idx, &self.seed_staging, 4);

        self.bullet_write_idx = (idx + n) % MAX_BULLETS;
    }

    // Dynamic particle spawning helper
    fn spawn_particle(&mut self, part: Particle) {
        let idx = self.particle_write_idx;
        self.render.write_particle_buffer(idx, bytemuck::bytes_of(&part));
        self.particle_write_idx = (self.particle_write_idx + 1) % MAX_PARTICLES;
        self.particles_spawned_this_frame += 1;
    }

    // Expose DOM statistical bindings
    #[wasm_bindgen]
    pub fn get_score(&self) -> u32 { self.state.player.score }

    #[wasm_bindgen]
    pub fn get_lives(&self) -> u32 { self.state.player.lives }

    #[wasm_bindgen]
    pub fn get_bombs(&self) -> u32 { self.state.player.bombs }

    #[wasm_bindgen]
    pub fn get_graze(&self) -> u32 { self.state.player.graze }

    #[wasm_bindgen]
    pub fn get_bullet_count(&self) -> u32 { self.state.bullet_count }

    #[wasm_bindgen]
    pub fn get_fps(&self) -> f32 { self.fps }

    #[wasm_bindgen]
    pub fn is_game_over(&self) -> bool { self.state.is_game_over }

    #[wasm_bindgen]
    pub fn is_victory(&self) -> bool { self.state.is_victory }

    #[wasm_bindgen]
    pub fn get_boss_hp_percent(&self) -> f32 {
        self.state.boss.hp / self.state.boss.max_hp
    }

    #[wasm_bindgen]
    pub fn get_boss_phase(&self) -> u32 {
        self.state.boss.current_phase
    }

    // True GPU Compute and Render Frame timings!
    #[wasm_bindgen]
    pub fn get_gpu_compute_ms(&self) -> f32 { self.gpu_compute_ms }

    #[wasm_bindgen]
    pub fn get_gpu_render_ms(&self) -> f32 { self.gpu_render_ms }

    #[wasm_bindgen]
    pub fn get_phase_display_name(&self) -> String {
        self.state.get_phase_display_name()
    }

    #[wasm_bindgen]
    pub fn get_final_spell_timer(&self) -> f32 {
        self.state.get_final_spell_timer()
    }

    #[wasm_bindgen]
    pub fn is_final_spell_active(&self) -> bool {
        self.state.is_final_spell_active()
    }

    #[wasm_bindgen]
    pub fn get_debug_counters_js(&self) -> JsValue {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("fps"), &JsValue::from_f64(self.debug_counters.fps as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("frame_ms"), &JsValue::from_f64(self.debug_counters.frame_ms as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("compute_ms"), &JsValue::from_f64(self.debug_counters.compute_ms as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("render_ms"), &JsValue::from_f64(self.debug_counters.render_ms as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("active_bullets"), &JsValue::from_f64(self.debug_counters.active_bullets as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("active_particles"), &JsValue::from_f64(self.debug_counters.active_particles as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("draw_calls"), &JsValue::from_f64(self.debug_counters.draw_calls as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("buffer_upload_bytes"), &JsValue::from_f64(self.debug_counters.buffer_upload_bytes as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("grid_max_bucket"), &JsValue::from_f64(self.debug_counters.grid_max_bucket as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("grid_avg_bucket"), &JsValue::from_f64(self.debug_counters.grid_avg_bucket as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("collision_hits"), &JsValue::from_f64(self.debug_counters.collision_hits as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("collision_grazes"), &JsValue::from_f64(self.debug_counters.collision_grazes as f64));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("timing_is_approximate"), &JsValue::from_f64(self.debug_counters.timing_is_approximate as f64));
        JsValue::from(obj)
    }

    /// Returns true if the device + browser support wgpu::Features::TIMESTAMP_QUERY.
    /// Currently informational only — Phase 7-b will wire real GPU timestamp readback.
    /// Until then, `timing_is_approximate` remains 1 regardless of this capability.
    #[wasm_bindgen]
    pub fn is_timestamp_query_capable(&self) -> bool {
        self.render.has_timestamp_query && self.compute.has_timestamp_query
    }
}
