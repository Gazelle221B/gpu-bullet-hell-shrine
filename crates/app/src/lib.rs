use wasm_bindgen::prelude::*;
use std::sync::Arc;
use game::GameState;
use render::RenderContext;
use compute::ComputeContext;
use shared::{BulletInit, Particle, MAX_BULLETS, MAX_PARTICLES};

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
    
    // Performance counters (for true debug HUD overlay!)
    gpu_compute_ms: f32,
    gpu_render_ms: f32,
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
            state: GameState::new(),
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
            
            // Instantly clear all active bullets
            self.bullet_write_idx = 0;
            self.state.bullet_count = 0;
            
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

        // 4. Procedural Spellcard Spawns from Boss (Emitting Bullets)
        let pat_id = self.state.active_pattern;
        
        if pat_id == 1 {
            // Pattern 1: Star circular ring (Dream Seal)
            if self.ticks % 20 == 0 {
                let count = 36;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 190.0, angle.sin() * 190.0],
                        acceleration: [0.0, 0.0],
                        radius: 6.0,
                        lifetime: 7.0,
                        pattern_id: 1,
                        bullet_type: 2, // Orb
                        color_id: (self.ticks / 20) % 6,
                        seed: i,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 2 {
            // Pattern 2: Helix double spiral
            if self.ticks % 3 == 0 {
                let base_angle = (self.ticks as f32) * 0.12;
                for i in 0..4 {
                    let angle = base_angle + (i as f32) * (std::f32::consts::PI / 2.0);
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 210.0, angle.sin() * 210.0],
                        acceleration: [0.0, 0.0],
                        radius: 5.0,
                        lifetime: 6.5,
                        pattern_id: 2,
                        bullet_type: 1, // Star
                        color_id: 3, // Purple
                        seed: i * self.ticks,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 3 {
            // Pattern 3: Lunar Lattice Rain (Gravity deflected Talismans)
            if self.ticks % 8 == 0 {
                for i in 0..5 {
                    let rx = 340.0 + ((self.ticks * 71 + i * 29) % 600) as f32;
                    self.spawn_bullet(BulletInit {
                        position: [rx, 80.0],
                        velocity: [0.0, 110.0],
                        acceleration: [0.0, 50.0], // Accelerates down
                        radius: 8.0,
                        lifetime: 8.0,
                        pattern_id: 3,
                        bullet_type: 3, // Talisman
                        color_id: 4, // Green
                        seed: i * 13,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 4 {
            // Pattern 4: Spreading Butterflies
            if self.ticks % 7 == 0 {
                let base_angle = (self.ticks as f32) * 0.09;
                for i in 0..6 {
                    let angle = base_angle + (i as f32) * (2.0 * std::f32::consts::PI / 6.0);
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 160.0, angle.sin() * 160.0],
                        acceleration: [0.0, 0.0],
                        radius: 7.0,
                        lifetime: 7.0,
                        pattern_id: 4,
                        bullet_type: 2, // Orb
                        color_id: 1, // Magenta
                        seed: i * 23,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 5 {
            // Pattern 5: Needles targeting player position
            if self.ticks % 14 == 0 {
                let p_pos = self.state.player.position;
                let target_angle = (p_pos[1] - boss_pos[1]).atan2(p_pos[0] - boss_pos[0]);
                for i in 0..3 {
                    let angle_offset = (i as f32 - 1.0) * 0.1;
                    let angle = target_angle + angle_offset;
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 320.0, angle.sin() * 320.0],
                        acceleration: [angle.cos() * 90.0, angle.sin() * 90.0],
                        radius: 4.0,
                        lifetime: 4.5,
                        pattern_id: 5,
                        bullet_type: 4, // Needle
                        color_id: 5, // Orange
                        seed: i + self.ticks,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 6 {
            // Pattern 6: Stardust Inversion (Stops and tracks player)
            if self.ticks % 25 == 0 {
                let count = 48;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 300.0, angle.sin() * 300.0],
                        acceleration: [-angle.cos() * 150.0, -angle.sin() * 150.0], // Slows down to stop
                        radius: 5.5,
                        lifetime: 7.5,
                        pattern_id: 6,
                        bullet_type: 1, // Star
                        color_id: 2, // Gold
                        seed: i * 47,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 7 {
            // Pattern 7: Celestial Stress Test (Double all frequencies!)
            if self.ticks % 10 == 0 {
                let count = 30;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    self.spawn_bullet(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 180.0, angle.sin() * 180.0],
                        acceleration: [0.0, 0.0],
                        radius: 6.0,
                        lifetime: 7.0,
                        pattern_id: 7,
                        bullet_type: 2,
                        color_id: i % 6,
                        seed: i,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
            if self.ticks % 2 == 0 {
                let angle = (self.ticks as f32) * 0.15;
                self.spawn_bullet(BulletInit {
                    position: boss_pos,
                    velocity: [angle.cos() * 240.0, angle.sin() * 240.0],
                    acceleration: [0.0, 0.0],
                    radius: 5.0,
                    lifetime: 6.0,
                    pattern_id: 7,
                    bullet_type: 1,
                    color_id: 3,
                    seed: self.ticks,
                    flags: 1,
                    _padding: [0; 3],
                });
            }
        }

        // 5. Run WebGPU Compute Passes (Updating physics, spatial hash, and collisions)
        let compute_start = web_sys::window().unwrap().performance().unwrap().now();
        self.compute.execute_compute_pass(self.state.bullet_count);
        
        // 6. Handle Collision Result Readback synchronously
        if let Some(col_result) = self.compute.read_collisions_sync() {
            let hits = col_result.hit_count;
            let grazes = col_result.graze_count;
            
            self.state.handle_collision_results(&col_result);

            // Handle death explosion on hits
            if hits > 0 && !self.state.player.is_invincible {
                let player_pos = self.state.player.position;
                for k in 0..80 {
                    let angle = (k as f32) * (2.0 * std::f32::consts::PI / 80.0);
                    let speed = 120.0 + (k as f32 % 4.0) * 80.0;
                    self.spawn_particle(Particle {
                        position: player_pos,
                        velocity: [angle.cos() * speed, angle.sin() * speed],
                        color: [1.0, 0.1, 0.4, 1.0], // Cherry blossom pink death ring!
                        size: 7.0,
                        age: 0.0,
                        lifetime: 1.1,
                        flags: 1,
                    });
                }
            }

            // Grazing white sparkle bursts
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
        }
        let compute_end = web_sys::window().unwrap().performance().unwrap().now();
        self.gpu_compute_ms = (compute_end - compute_start) as f32;
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

    // Dynamic bullet spawning helper
    fn spawn_bullet(&mut self, init: BulletInit) {
        let idx = self.bullet_write_idx;
        
        let mut meta_bytes = [0u8; 16];
        meta_bytes[0..4].copy_from_slice(&init.radius.to_ne_bytes());
        meta_bytes[4..8].copy_from_slice(&0.0f32.to_ne_bytes()); // age = 0.0
        meta_bytes[8..12].copy_from_slice(&init.lifetime.to_ne_bytes());
        meta_bytes[12..16].copy_from_slice(&(init.flags | 1u32).to_ne_bytes()); // set active bit

        self.render.queue.write_buffer(&self.render.bullet_pos_buf, (idx * 8) as u64, bytemuck::bytes_of(&init.position));
        self.render.queue.write_buffer(&self.render.bullet_vel_buf, (idx * 8) as u64, bytemuck::bytes_of(&init.velocity));
        self.render.queue.write_buffer(&self.render.bullet_accel_buf, (idx * 8) as u64, bytemuck::bytes_of(&init.acceleration));
        self.render.queue.write_buffer(&self.render.bullet_meta_buf, (idx * 16) as u64, &meta_bytes);
        
        let typeinfo = init.bullet_type | (init.pattern_id << 8) | (init.color_id << 16) | (init.flags << 24);
        self.render.queue.write_buffer(&self.render.bullet_typeinfo_buf, (idx * 4) as u64, bytemuck::bytes_of(&typeinfo));
        self.render.queue.write_buffer(&self.render.bullet_seed_buf, (idx * 4) as u64, bytemuck::bytes_of(&init.seed));

        self.bullet_write_idx = (self.bullet_write_idx + 1) % MAX_BULLETS;
        self.state.bullet_count = self.state.bullet_count.max((idx + 1) as u32);
    }

    // Dynamic particle spawning helper
    fn spawn_particle(&mut self, part: Particle) {
        let idx = self.particle_write_idx;
        self.render.queue.write_buffer(&self.render.particle_buf, (idx * std::mem::size_of::<Particle>()) as u64, bytemuck::bytes_of(&part));
        self.particle_write_idx = (self.particle_write_idx + 1) % MAX_PARTICLES;
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
}
