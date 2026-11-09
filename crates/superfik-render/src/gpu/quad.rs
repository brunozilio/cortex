//! Pipeline and instance buffer for rounded quads (see `shaders/quad.wgsl`).

use std::ops::Range;

use bytemuck::{Pod, Zeroable};

use super::buffer::GrowableBuffer;
use super::globals::blend_target;
use crate::scene::{Layer, Quad};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuQuad {
    rect: [f32; 4],
    top: [f32; 4],
    bottom: [f32; 4],
    border: [f32; 4],
    inset_color: [f32; 4],
    shadow_color: [f32; 4],
    params: [f32; 4],
    shadows: [f32; 4],
    extra: [f32; 4],
    clip: [f32; 4],
}

fn encode(q: &Quad) -> GpuQuad {
    let (pattern, value) = q.pattern.encode();
    GpuQuad {
        rect: q.rect.to_array(),
        top: q.fill.top.to_array(),
        bottom: q.fill.bottom.to_array(),
        border: q.border.to_array(),
        inset_color: q.inset.color.to_array(),
        shadow_color: q.shadow.color.to_array(),
        params: [q.radius, q.border_width, q.opacity, pattern],
        shadows: [q.inset.offset_y, q.inset.blur, q.shadow.offset_y, q.shadow.blur],
        extra: [value, 0.0, 0.0, 0.0],
        clip: q.clip.to_array(),
    }
}

const ATTRIBUTES: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
    5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4,
];

#[derive(Debug)]
pub(crate) struct QuadPipeline {
    pipeline: wgpu::RenderPipeline,
    instances: GrowableBuffer,
    ranges: Vec<Range<u32>>,
}

impl QuadPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, globals: &wgpu::BindGroupLayout) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/quad.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            bind_group_layouts: &[Some(globals)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &blend_target(format),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            instances: GrowableBuffer::new(device, "quads", wgpu::BufferUsages::VERTEX),
            ranges: Vec::new(),
        }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, layers: &[Layer]) {
        let mut quads = Vec::new();
        self.ranges.clear();
        for layer in layers {
            let start = quads.len() as u32;
            quads.extend(layer.quads.iter().filter(|q| q.opacity > 0.0).map(encode));
            self.ranges.push(start..quads.len() as u32);
        }
        self.instances.write(device, queue, bytemuck::cast_slice(&quads));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, globals: &wgpu::BindGroup, index: usize) {
        let Some(range) = self.ranges.get(index).filter(|r| !r.is_empty()) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_vertex_buffer(0, self.instances.buffer().slice(..));
        pass.draw(0..6, range.clone());
    }
}
