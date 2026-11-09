//! A page's frame from the GPU into memory. The page's texture is drawn
//! into one of ours (it may not allow copies, and may be BGRA or sRGB),
//! which is copied into a buffer and mapped: straight RGBA, rows padded to
//! the GPU's copy alignment.

use superfik_browser::Frame;

const SHADER: &str = r"
@group(0) @binding(0) var page: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(page, vec2<i32>(at.xy), 0);
}
";

#[derive(Debug)]
pub struct ReadPixels {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
}

#[derive(Debug)]
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

impl Gpu {
    pub fn headless() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let descriptor = wgpu::DeviceDescriptor::default();
        let (device, queue) = match superfik_browser::open_device(&adapter, &descriptor) {
            Some(opened) => opened,
            None => pollster::block_on(adapter.request_device(&descriptor)).ok()?,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("page readback"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        Some(Self { device, queue, layout, pipelines: Vec::new() })
    }

    pub fn read(&mut self, frame: &Frame) -> Option<ReadPixels> {
        let (width, height) = frame.size;
        if width == 0 || height == 0 {
            return None;
        }
        let format =
            if frame.format.is_srgb() { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
        let pipeline = self.pipeline(format).clone();
        let device = &self.device;
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("page readback"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("page readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("page readback"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&frame.view) }],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let view = target.create_view(&wgpu::TextureViewDescriptor::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("page readback"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None },
            },
            size,
        );
        self.queue.submit([encoder.finish()]);
        let (sent, mapped) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = sent.send(result);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        mapped.recv().ok()?.ok()?;
        let data = buffer.slice(..).get_mapped_range().ok()?.to_vec();
        buffer.unmap();
        Some(ReadPixels { data, width, height, stride: stride as usize })
    }

    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        if let Some(index) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[index].1;
        }
        let device = &self.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("page readback"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("page readback"),
            bind_group_layouts: &[Some(&self.layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("page readback"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, pipeline));
        &self.pipelines[self.pipelines.len() - 1].1
    }
}
