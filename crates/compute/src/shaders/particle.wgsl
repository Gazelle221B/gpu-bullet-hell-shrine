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

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    color: vec4<f32>,
    size: f32,
    age: f32,
    lifetime: f32,
    flags: u32,
};

@group(0) @binding(0) var<uniform> uniforms: FrameUniforms;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&particles)) {
        return;
    }

    var p = particles[index];
    if (p.flags == 0u) {
        return;
    }

    // Update age
    let dt = uniforms.delta_time;
    p.age += dt;

    if (p.age >= p.lifetime) {
        p.flags = 0u; // Deactivate
        particles[index] = p;
        return;
    }

    // Euler integration
    p.position += p.velocity * dt;
    
    // Slow down velocity slightly (air friction)
    p.velocity = p.velocity * 0.98;

    // Fade out color and size
    let t = p.age / p.lifetime;
    p.color.a = 1.0 - t;
    p.size = p.size * (1.0 - t * 0.5);

    // Write back
    particles[index] = p;
}
