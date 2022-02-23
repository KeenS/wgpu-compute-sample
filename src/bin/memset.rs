use pollster::FutureExt;
use wgpu::{Backends, BufferBinding, DeviceDescriptor, Instance, RequestAdapterOptions};

fn main() {
    env_logger::init();

    let shader = wgpu::ShaderSource::Wgsl(include_str!("../memset.wgsl").into());
    let instance = Instance::new(Backends::all());

    let (device, queue) = pollster::block_on(async {
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
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

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output data"),
        size: 512 * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Output bind group"),
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
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: "cs_main",
    });
    let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Output bind group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(BufferBinding {
                buffer: &output_buffer,
                offset: 0,
                size: Some(wgpu::BufferSize::new(512 * 4).unwrap()),
            }),
        }],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command encoder"),
    });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute pass"),
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &output_bind_group, &[]);
        compute_pass.dispatch(512, 1, 1);
    }
    queue.submit([encoder.finish()]);
    let buffer_slice = output_buffer.slice(..);
    let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
    device.poll(wgpu::Maintain::Wait);
    mapping.block_on().unwrap();
    let output_data: &[u8] = &buffer_slice.get_mapped_range();
    // danger
    let output_data: &[u32] = bytemuck::cast_slice(output_data);
    println!("{output_data:?}");
}
