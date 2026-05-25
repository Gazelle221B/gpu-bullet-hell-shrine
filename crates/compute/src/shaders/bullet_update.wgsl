struct FrameUniforms {
    time: f32,
    delta_time: f32,
    phase_time: f32,
    bullet_count: u32,
    player_position: vec2<f32>,
    boss_position: vec2<f32>,
    screen_size: vec2<f32>,
    pattern_id: u32,
    grid_cell_size: f32,
    grid_dims: vec2<u32>,
};

struct BulletMeta {
    radius: f32,
    age: f32,
    lifetime: f32,
    packed_flags: u32,
};

@group(0) @binding(0) var<uniform> uniforms: FrameUniforms;
@group(0) @binding(1) var<storage, read_write> bullet_position: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> bullet_velocity: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read_write> bullet_accel: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read_write> bullet_meta: array<BulletMeta>;
@group(0) @binding(5) var<storage, read_write> bullet_typeinfo: array<u32>;
@group(0) @binding(6) var<storage, read_write> bullet_seed: array<u32>;
@group(0) @binding(7) var<storage, read_write> active_bullet_count: atomic<u32>;

// PRNG from seed for pattern variations
fn rand(seed: ptr<function, u32>) -> f32 {
    *seed = (*seed ^ 61u) ^ (*seed >> 16u);
    *seed *= 9u;
    *seed = *seed ^ (*seed >> 4u);
    *seed *= 0x27d4eb2du;
    *seed = *seed ^ (*seed >> 15u);
    return f32(*seed) / 4294967295.0;
}

@compute @workgroup_size(1)
fn clear_active_count() {
    atomicStore(&active_bullet_count, 0u);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= uniforms.bullet_count) {
        return;
    }

    var meta = bullet_meta[index];
    
    // Check if active (bit 0 of packed_flags)
    if ((meta.packed_flags & 1u) == 0u) {
        return;
    }

    // Update age
    let dt = uniforms.delta_time;
    meta.age += dt;

    if (meta.age >= meta.lifetime) {
        meta.packed_flags &= ~1u; // Deactivate
        bullet_meta[index] = meta;
        return;
    }

    var pos = bullet_position[index];
    var vel = bullet_velocity[index];
    var accel = bullet_accel[index];
    
    var seed = bullet_seed[index];
    
    // Unpack typeinfo
    let typeinfo = bullet_typeinfo[index];
    let bullet_type = typeinfo & 0xFFu;
    let pattern_id = (typeinfo >> 8u) & 0xFFu;
    let color_id = (typeinfo >> 16u) & 0xFFu;

    // Apply pattern physics
    if (pattern_id == 1u) {
        // Pattern 1: Star circular rings - slow down slightly
        vel = vel * 0.995;
    } else if (pattern_id == 2u) {
        // Pattern 2: Helix double spiral - add tangential force
        let r = length(pos - uniforms.boss_position);
        if (r > 10.0) {
            let dir = (pos - uniforms.boss_position) / r;
            let tangent = vec2<f32>(-dir.y, dir.x);
            vel += tangent * sin(uniforms.time * 2.0 + r * 0.05) * 5.0 * dt;
        }
    } else if (pattern_id == 3u) {
        // Pattern 3: Lunar Lattice Rain - add downward gravity
        accel.y = 80.0;
    } else if (pattern_id == 4u) {
        // Pattern 4: Butterflies - sinusoidal swaying
        vel.x += sin(meta.age * 5.0 + f32(index)) * 40.0 * dt;
    } else if (pattern_id == 5u) {
        // Pattern 5: Lasers / Needles - speed up over time
        vel += normalize(vel + vec2<f32>(1e-5, 0.0)) * 60.0 * dt;
    } else if (pattern_id == 6u) {
        // Pattern 6: Stardust Inversion - stop and fly back towards player
        let mid_life = meta.lifetime * 0.45;
        if (meta.age > mid_life && meta.age < mid_life + dt * 1.5) {
            // Target the player
            let player_dir = normalize(uniforms.player_position - pos + vec2<f32>(rand(&seed) - 0.5, rand(&seed) - 0.5) * 20.0);
            vel = player_dir * 300.0;
            accel = vec2<f32>(0.0, 0.0);
        }
    } else if (pattern_id == 7u) {
        // Final Spell: Celestial Stress Test - mix spiral and tracking
        let wave = sin(uniforms.time * 4.0 + f32(index % 10u));
        vel += vec2<f32>(wave * 30.0, abs(wave) * 15.0) * dt;
    }

    // Integrate
    vel += accel * dt;
    pos += vel * dt;

    // Check screen boundaries (derived from uniforms.screen_size)
    // Playfield occupies middle 50% horizontally, full height minus 50px top/bottom.
    // At 1280×960 these reduce to the previous hardcoded margins.
    let margin = 50.0;
    let pf_w = uniforms.screen_size.x * 0.5;
    let pf_cx = uniforms.screen_size.x * 0.5;
    let min_x = pf_cx - pf_w * 0.5 - margin;
    let max_x = pf_cx + pf_w * 0.5 + margin;
    let min_y = 50.0 - margin;
    let max_y = uniforms.screen_size.y - 50.0 + margin;

    if (pos.x < min_x || pos.x > max_x || pos.y < min_y || pos.y > max_y) {
        meta.packed_flags &= ~1u; // Deactivate if offscreen
    }

    if ((meta.packed_flags & 1u) != 0u) {
        atomicAdd(&active_bullet_count, 1u);
    }

    // Write back SoA
    bullet_position[index] = pos;
    bullet_velocity[index] = vel;
    bullet_meta[index] = meta;
    bullet_seed[index] = seed;
}
