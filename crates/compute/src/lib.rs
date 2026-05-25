use shared::{CollisionResult, MAX_BULLETS, MAX_PARTICLES, GRID_WIDTH, GRID_HEIGHT};
use std::sync::Arc;

pub struct ComputeContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    
    // Core Pipelines
    pub bullet_update_pipeline: wgpu::ComputePipeline,
    pub clear_active_count_pipeline: wgpu::ComputePipeline,
    pub spatial_clear_pipeline: wgpu::ComputePipeline,
    pub spatial_count_pipeline: wgpu::ComputePipeline,
    pub spatial_prefix_pipeline: wgpu::ComputePipeline,
    pub spatial_sort_pipeline: wgpu::ComputePipeline,
    pub collision_clear_pipeline: wgpu::ComputePipeline,
    pub collision_detect_pipeline: wgpu::ComputePipeline,
    pub particle_update_pipeline: wgpu::ComputePipeline,

    // Spatial Hash Grid and Collision Buffers
    pub grid_count_buf: wgpu::Buffer,
    pub grid_offset_buf: wgpu::Buffer,
    pub grid_items_buf: wgpu::Buffer,
    pub grid_tracker_buf: wgpu::Buffer,
    
    pub collision_result_buf: wgpu::Buffer,
    pub collision_readback_buf: wgpu::Buffer,

    pub active_bullet_count_buf: wgpu::Buffer,
    pub active_bullet_count_readback_buf: wgpu::Buffer,
    pub last_frame_active_bullets: u32,

    pub grid_readback_buf: wgpu::Buffer,
    pub compute_bind_group: wgpu::BindGroup,
    pub spatial_bind_group: wgpu::BindGroup,
    pub collision_bind_group: wgpu::BindGroup,
    pub particle_bind_group: wgpu::BindGroup,
    pub has_timestamp_query: bool,
    pub last_frame_grid_max_bucket: u32,
    pub last_frame_grid_avg_bucket: f32,
    pub last_frame_compute_ms: f32,
    pub last_frame_collision_hits: u32,
    pub last_frame_collision_grazes: u32,

    // Grid readback async state machine
    grid_readback_pending: bool,
    grid_readback_receiver: Option<futures_channel::oneshot::Receiver<Result<(), wgpu::BufferAsyncError>>>,

    // Collision readback async state machine
    collision_readback_pending: bool,
    collision_readback_receiver: Option<futures_channel::oneshot::Receiver<Result<(), wgpu::BufferAsyncError>>>,

    // Active bullet count async readback state machine
    active_count_readback_pending: bool,
    active_count_readback_receiver: Option<futures_channel::oneshot::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    collision_readback_start_time: f64,
}

