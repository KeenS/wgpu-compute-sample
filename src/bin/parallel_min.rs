use pollster::FutureExt;
use std::mem;
use wgpu::{
    util::DeviceExt, Backends, BufferBinding, DeviceDescriptor, Instance, RequestAdapterOptions,
};

fn main() {
    env_logger::init();

    const NUM_SRC_ITEMS: u32 = 4096 * 4096;
    let mut src_ptr = Vec::<u32>::with_capacity(NUM_SRC_ITEMS as usize);
    src_ptr.resize(NUM_SRC_ITEMS as usize, 0);
    let a = 21341u32;
    let mut b = 23458u32;
    let mut min = u32::MAX;
    for cell in &mut src_ptr {
        b = a.wrapping_mul(b & 65535);
        *cell = b + (b >> 16);
        min = if *cell < min { *cell } else { min };
    }
    println!("min: {min}");
    let ws = 64u32;
    // 7 wavefronts per SIMD
    let compute_units = 30;
    let mut global_work_size: u32 = compute_units * 7 * ws;
    while ((NUM_SRC_ITEMS) / 4) % global_work_size != 0 {
        global_work_size += ws;
    }
    let local_work_size = ws;
    let num_groups = global_work_size / local_work_size;
    println!("global_work_size: {global_work_size}, num_groups: {num_groups}");

    let min_shader = wgpu::ShaderSource::Wgsl(include_str!("../parallel_min.wgsl").into());
    let reduce_shader =
        wgpu::ShaderSource::Wgsl(include_str!("../parallel_min_reduce.wgsl").into());
    let instance = Instance::new(Backends::all());

    let (device, queue) = pollster::block_on(async {
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: wgpu::Features::PUSH_CONSTANTS,
                    limits: wgpu::Limits {
                        max_push_constant_size: mem::size_of::<u32>() as u32,
                        ..wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await
            .unwrap();
        (device, queue)
    });

    let min_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: min_shader,
    });
    let reduce_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: reduce_shader,
    });

    let src_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("src"),
        contents: bytemuck::cast_slice(&src_ptr),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let dst_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dst"),
        size: num_groups as u64 * mem::size_of::<u32>() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let dbg_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dbg"),
        size: global_work_size as u64 * mem::size_of::<u32>() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let min_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Compute min bind group"),
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
        ],
    });
    let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Compute reduce bind group"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let min_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute min Pipeline Layout"),
        bind_group_layouts: &[&min_layout],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..4,
        }],
    });
    let reduce_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute reduce Pipeline Layout"),
        bind_group_layouts: &[&reduce_layout],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..4,
        }],
    });
    let min_compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute min Pipeline"),
        layout: Some(&min_pipeline_layout),
        module: &min_module,
        entry_point: "minp",
    });
    let reduce_compute_pipeline =
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute reduce Pipeline"),
            layout: Some(&reduce_pipeline_layout),
            module: &reduce_module,
            entry_point: "reduce",
        });
    let min_data_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("min Data bind group"),
        layout: &min_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &src_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &dst_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &dbg_buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });
    let reduce_data_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("reduce Data bind group"),
        layout: &reduce_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(BufferBinding {
                buffer: &dst_buffer,
                offset: 0,
                size: None,
            }),
        }],
    });

    let mut min_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("min Command encoder"),
    });
    {
        let mut compute_pass = min_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("min Compute pass"),
        });
        compute_pass.set_pipeline(&min_compute_pipeline);
        compute_pass.set_bind_group(0, &min_data_bind_group, &[]);
        compute_pass.set_push_constants(0, bytemuck::bytes_of(&NUM_SRC_ITEMS));
        compute_pass.dispatch(global_work_size as u32, 1, 1);
    }
    let mut reduce_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("reduce Command encoder"),
    });
    {
        let mut compute_pass = reduce_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("reguce Compute pass"),
        });
        compute_pass.set_pipeline(&reduce_compute_pipeline);
        compute_pass.set_bind_group(0, &reduce_data_bind_group, &[]);
        compute_pass.set_push_constants(0, bytemuck::bytes_of(&NUM_SRC_ITEMS));
    }
    queue.submit([min_encoder.finish(), reduce_encoder.finish()]);

    let buffer_slice = dst_buffer.slice(..);
    let debug_slice = dbg_buffer.slice(..);
    let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
    let debug_mapping = debug_slice.map_async(wgpu::MapMode::Read);
    device.poll(wgpu::Maintain::Wait);
    mapping.block_on().unwrap();
    debug_mapping.block_on().unwrap();
    let output_data: &[u8] = &buffer_slice.get_mapped_range();
    // danger
    let output: &[u32] = bytemuck::cast_slice(output_data);
    let debug_data: &[u8] = &debug_slice.get_mapped_range();
    // danger
    let debug: &[u32] = bytemuck::cast_slice(debug_data);
    println!(
        "{} groups, {} threads, count {}, stride {}, nitems {}",
        debug[0], debug[1], debug[2], debug[3], debug[4]
    );
    println!("{}", output[0]);
}
