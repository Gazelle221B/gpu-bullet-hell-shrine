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
    _padding: vec2<u32>,
};

struct BulletMeta {
    radius: f32,
    age: f32,
    lifetime: f32,
    packed_flags: u32,
};

struct CollisionResult {
    hit_count: atomic<u32>,
    graze_count: atomic<u32>,
    hit_bullet_ids: array<u32, 16>,
    graze_bullet_ids: array<u32, 64>,
};

@group(0) @binding(0) var<uniform> uniforms: FrameUniforms;
@group(0) @binding(1) var<storage, read_write> bullet_position: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read_write> bullet_meta: array<BulletMeta>;

@group(1) @binding(0) var<storage, read> grid_count: array<u32>;
@group(1) @binding(1) var<storage, read> grid_offset: array<u32>;
@group(1) @binding(2) var<storage, read> grid_items: array<u32>;
@group(1) @binding(3) var<storage, read_write> results: CollisionResult;

// 1. Clear Collision Result Counters
@compute @workgroup_size(1)
fn clear_collision() {
    atomicStore(&results.hit_count, 0u);
    atomicStore(&results.graze_count, 0u);
}

// 2. Compute Player Spatial Hash Collision (9-thread workgroup: one thread per 3×3 cell)
@compute @workgroup_size(9)
fn detect_collision(@builtin(local_invocation_id) lid: vec3<u32>) {
    let player_pos = uniforms.player_position;
    
    let player_hitbox_r = 3.0;
    let player_graze_r = 24.0;
    
    let player_cell_x = i32(player_pos.x / uniforms.grid_cell_size);
    let player_cell_y = i32(player_pos.y / uniforms.grid_cell_size);
    
    let grid_w = i32(uniforms.grid_dims.x);
    let grid_h = i32(uniforms.grid_dims.y);

    let i = lid.x;
    let dx = i32(i % 3u) - 1;
    let dy = i32(i / 3u) - 1;
    let cx = player_cell_x + dx;
    let cy = player_cell_y + dy;

    if (cx < 0 || cx >= grid_w || cy < 0 || cy >= grid_h) {
        return;
    }

    let cell_idx = u32(cy * grid_w + cx);
    let count = grid_count[cell_idx];
    if (count == 0u) {
        return;
    }

    let offset = grid_offset[cell_idx];

    for (var i = 0u; i < count; i = i + 1u) {
        let bullet_idx = grid_items[offset + i];
        let b_pos = bullet_position[bullet_idx];
        let b_meta = bullet_meta[bullet_idx];

        if ((b_meta.packed_flags & 1u) == 0u) {
            continue;
        }

        let dist = distance(player_pos, b_pos);
        let col_dist = player_hitbox_r + b_meta.radius;
        let graze_dist = player_graze_r + b_meta.radius;

        if (dist < col_dist) {
            let slot = atomicAdd(&results.hit_count, 1u);
            if (slot < 16u) {
                results.hit_bullet_ids[slot] = bullet_idx;
            }
        } else if (dist < graze_dist) {
            let slot = atomicAdd(&results.graze_count, 1u);
            if (slot < 64u) {
                results.graze_bullet_ids[slot] = bullet_idx;
            }
        }
    }
}
