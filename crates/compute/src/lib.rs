use shared::{CollisionResult, MAX_BULLETS, MAX_PARTICLES, GRID_WIDTH, GRID_HEIGHT};
use std::sync::Arc;

pub struct ComputeContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    
    // Core Pipelines
    pub bullet_update_pipeline: wgpu::ComputePipeline,
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

    // Bind Groups (using buffers shared from RenderContext)
    pub compute_bind_group: wgpu::BindGroup,
    pub spatial_bind_group: wgpu::BindGroup,
    pub collision_bind_group: wgpu::BindGroup,
    pub particle_bind_group: wgpu::BindGroup,
}

impl ComputeContext {
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

        Self {
            device,
            queue,
            bullet_update_pipeline,
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
            compute_bind_group,
            spatial_bind_group,
            collision_bind_group,
            particle_bind_group,
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
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Bullet Update Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.bullet_update_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            let workgroups = (bullet_count + 63) / 64;
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
            let workgroups = (MAX_PARTICLES as u32 + 63) / 64;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // 3. Build Spatial Hash Grid
        if bullet_count > 0 {
            let total_cells = (GRID_WIDTH * GRID_HEIGHT) as u32;

            // Clear Grid Cell Counts
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Spatial Clear Grid Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.spatial_clear_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                compute_pass.set_bind_group(1, &self.spatial_bind_group, &[]);
                let workgroups = (total_cells + 63) / 64;
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
                let workgroups = (bullet_count + 63) / 64;
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
                let workgroups = (bullet_count + 63) / 64;
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

        // Copy collision results to CPU map-read buffer
        encoder.copy_buffer_to_buffer(
            &self.collision_result_buf,
            0,
            &self.collision_readback_buf,
            0,
            std::mem::size_of::<CollisionResult>() as u64,
        );

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub async fn readback_collisions(&self) -> Option<CollisionResult> {
        let buffer_slice = self.collision_readback_buf.slice(..);
        
        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Wait for WebGPU to complete the mapping operation
        self.device.poll(wgpu::Maintain::Wait);

        if let Ok(Ok(())) = receiver.await {
            let data = buffer_slice.get_mapped_range();
            let result: CollisionResult = *bytemuck::from_bytes(&data);
            drop(data);
            self.collision_readback_buf.unmap();
            Some(result)
        } else {
            None
        }
    }

    pub fn read_collisions_sync(&self) -> Option<CollisionResult> {
        let buffer_slice = self.collision_readback_buf.slice(..);
        
        let (sender, mut receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        if let Ok(Some(Ok(()))) = receiver.try_recv() {
            let data = buffer_slice.get_mapped_range();
            let result: CollisionResult = *bytemuck::from_bytes(&data);
            drop(data);
            self.collision_readback_buf.unmap();
            Some(result)
        } else {
            None
        }
    }
}

