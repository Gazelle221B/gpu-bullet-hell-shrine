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
@group(0) @binding(7) var<storage, read_write> active_bullet_count: atomic<u32>;

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

// 3. Prefix Sum Pass — Blelloch single-workgroup exclusive scan
// Pads grid to next power-of-two (4096 for 40×60=2400 cells).
// Workgroup memory: 4096 × 4 = 16 KB (within WebGPU-mandated minimum).

const SCAN_N: u32 = 4096u;
const SCAN_THREADS: u32 = 256u;
const SCAN_PER_THREAD: u32 = SCAN_N / SCAN_THREADS;

var<workgroup> scan_data: array<u32, SCAN_N>;

@compute @workgroup_size(SCAN_THREADS)
fn prefix_sum(@builtin(local_invocation_id) lid: vec3<u32>) {
    let total = uniforms.grid_dims.x * uniforms.grid_dims.y;
    let tid = lid.x;

    // Load phase: read each atomic cell count into workgroup memory
    for (var k: u32 = 0u; k < SCAN_PER_THREAD; k = k + 1u) {
        let idx = tid * SCAN_PER_THREAD + k;
        if (idx < total) {
            scan_data[idx] = atomicLoad(&grid_count[idx]);
        } else {
            scan_data[idx] = 0u;
        }
    }
    workgroupBarrier();

    // Blelloch up-sweep (reduce phase)
    var stride: u32 = 1u;
    while (stride < SCAN_N) {
        let span = stride * 2u;
        for (var base: u32 = 0u; base < SCAN_N; base = base + SCAN_THREADS * span) {
            let idx = base + (tid + 1u) * span - 1u;
            if (idx < SCAN_N) {
                scan_data[idx] = scan_data[idx] + scan_data[idx - stride];
            }
        }
        workgroupBarrier();
        stride = stride * 2u;
    }

    // Clear last element for exclusive scan
    if (tid == 0u) {
        scan_data[SCAN_N - 1u] = 0u;
    }
    workgroupBarrier();

    // Blelloch down-sweep (distribution phase)
    stride = SCAN_N / 2u;
    while (stride > 0u) {
        let span = stride * 2u;
        for (var base: u32 = 0u; base < SCAN_N; base = base + SCAN_THREADS * span) {
            let idx = base + (tid + 1u) * span - 1u;
            if (idx < SCAN_N) {
                let t = scan_data[idx - stride];
                scan_data[idx - stride] = scan_data[idx];
                scan_data[idx] = scan_data[idx] + t;
            }
        }
        workgroupBarrier();
        stride = stride / 2u;
    }

    // Store back exclusive scan results to grid_offset
    for (var k: u32 = 0u; k < SCAN_PER_THREAD; k = k + 1u) {
        let idx = tid * SCAN_PER_THREAD + k;
        if (idx < total) {
            grid_offset[idx] = scan_data[idx];
        }
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
