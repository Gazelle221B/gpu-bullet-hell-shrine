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

@group(0) @binding(1) var<storage, read> bullet_position: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> bullet_meta: array<BulletMeta>;
@group(0) @binding(3) var<storage, read> bullet_typeinfo: array<u32>;

@group(0) @binding(4) var<storage, read> particles: array<Particle>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @flat typeinfo: u32,
    @location(2) @flat radius: f32,
    @location(3) color: vec4<f32>,
};

// Projection helper from game space (1280x960) to WebGPU clip space
fn project_pos(pos: vec2<f32>) -> vec2<f32> {
    let clip_x = (pos.x / 1280.0) * 2.0 - 1.0;
    let clip_y = 1.0 - (pos.y / 960.0) * 2.0;
    return vec2<f32>(clip_x, clip_y);
}

// 1. Instanced Bullets Shader Stage
@vertex
fn vs_bullet(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32
) -> VertexOutput {
    var out: VertexOutput;

    let meta = bullet_meta[instance_idx];
    let typeinfo = bullet_typeinfo[instance_idx];
    
    // Check active
    if ((meta.packed_flags & 1u) == 0u) {
        out.clip_position = vec4<f32>(0.0, 0.0, 0.0, 0.0); // Discard
        return out;
    }

    let b_pos = bullet_position[instance_idx];
    out.typeinfo = typeinfo;
    out.radius = meta.radius;

    // Construct local quad coordinates based on vertex index (0 to 5 for standard triangle list)
    // Vertices: (-1, -1), (1, -1), (-1, 1), (-1, 1), (1, -1), (1, 1)
    var local_uv = vec2<f32>(-1.0, -1.0);
    if (vertex_idx == 1u || vertex_idx == 4u) {
        local_uv.x = 1.0;
    }
    if (vertex_idx == 2u || vertex_idx == 3u) {
        local_uv.y = 1.0;
    }
    if (vertex_idx == 5u) {
        local_uv = vec2<f32>(1.0, 1.0);
    }

    out.uv = local_uv;

    // Offset is scaled by the bullet's visual bounds (giving extra padding for outer glow)
    let visual_r = meta.radius * 2.5;
    let vertex_pos = b_pos + local_uv * visual_r;

    out.clip_position = vec4<f32>(project_pos(vertex_pos), 0.0, 1.0);
    return out;
}

// Color palettes for bullet color IDs
fn get_bullet_color(color_id: u32) -> vec4<f32> {
    if (color_id == 0u) { return vec4<f32>(0.0, 0.9, 1.0, 1.0); } // Cyan
    if (color_id == 1u) { return vec4<f32>(1.0, 0.2, 0.6, 1.0); } // Magenta
    if (color_id == 2u) { return vec4<f32>(1.0, 0.8, 0.0, 1.0); } // Gold
    if (color_id == 3u) { return vec4<f32>(0.7, 0.1, 1.0, 1.0); } // Purple
    if (color_id == 4u) { return vec4<f32>(0.1, 0.9, 0.4, 1.0); } // Green
    return vec4<f32>(1.0, 0.4, 0.1, 1.0); // Orange
}

