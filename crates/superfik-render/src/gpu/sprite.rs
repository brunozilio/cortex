//! Pipeline and instance buffer for textured sprites (see `shaders/sprite.wgsl`).

use std::ops::Range;

use bytemuck::{Pod, Zeroable};

use super::buffer::GrowableBuffer;
use super::globals::blend_target;
use crate::scene::ImageId;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct SpriteInstance {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
    pub clip: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpriteKind {
    Mask,
    Color,
    Image(ImageId),
}

#[derive(Debug, Clone)]
pub(crate) struct SpriteBatch {
    pub kind: SpriteKind,
    pub range: Range<u32>,
}

const ATTRIBUTES: [wgpu::VertexAttribute; 5] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4];

#[derive(Debug)]
pub(crate) struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    instances: GrowableBuffer,
    staged: Vec<SpriteInstance>,
    batches: Vec<Vec<SpriteBatch>>,
}

impl SpritePipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals: &wgpu::BindGroupLayout,
        textures: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/sprite.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(globals), Some(textures)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteInstance>() as u64,
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
            instances: GrowableBuffer::new(device, "sprites", wgpu::BufferUsages::VERTEX),
            staged: Vec::new(),
            batches: Vec::new(),
        }
    }

    pub fn begin(&mut self) {
        self.staged.clear();
        self.batches.clear();
    }

    pub fn begin_layer(&mut self) {
        self.batches.push(Vec::new());
    }

    pub fn push(&mut self, kind: SpriteKind, sprite: SpriteInstance) {
        let index = self.staged.len() as u32;
        self.staged.push(sprite);
        let Some(batches) = self.batches.last_mut() else { return };
        match batches.last_mut() {
            Some(batch) if batch.kind == kind => batch.range.end = index + 1,
            _ => batches.push(SpriteBatch { kind, range: index..index + 1 }),
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.instances.write(device, queue, bytemuck::cast_slice(&self.staged));
    }

    pub fn draw<'a>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        globals: &wgpu::BindGroup,
        index: usize,
        texture: impl Fn(SpriteKind) -> Option<&'a wgpu::BindGroup>,
    ) {
        let Some(batches) = self.batches.get(index).filter(|b| !b.is_empty()) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_vertex_buffer(0, self.instances.buffer().slice(..));
        for batch in batches {
            if let Some(group) = texture(batch.kind) {
                pass.set_bind_group(1, group, &[]);
                pass.draw(0..6, batch.range.clone());
            }
        }
    }
}
