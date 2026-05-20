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
@group(0) @binding(1) var<storage, read> bullet_position: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> bullet_meta: array<BulletMeta>;

@group(1) @binding(0) var<storage, read_write> grid_count: array<atomic<u32>>;
@group(1) @binding(1) var<storage, read_write> grid_offset: array<u32>;
@group(1) @binding(2) var<storage, read_write> grid_items: array<u32>;
@group(1) @binding(3) var<storage, read_write> grid_tracker: array<atomic<u32>>;

// Helper to get 1D grid cell index
fn get_cell_idx(pos: vec2<f32>) -> u32 {
    let cell_x = clamp(u32(pos.x / uniforms.grid_cell_size), 0u, uniforms.grid_dims.x - 1u);
    let cell_y = clamp(u32(pos.y / uniforms.grid_cell_size), 0u, uniforms.grid_dims.y - 1u);
    return cell_y * uniforms.grid_dims.x + cell_x;
}

// 1. Clear Pass
@compute @workgroup_size(64)
@diagnostic(off, derivative_uniform_control_flow)
fn clear_grid(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    let total_cells = uniforms.grid_dims.x * uniforms.grid_dims.y;
    if (index >= total_cells) {
        return;
    }
    atomicStore(&grid_count[index], 0u);
    atomicStore(&grid_tracker[index], 0u);
}

// 2. Count Pass
@compute @workgroup_size(64)
fn count_grid(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= uniforms.bullet_count) {
        return;
    }

    let meta = bullet_meta[index];
    if ((meta.packed_flags & 1u) == 0u) {
        return;
    }

    let pos = bullet_position[index];
    let cell_idx = get_cell_idx(pos);
    
    atomicAdd(&grid_count[cell_idx], 1u);
}

// 3. Prefix Sum Pass (Single workgroup of 1 thread for fast prefix calculation)
@compute @workgroup_size(1)
fn prefix_sum() {
    let total_cells = uniforms.grid_dims.x * uniforms.grid_dims.y;
    var sum = 0u;
    for (var i = 0u; i < total_cells; i = i + 1u) {
        grid_offset[i] = sum;
        sum += atomicLoad(&grid_count[i]);
    }
}

// 4. Sort / Populate Pass
@compute @workgroup_size(64)
fn sort_grid(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= uniforms.bullet_count) {
        return;
    }

    let meta = bullet_meta[index];
    if ((meta.packed_flags & 1u) == 0u) {
        return;
    }

    let pos = bullet_position[index];
    let cell_idx = get_cell_idx(pos);
    
    // Atomically increment local index in this bucket
    let local_idx = atomicAdd(&grid_tracker[cell_idx], 1u);
    let offset = grid_offset[cell_idx];
    
    // Write bullet index to grid items
    grid_items[offset + local_idx] = index;
}