@fragment
fn fs_bullet(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.clip_position.w == 0.0) {
        discard;
    }

    let dist_sq = dot(in.uv, in.uv);
    if (dist_sq > 1.0) {
        discard;
    }

    let bullet_type = in.typeinfo & 0xFFu;
    let color_id = (in.typeinfo >> 16u) & 0xFFu;
    let base_color = get_bullet_color(color_id);

    // Procedural shape rendering
    if (bullet_type == 1u) {
        // Shape 1: 5-Pointed Star
        let r = length(in.uv);
        let angle = atan2(in.uv.y, in.uv.x) + uniforms.time * 2.0;
        let star_shape = abs(sin(angle * 5.0 * 0.5)) * 0.5 + 0.5;
        if (r > star_shape) {
            discard;
        }
        let glow = (1.0 - r / star_shape) * 1.5;
        let c = mix(vec4<f32>(1.0, 1.0, 1.0, 1.0), base_color, clamp(r * 2.0, 0.0, 1.0));
        return c * glow;
    } else if (bullet_type == 3u) {
        // Shape 3: Talisman Rectangle
        let dist_x = abs(in.uv.x);
        let dist_y = abs(in.uv.y);
        if (dist_x > 0.85 || dist_y > 0.95) {
            discard;
        }
        let border = smoothstep(0.7, 0.85, dist_x) + smoothstep(0.8, 0.95, dist_y);
        let border_color = mix(base_color * 1.5, vec4<f32>(1.0, 1.0, 1.0, 1.0), border);
        return border_color;
    } else if (bullet_type == 4u) {
        // Shape 4: Arrow / Needle
        let width_scale = 1.0 - (in.uv.y + 1.0) * 0.5; // Tapers at tip
        if (abs(in.uv.x) > width_scale * 0.5) {
            discard;
        }
        let radial = 1.0 - abs(in.uv.x);
        return mix(vec4<f32>(1.0, 1.0, 1.0, 1.0), base_color, t(in.uv.y)) * radial;
    }

    // Shape 2: Orb circle (Default) with premium core glow
    let r = length(in.uv);
    let border = smoothstep(0.4, 0.9, r);
    let glow = (1.0 - r) * 2.0;
    
    // Core white center transitioning to colored edge glow
    let color = mix(vec4<f32>(1.0, 1.0, 1.0, 1.0), base_color, border);
    return color * glow;
}

fn t(v: f32) -> f32 {
    return clamp((v + 1.0) * 0.5, 0.0, 1.0);
}

// 2. Background Shader Stage
@vertex
fn vs_background(@builtin(vertex_index) vertex_idx: u32) -> @builtin(position) vec4<f32> {
    // Generate full-screen quad directly
    var pos = vec2<f32>(-1.0, -1.0);
    if (vertex_idx == 1u || vertex_idx == 4u) {
        pos.x = 1.0;
    }
    if (vertex_idx == 2u || vertex_idx == 3u) {
        pos.y = 1.0;
    }
    if (vertex_idx == 5u) {
        pos = vec2<f32>(1.0, 1.0);
    }
    return vec4<f32>(pos, 0.999, 1.0); // Render at far clip plane
}

@fragment
fn fs_background(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_pos.xy / uniforms.screen_size;
    
    // Modern ambient space-shrine backdrop
    // Vertical gradient
    let top_color = vec3<f32>(0.02, 0.01, 0.05);
    let bot_color = vec3<f32>(0.05, 0.03, 0.12);
    var bg = mix(top_color, bot_color, uv.y);

    // Glowing shrine border boundary
    // Playfield coordinates on screen: 320 to 960 widthwise, 50 to 910 heightwise
    let border_x_min = 320.0 / 1280.0;
    let border_x_max = 960.0 / 1280.0;
    let border_y_min = 50.0 / 960.0;
    let border_y_max = 910.0 / 960.0;

    let arena_margin = 0.005;

    // Glowing border calculations
    if (uv.x > border_x_min - arena_margin && uv.x < border_x_max + arena_margin &&
        uv.y > border_y_min - arena_margin && uv.y < border_y_max + arena_margin) {
        
        let on_edge_x = min(abs(uv.x - border_x_min), abs(uv.x - border_x_max));
        let on_edge_y = min(abs(uv.y - border_y_min), abs(uv.y - border_y_max));
        let edge = min(on_edge_x, on_edge_y);

        if (edge < 0.002) {
            let pulse = sin(uniforms.time * 3.0) * 0.3 + 0.7;
            return vec4<f32>(vec3<f32>(0.5, 0.2, 0.8) * pulse, 1.0); // Purple barrier border glow!
        }
    } else {
        // Dim the outside playfield menu areas
        bg *= 0.6;
    }

    // Add rotating abstract star field
    let cos_t = cos(uniforms.time * 0.03);
    let sin_t = sin(uniforms.time * 0.03);
    let rot_uv = vec2<f32>(
        (uv.x - 0.5) * cos_t - (uv.y - 0.5) * sin_t,
        (uv.x - 0.5) * sin_t + (uv.y - 0.5) * cos_t
    );

    let star_grid = sin(rot_uv.x * 30.0) * sin(rot_uv.y * 30.0);
    if (star_grid > 0.99) {
        bg += vec3<f32>(0.8, 0.9, 1.0) * 0.5 * (sin(uniforms.time + star_grid * 20.0) * 0.5 + 0.5);
    }

    return vec4<f32>(bg, 1.0);
}