impl ComputeContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        bullet_pos_buf: &wgpu::Buffer,
        bullet_vel_buf: &wgpu::Buffer,
        bullet_accel_buf: &wgpu::Buffer,
        bullet_meta_buf: &wgpu::Buffer,
        bullet_typeinfo_buf: &wgpu::Buffer,
        bullet_seed_buf: &wgpu::Buffer,
        particle_buf: &wgpu::Buffer,
        uniform_buffer: &wgpu::Buffer,
    ) -> Self {
        // Compile Shaders
        let bullet_update_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Bullet Update Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/bullet_update.wgsl"))),
        });

        let spatial_hash_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Spatial Hash Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/spatial_hash.wgsl"))),
        });

        let collision_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Collision Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/collision.wgsl"))),
        });

        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/particle.wgsl"))),
        });

        // 1. Allocate Spatial Grid and Collision Result Buffers
        let total_cells = (GRID_WIDTH * GRID_HEIGHT) as u64;

        let grid_count_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Count Buffer"),
            size: total_cells * 4, // u32 cell count
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let grid_offset_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Offset Buffer"),
            size: total_cells * 4, // u32 offset
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let grid_items_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Items Buffer"),
            size: (MAX_BULLETS * 4) as u64, // Bullet indexes
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let grid_tracker_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Tracker Buffer"),
            size: total_cells * 4, // Temporary tracking atoms
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let collision_result_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Collision Result Buffer"),
            size: std::mem::size_of::<CollisionResult>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let collision_readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Collision Readback Buffer"),
            size: std::mem::size_of::<CollisionResult>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let grid_readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Readback Buffer"),
            size: (GRID_WIDTH * GRID_HEIGHT * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let active_bullet_count_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Active Bullet Count Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let active_bullet_count_readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Active Bullet Count Readback Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // 2. Set Up Bind Group Layouts
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bullet Update Compute Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let spatial_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Spatial Hashing Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let collision_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Collision Broadphase Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let particle_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Update Compute Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // 3. Create Bind Group Instances
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bullet Update Bind Group"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: bullet_pos_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: bullet_vel_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: bullet_accel_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: bullet_meta_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: bullet_typeinfo_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: bullet_seed_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: active_bullet_count_buf.as_entire_binding() },
            ],
        });

        let spatial_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Spatial Hash Bind Group"),
            layout: &spatial_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: grid_count_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: grid_offset_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: grid_items_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: grid_tracker_buf.as_entire_binding() },
            ],
        });

        let collision_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Collision Bind Group"),
            layout: &collision_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: grid_count_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: grid_offset_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: grid_items_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: collision_result_buf.as_entire_binding() },
            ],
        });

        let particle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Update Bind Group"),
            layout: &particle_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: particle_buf.as_entire_binding() },
            ],
        });

        // 4. Create Compute Pipelines
        let bullet_update_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bullet Update Pipeline Layout"),
            bind_group_layouts: &[&compute_layout],
            push_constant_ranges: &[],
        });

        let clear_active_count_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Clear Active Count Compute Pipeline"),
            layout: Some(&bullet_update_pipeline_layout),
            module: &bullet_update_shader,
            entry_point: "clear_active_count",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let bullet_update_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bullet Update Compute Pipeline"),
            layout: Some(&bullet_update_pipeline_layout),
            module: &bullet_update_shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let spatial_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Spatial Hashing Pipeline Layout"),
            bind_group_layouts: &[&compute_layout, &spatial_layout],
            push_constant_ranges: &[],
        });

        let spatial_clear_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Spatial Clear Compute Pipeline"),
            layout: Some(&spatial_pipeline_layout),
            module: &spatial_hash_shader,
            entry_point: "clear_grid",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let spatial_count_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Spatial Count Compute Pipeline"),
            layout: Some(&spatial_pipeline_layout),
            module: &spatial_hash_shader,
            entry_point: "count_grid",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let spatial_prefix_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Spatial Prefix Compute Pipeline"),
            layout: Some(&spatial_pipeline_layout),
            module: &spatial_hash_shader,
            entry_point: "prefix_sum",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let spatial_sort_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Spatial Sort Compute Pipeline"),
            layout: Some(&spatial_pipeline_layout),
            module: &spatial_hash_shader,
            entry_point: "sort_grid",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let collision_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Collision Pipeline Layout"),
            bind_group_layouts: &[&compute_layout, &collision_layout],
            push_constant_ranges: &[],
        });

        let collision_clear_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Collision Clear Compute Pipeline"),
            layout: Some(&collision_pipeline_layout),
            module: &collision_shader,
            entry_point: "clear_collision",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let collision_detect_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Collision Detect Compute Pipeline"),
            layout: Some(&collision_pipeline_layout),
            module: &collision_shader,
            entry_point: "detect_collision",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let particle_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[&particle_layout],
            push_constant_ranges: &[],
        });

        let particle_update_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particle Update Compute Pipeline"),
            layout: Some(&particle_pipeline_layout),
            module: &particle_shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let has_ts = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);

        Self {
            device,
            queue,
            bullet_update_pipeline,
            clear_active_count_pipeline,
            spatial_clear_pipeline,
            spatial_count_pipeline,
            spatial_prefix_pipeline,
            spatial_sort_pipeline,
            collision_clear_pipeline,
            collision_detect_pipeline,
            particle_update_pipeline,
            grid_count_buf,
            grid_offset_buf,
            grid_items_buf,
            grid_tracker_buf,
            collision_result_buf,
            collision_readback_buf,
            active_bullet_count_buf,
            active_bullet_count_readback_buf,
            last_frame_active_bullets: 0,
            grid_readback_buf,
            compute_bind_group,
            spatial_bind_group,
            collision_bind_group,
            particle_bind_group,
            has_timestamp_query: has_ts,
            last_frame_grid_max_bucket: 0,
            last_frame_grid_avg_bucket: 0.0,
            last_frame_compute_ms: 0.0,
            last_frame_collision_hits: 0,
            last_frame_collision_grazes: 0,
            grid_readback_pending: false,
            grid_readback_receiver: None,
            collision_readback_pending: false,
            collision_readback_receiver: None,
            active_count_readback_pending: false,
            active_count_readback_receiver: None,
            collision_readback_start_time: 0.0,
        }
    }

    pub fn execute_compute_pass(&self, bullet_count: u32) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU Compute Pass Encoder"),
            });

        // 1. Bullet Update Pass
        if bullet_count > 0 {
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Clear Active Count Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.clear_active_count_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }

            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Bullet Update Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.bullet_update_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            let workgroups = bullet_count.div_ceil(64);
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // 2. Particle Update Pass
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Particle Update Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.particle_update_pipeline);
            compute_pass.set_bind_group(0, &self.particle_bind_group, &[]);
            let workgroups = (MAX_PARTICLES as u32).div_ceil(64);
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // 3. Build Spatial Hash Grid
        if bullet_count > 0 {
            let total_cells = GRID_WIDTH * GRID_HEIGHT;

            // Clear Grid Cell Counts
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Spatial Clear Grid Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.spatial_clear_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.spatial_bind_group, &[]);
                let workgroups = total_cells.div_ceil(64);
                compute_pass.dispatch_workgroups(workgroups, 1, 1);
            }

            // Populate Grid Cell Counts
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Spatial Count Grid Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.spatial_count_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.spatial_bind_group, &[]);
                let workgroups = bullet_count.div_ceil(64);
                compute_pass.dispatch_workgroups(workgroups, 1, 1);
            }

            // Prefix Sum
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Spatial Prefix Sum Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.spatial_prefix_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.spatial_bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }

            // Sort & Populate Grid Items
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Spatial Sort Grid Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.spatial_sort_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.spatial_bind_group, &[]);
                let workgroups = bullet_count.div_ceil(64);
                compute_pass.dispatch_workgroups(workgroups, 1, 1);
            }
        }

        // 4. Collision Pass
        {
            // Clear collision result counters
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Collision Clear Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.collision_clear_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.collision_bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }

            // Detect player spatial hashing collisions
            if bullet_count > 0 {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Collision Detect Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.collision_detect_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.collision_bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }
        }

        // Copy collision results to CPU map-read buffer (skip if previous readback still in flight)
        if !self.collision_readback_pending {
            encoder.copy_buffer_to_buffer(
                &self.collision_result_buf,
                0,
                &self.collision_readback_buf,
                0,
                std::mem::size_of::<shared::CollisionResult>() as u64,
            );
        }

        if !self.active_count_readback_pending {
            encoder.copy_buffer_to_buffer(
                &self.active_bullet_count_buf,
                0,
                &self.active_bullet_count_readback_buf,
                0,
                4,
            );
        }

        // Skip grid_readback copy while a readback is in flight — otherwise
        // copy_buffer_to_buffer into a mapped buffer triggers wgpu validation errors.
        if !self.grid_readback_pending {
            let total_cells = (GRID_WIDTH * GRID_HEIGHT * 4) as u64;
            encoder.copy_buffer_to_buffer(
                &self.grid_count_buf,
                0,
                &self.grid_readback_buf,
                0,
                total_cells,
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Non-blocking grid stats sampler. Uses a pending-state machine so the main
    /// thread never blocks on GPU completion: if a previous readback is still in
    /// flight, we just check (without waiting) whether it's ready. Stats values
    /// remain at the last successful readback (1-2 frame staleness, fine for HUD).
    pub fn sample_grid_stats(&mut self) {
        // Advance the wgpu queue without blocking. This is what gives the
        // GPU a chance to surface map_async callbacks.
        self.device.poll(wgpu::Maintain::Poll);

        if !self.grid_readback_pending {
            // No request in flight — issue a new map_async. The copy_buffer_to_buffer
            // was already submitted at the end of execute_compute_pass for this frame.
            let buffer_slice = self.grid_readback_buf.slice(..);
            let (sender, receiver) = futures_channel::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            self.grid_readback_pending = true;
            self.grid_readback_receiver = Some(receiver);
            return;
        }

        // A request is in flight — see if it has completed without blocking.
        let received = if let Some(receiver) = self.grid_readback_receiver.as_mut() {
            matches!(receiver.try_recv(), Ok(Some(Ok(()))))
        } else {
            false
        };

        if !received {
            return;
        }

        // Buffer is now mapped — read, compute stats, unmap, clear pending state.
        {
            let buffer_slice = self.grid_readback_buf.slice(..);
            let data = buffer_slice.get_mapped_range();
            let cells: &[u32] = bytemuck::cast_slice(&data);

            let mut max_bucket: u32 = 0;
            let mut sum: u64 = 0;
            let mut occupied: u64 = 0;

            for &count in cells {
                if count > max_bucket {
                    max_bucket = count;
                }
                if count > 0 {
                    sum += count as u64;
                    occupied += 1;
                }
            }

            let avg = if occupied > 0 {
                sum as f32 / occupied as f32
            } else {
                0.0
            };

            self.last_frame_grid_max_bucket = max_bucket;
            self.last_frame_grid_avg_bucket = avg;
            // data is dropped at end of scope so unmap() below is safe.
        }
        self.grid_readback_buf.unmap();
        self.grid_readback_pending = false;
        self.grid_readback_receiver = None;
    }

    pub fn sample_active_count(&mut self) {
        self.device.poll(wgpu::Maintain::Poll);

        if !self.active_count_readback_pending {
            let buffer_slice = self.active_bullet_count_readback_buf.slice(..);
            let (sender, receiver) = futures_channel::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            self.active_count_readback_pending = true;
            self.active_count_readback_receiver = Some(receiver);
            return;
        }

        let received = if let Some(receiver) = self.active_count_readback_receiver.as_mut() {
            matches!(receiver.try_recv(), Ok(Some(Ok(()))))
        } else {
            false
        };

        if !received {
            return;
        }

        {
            let buffer_slice = self.active_bullet_count_readback_buf.slice(..);
            let data = buffer_slice.get_mapped_range();
            self.last_frame_active_bullets = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
        }
        self.active_bullet_count_readback_buf.unmap();
        self.active_count_readback_pending = false;
        self.active_count_readback_receiver = None;
    }

    /// Non-blocking collision result sampler. Returns Some(result) when the GPU
    /// has finished writing, None otherwise. Collision response is delayed 1-2
    /// frames, which is imperceptible for 3px hitboxes at 120fps on a 1280×960 canvas.
    pub fn sample_collisions(&mut self, now_ms: f64) -> Option<CollisionResult> {
        self.device.poll(wgpu::Maintain::Poll);

        if !self.collision_readback_pending {
            let buffer_slice = self.collision_readback_buf.slice(..);
            let (sender, receiver) = futures_channel::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            self.collision_readback_pending = true;
            self.collision_readback_receiver = Some(receiver);
            self.collision_readback_start_time = now_ms;
            return None;
        }

        let received = if let Some(receiver) = self.collision_readback_receiver.as_mut() {
            matches!(receiver.try_recv(), Ok(Some(Ok(()))))
        } else {
            false
        };

        if !received {
            return None;
        }

        let result = {
            let buffer_slice = self.collision_readback_buf.slice(..);
            let data = buffer_slice.get_mapped_range();
            *bytemuck::from_bytes(&data)
        };
        self.collision_readback_buf.unmap();
        self.collision_readback_pending = false;
        self.collision_readback_receiver = None;

        self.last_frame_compute_ms = (now_ms - self.collision_readback_start_time) as f32;

        Some(result)
    }
}

