use shared::{FrameUniforms, CollisionResult, BulletInit};

pub struct Player {
    pub position: [f32; 2],
    pub speed_normal: f32,
    pub speed_slow: f32,
    pub lives: u32,
    pub bombs: u32,
    pub score: u32,
    pub graze: u32,
    pub is_invincible: bool,
    pub invincibility_timer: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            position: [640.0, 800.0], // Centered at bottom of 1280x960 arena coordinates
            speed_normal: 5.5,
            speed_slow: 2.2,
            lives: 3,
            bombs: 2,
            score: 0,
            graze: 0,
            is_invincible: false,
            invincibility_timer: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.is_invincible {
            self.invincibility_timer -= dt;
            if self.invincibility_timer <= 0.0 {
                self.is_invincible = false;
                self.invincibility_timer = 0.0;
            }
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Boss {
    pub position: [f32; 2],
    pub hp: f32,
    pub max_hp: f32,
    pub current_phase: u32,
    pub phase_timer: f32,
    pub phase_durations: [f32; 5],
    pub final_spell_timer: f32,
}

impl Boss {
    pub fn new() -> Self {
        Self {
            position: [640.0, 250.0],
            hp: 1000.0,
            max_hp: 1000.0,
            current_phase: 0,
            phase_timer: 0.0,
            phase_durations: [20.0, 40.0, 45.0, 45.0, 30.0],
            final_spell_timer: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.phase_timer += dt;
        // Simple horizontal hover movement
        self.position[0] = 640.0 + (self.phase_timer * 1.2).sin() * 250.0;
        self.position[1] = 250.0 + (self.phase_timer * 0.8).cos() * 30.0;
    }
}

impl Default for Boss {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GameState {
    pub player: Player,
    pub boss: Boss,
    pub time: f32,
    pub is_game_over: bool,
    pub is_victory: bool,
    pub active_pattern: u32,
    pub bullet_count: u32,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player::new(),
            boss: Boss::new(),
            time: 0.0,
            is_game_over: false,
            is_victory: false,
            active_pattern: 1, // Pattern 1 (Star circular ring)
            bullet_count: 0,
        }
    }

    pub fn update(&mut self, dt: f32, keys: &[bool; 256], shift: bool) {
        if self.is_game_over || self.is_victory {
            return;
        }

        self.time += dt;
        self.player.update(dt);
        self.boss.update(dt);

        // Manage keyboard movement inputs
        let speed = if shift { self.player.speed_slow } else { self.player.speed_normal };
        let mut dx = 0.0_f32;
        let mut dy = 0.0_f32;

        // WASD or Arrow Keys
        if keys[87] || keys[38] { dy -= 1.0; } // W / Up
        if keys[83] || keys[40] { dy += 1.0; } // S / Down
        if keys[65] || keys[37] { dx -= 1.0; } // A / Left
        if keys[68] || keys[39] { dx += 1.0; } // D / Right

        if dx != 0.0 && dy != 0.0 {
            let length = (dx * dx + dy * dy).sqrt();
            dx /= length;
            dy /= length;
        }

        self.player.position[0] += dx * speed;
        self.player.position[1] += dy * speed;

        // Keep inside bounds (standard layout boundaries)
        let margin = 20.0;
        let min_x = 320.0;
        let max_x = 960.0; // Playfield bounds
        let min_y = 50.0;
        let max_y = 910.0;

        if self.player.position[0] < min_x + margin { self.player.position[0] = min_x + margin; }
        if self.player.position[0] > max_x - margin { self.player.position[0] = max_x - margin; }
        if self.player.position[1] < min_y + margin { self.player.position[1] = min_y + margin; }
        if self.player.position[1] > max_y - margin { self.player.position[1] = max_y - margin; }

        // Phase state machine (GDD §9.2)
        if self.boss.current_phase < 4 {
            if self.boss.phase_timer >= self.boss.phase_durations[self.boss.current_phase as usize] {
                self.boss.phase_timer = 0.0;
                self.boss.current_phase += 1;
                if self.boss.current_phase == 4 {
                    self.boss.final_spell_timer = 30.0;
                }
            }
            // Set active_pattern based on (phase, half-point)
            let half = self.boss.phase_durations[self.boss.current_phase as usize] * 0.5;
            self.active_pattern = match self.boss.current_phase {
                0 => 1,
                1 => if self.boss.phase_timer >= half { 2 } else { 1 },
                2 => if self.boss.phase_timer >= half { 4 } else { 3 },
                3 => if self.boss.phase_timer >= half { 6 } else { 5 },
                _ => 1,
            };
        } else if self.boss.current_phase == 4 {
            self.boss.final_spell_timer -= dt;
            self.active_pattern = 7;
            if self.boss.final_spell_timer <= 0.0 {
                self.is_victory = true;
            }
        }
    }

    pub fn trigger_bomb(&mut self) -> bool {
        if self.player.bombs > 0 && !self.player.is_invincible {
            self.player.bombs -= 1;
            self.player.is_invincible = true;
            self.player.invincibility_timer = 3.0; // 3 seconds invincibility
            true
        } else {
            false
        }
    }

    pub fn handle_collision_results(&mut self, results: &CollisionResult) {
        if self.player.is_invincible {
            return;
        }

        // Add graze score
        if results.graze_count > 0 {
            self.player.graze += results.graze_count;
            self.player.score += results.graze_count * 150;
        }

        // Handle hit
        if results.hit_count > 0 {
            if self.player.lives > 0 {
                self.player.lives -= 1;
                self.player.bombs = 2; // Restore bombs on death
                self.player.is_invincible = true;
                self.player.invincibility_timer = 2.5; // Invincible on spawn
                self.player.position = [640.0, 800.0]; // Reset position
            } else {
                self.is_game_over = true;
            }
        }
    }

    pub fn emit_pattern(&self, ticks: u32) -> Vec<BulletInit> {
        let boss_pos = self.boss.position;
        let pat_id = self.active_pattern;
        let mut bullets = Vec::new();

        if pat_id == 1 {
            if ticks.is_multiple_of(20) {
                let count = 36;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    bullets.push(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 190.0, angle.sin() * 190.0],
                        acceleration: [0.0, 0.0],
                        radius: 6.0,
                        lifetime: 7.0,
                        pattern_id: 1,
                        bullet_type: 2,
                        color_id: (ticks / 20) % 6,
                        seed: i,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 2 {
            if ticks.is_multiple_of(3) {
                let base_angle = (ticks as f32) * 0.12;
                for i in 0..4 {
                    let angle = base_angle + (i as f32) * (std::f32::consts::PI / 2.0);
                    bullets.push(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 210.0, angle.sin() * 210.0],
                        acceleration: [0.0, 0.0],
                        radius: 5.0,
                        lifetime: 6.5,
                        pattern_id: 2,
                        bullet_type: 1,
                        color_id: 3,
                        seed: i * ticks,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 3 {
            if ticks.is_multiple_of(8) {
                for i in 0..5 {
                    let rx = 340.0 + ((ticks * 71 + i * 29) % 600) as f32;
                    bullets.push(BulletInit {
                        position: [rx, 80.0],
                        velocity: [0.0, 110.0],
                        acceleration: [0.0, 50.0],
                        radius: 8.0,
                        lifetime: 8.0,
                        pattern_id: 3,
                        bullet_type: 3,
                        color_id: 4,
                        seed: i * 13,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 4 {
            if ticks.is_multiple_of(7) {
                let base_angle = (ticks as f32) * 0.09;
                for i in 0..6 {
                    let angle = base_angle + (i as f32) * (2.0 * std::f32::consts::PI / 6.0);
                    bullets.push(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 160.0, angle.sin() * 160.0],
                        acceleration: [0.0, 0.0],
                        radius: 7.0,
                        lifetime: 7.0,
                        pattern_id: 4,
                        bullet_type: 2,
                        color_id: 1,
                        seed: i * 23,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 5 {
            if ticks.is_multiple_of(14) {
                let p_pos = self.player.position;
                let target_angle = (p_pos[1] - boss_pos[1]).atan2(p_pos[0] - boss_pos[0]);
                for i in 0..3 {
                    let angle_offset = (i as f32 - 1.0) * 0.1;
                    let angle = target_angle + angle_offset;
                    bullets.push(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 320.0, angle.sin() * 320.0],
                        acceleration: [0.0, 0.0],
                        radius: 4.0,
                        lifetime: 4.5,
                        pattern_id: 5,
                        bullet_type: 4,
                        color_id: 5,
                        seed: i + ticks,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 6 {
            if ticks.is_multiple_of(25) {
                let count = 48;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    bullets.push(BulletInit {
                        position: boss_pos,
                        velocity: [angle.cos() * 300.0, angle.sin() * 300.0],
                        acceleration: [-angle.cos() * 150.0, -angle.sin() * 150.0],
                        radius: 5.5,
                        lifetime: 7.5,
                        pattern_id: 6,
                        bullet_type: 1,
                        color_id: 2,
                        seed: i * 47,
                        flags: 1,
                        _padding: [0; 3],
                    });
                }
            }
        } else if pat_id == 7 {
            if ticks.is_multiple_of(10) {
                let count = 30;
                for i in 0..count {
                    let angle = (i as f32) * (2.0 * std::f32::consts::PI / count as f32);
                    bullets.push(BulletInit {
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
            if ticks.is_multiple_of(2) {
                let angle = (ticks as f32) * 0.15;
                bullets.push(BulletInit {
                    position: boss_pos,
                    velocity: [angle.cos() * 240.0, angle.sin() * 240.0],
                    acceleration: [0.0, 0.0],
                    radius: 5.0,
                    lifetime: 6.0,
                    pattern_id: 7,
                    bullet_type: 1,
                    color_id: 3,
                    seed: ticks,
                    flags: 1,
                    _padding: [0; 3],
                });
            }
        }

        bullets
    }

    pub fn fill_uniforms(&self, screen_w: f32, screen_h: f32) -> FrameUniforms {
        FrameUniforms {
            time: self.time,
            delta_time: 0.0166,
            phase_time: self.boss.phase_timer,
            bullet_count: self.bullet_count,
            player_position: self.player.position,
            boss_position: self.boss.position,
            screen_size: [screen_w, screen_h],
            pattern_id: self.active_pattern,
            grid_cell_size: shared::GRID_CELL_SIZE,
            grid_dims: [shared::GRID_WIDTH, shared::GRID_HEIGHT],
            _padding: [0; 3],
        }
    }

    pub fn get_phase_display_name(&self) -> String {
        match (self.boss.current_phase, self.active_pattern) {
            (0, _) => "Tutorial: 結界調律".to_string(),
            (1, 1) => "Phase 1-A: 星降りの円環".to_string(),
            (1, 2) => "Phase 1-B: 二重螺旋の霊札".to_string(),
            (2, 3) => "Phase 2-A: 月蝕の格子雨".to_string(),
            (2, 4) => "Phase 2-B: 蝶の迷路".to_string(),
            (3, 5) => "Phase 3-A: 時計盤レーザー".to_string(),
            (3, 6) => "Phase 3-B: 星屑反転".to_string(),
            (4, _) => "Final Spell: 天球演算「星守ノ夜」".to_string(),
            _ => "—".to_string(),
        }
    }

    pub fn get_final_spell_timer(&self) -> f32 {
        self.boss.final_spell_timer
    }

    pub fn is_final_spell_active(&self) -> bool {
        self.boss.current_phase == 4
    }
}
