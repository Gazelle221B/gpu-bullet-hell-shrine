use bytemuck::{Pod, Zeroable};

// Constants
pub const MAX_BULLETS: usize = 30000;
pub const MAX_PARTICLES: usize = 16384;
pub const GRID_CELL_SIZE: f32 = 32.0;
pub const GRID_WIDTH: u32 = 40; // 40 * 32 = 1280px
pub const GRID_HEIGHT: u32 = 60; // 60 * 32 = 1920px (large screen arena)
pub const MAX_GRID_BUCKET_ITEMS: usize = 64;

/// Frame uniform layout passed to all shader stages
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FrameUniforms {
    pub time: f32,
    pub delta_time: f32,
    pub phase_time: f32,
    pub bullet_count: u32,
    pub player_position: [f32; 2],
    pub boss_position: [f32; 2],
    pub screen_size: [f32; 2],
    pub pattern_id: u32,
    pub grid_cell_size: f32,
    pub grid_dims: [u32; 2],
    pub _padding: [u32; 3], // Align to 16 bytes (std140)
}

/// A CPU-side Bullet description used during generation/init
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BulletInit {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub acceleration: [f32; 2],
    pub radius: f32,
    pub lifetime: f32,
    pub pattern_id: u32,
    pub bullet_type: u32,
    pub color_id: u32,
    pub seed: u32,
    pub flags: u32,
    pub _padding: [u32; 3], // Alignment padding
}

/// The result written back by the GPU collision compute pass
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CollisionResult {
    pub hit_count: u32,
    pub graze_count: u32,
    pub hit_bullet_ids: [u32; 16],
    pub graze_bullet_ids: [u32; 64],
}

/// Particle struct used in the compute particle pipeline
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub age: f32,
    pub lifetime: f32,
    pub flags: u32,
}

/// Structured debugging counter to send to the UI
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct DebugCounters {
    pub fps: f32,
    pub frame_ms: f32,
    pub compute_ms: f32,
    pub render_ms: f32,
    pub active_bullets: u32,
    pub active_particles: u32,
    pub draw_calls: u32,
    pub buffer_upload_bytes: u32,
    pub grid_max_bucket: u32,
    pub grid_avg_bucket: f32,
    pub collision_hits: u32,
    pub collision_grazes: u32,
    pub timing_is_approximate: u32, // 0 = real timestamp-query, 1 = CPU approximation
    pub _pad_counters: [u32; 3],    // 16-byte boundary alignment
}