// 3. Particles Shader Stage
@vertex
fn vs_particles(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32
) -> VertexOutput {
    var out: VertexOutput;

    let p = particles[instance_idx];
    if (p.flags == 0u) {
        out.clip_position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    out.color = p.color;

    // Billboard quad offsets
    var local_uv = vec2<f32>(-1.0, -1.0);
    if (vertex_idx == 1u || vertex_idx == 4u) {
        local_uv.x = 1.0;
    }
    if (vertex_idx == 2u || vertex_idx == 3u) {
        local_uv.y = 1.0;
    }
    if (vertex_idx == 5u) {
        local_uv = vec2<f32>(1.0, 1.0);
    }

    out.uv = local_uv;

    let vertex_pos = p.position + local_uv * p.size;
    out.clip_position = vec4<f32>(project_pos(vertex_pos), 0.0, 1.0);

    return out;
}

@fragment
fn fs_particles(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.clip_position.w == 0.0) {
        discard;
    }
    let r = length(in.uv);
    if (r > 1.0) {
        discard;
    }
    let glow = (1.0 - r);
    return in.color * glow * 1.5;
}

// 4. Character rendering (Player and Boss)
@vertex
fn vs_entity(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32 // 0 = Player, 1 = Boss
) -> VertexOutput {
    var out: VertexOutput;
    
    var base_pos = uniforms.player_position;
    var size = 16.0;
    out.typeinfo = 0u; // Player

    if (instance_idx == 1u) {
        base_pos = uniforms.boss_position;
        size = 32.0;
        out.typeinfo = 1u; // Boss
    }

    var local_uv = vec2<f32>(-1.0, -1.0);
    if (vertex_idx == 1u || vertex_idx == 4u) {
        local_uv.x = 1.0;
    }
    if (vertex_idx == 2u || vertex_idx == 3u) {
        local_uv.y = 1.0;
    }
    if (vertex_idx == 5u) {
        local_uv = vec2<f32>(1.0, 1.0);
    }

    out.uv = local_uv;
    
    // Apply size multiplier for outer halo
    let vertex_pos = base_pos + local_uv * size * 1.8;
    out.clip_position = vec4<f32>(project_pos(vertex_pos), 0.0, 1.0);
    return out;
}

@fragment
fn fs_entity(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = length(in.uv);
    if (r > 1.0) {
        discard;
    }

    if (in.typeinfo == 0u) {
        // Player: Sleek glowing cyan diamond with inner white core
        let manhattan = abs(in.uv.x) + abs(in.uv.y);
        if (manhattan > 1.0) {
            discard;
        }
        let glow = (1.0 - manhattan) * 2.0;
        let c = mix(vec4<f32>(1.0, 1.0, 1.0, 1.0), vec4<f32>(0.0, 0.9, 1.0, 1.0), manhattan);
        return c * glow;
    } else {
        // Boss: Premium rotating magical purple seal
        let glow = (1.0 - r) * 1.5;
        let angle = atan2(in.uv.y, in.uv.x) + uniforms.time * 1.5;
        let ring = abs(sin(r * 15.0 - uniforms.time * 5.0));
        let spokes = abs(sin(angle * 8.0));
        let pattern = smoothstep(0.4, 0.6, ring * spokes + 0.1);
        let color = mix(vec4<f32>(0.8, 0.2, 1.0, 1.0), vec4<f32>(1.0, 0.4, 0.9, 1.0), r);
        return color * pattern * glow;
    }
}
