use pollster::FutureExt;
use std::mem;
use wgpu::{
    util::DeviceExt, Backends, BufferBinding, DeviceDescriptor, Instance, RequestAdapterOptions,
};

const DATA_SIZE: usize = 256;

fn main() {
    env_logger::init();

    let a = 2.0f32;
    let x = (0..DATA_SIZE)
        .into_iter()
        .map(|i| i as f32)
        .collect::<Vec<f32>>();
    let y = (0..DATA_SIZE)
        .into_iter()
        .map(|i| (DATA_SIZE - i) as f32)
        .collect::<Vec<f32>>();

    let shader = wgpu::ShaderSource::Wgsl(include_str!("../saxpy.wgsl").into());
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
                        max_push_constant_size: mem::size_of::<f32>() as u32,
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

    let module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: shader,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("x data"),
        contents: bytemuck::cast_slice(&x),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::STORAGE,
    });

    let y_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("y data"),
        contents: bytemuck::cast_slice(&y),
        usage: wgpu::BufferUsages::MAP_WRITE
            | wgpu::BufferUsages::MAP_READ
            | wgpu::BufferUsages::STORAGE,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Compute bind group"),
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
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..4,
        }],
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: "cs_main",
    });
    let data_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Data bind group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &x_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(BufferBinding {
                    buffer: &y_buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command encoder"),
    });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute pass"),
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &data_bind_group, &[]);
        compute_pass.set_push_constants(0, bytemuck::bytes_of(&a));
        compute_pass.dispatch(DATA_SIZE as u32, 1, 1);
    }
    queue.submit([encoder.finish()]);
    let buffer_slice = y_buffer.slice(..);
    let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
    device.poll(wgpu::Maintain::Wait);
    mapping.block_on().unwrap();
    let output_data: &[u8] = &buffer_slice.get_mapped_range();
    // danger
    let output_data: &[f32] = bytemuck::cast_slice(output_data);
    println!("{output_data:?}");
}
