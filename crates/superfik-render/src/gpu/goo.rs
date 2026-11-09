//! Pipeline and buffers for goo surfaces (see `shaders/goo.wgsl`).

use std::ops::Range;

use bytemuck::{Pod, Zeroable};

use super::buffer::GrowableBuffer;
use super::globals::{blend_target, storage_entry, uniform_entry};
use crate::scene::goo::neighbor_lists;
use crate::scene::{GooPrim, GooShape, GooStyle, Layer};

const LIGHTING_STRIDE: u64 = 256;
const GHOST: u32 = 1;
const LINE: u32 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuPrim {
    kind: u32,
    flags: u32,
    neighbor_start: u32,
    neighbor_count: u32,
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    reach: [f32; 4],
    top: [f32; 4],
    bottom: [f32; 4],
    style: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuLighting {
    highlight: [f32; 4],
    shadow: [f32; 4],
    params: [f32; 4],
    _pad: [f32; 52],
}

fn encode(prim: &GooPrim, neighbor_start: u32, neighbor_count: u32) -> GpuPrim {
    let half = |r: crate::Rect| {
        let c = r.center();
        [c.x, c.y, r.w * 0.5, r.h * 0.5]
    };
    let (kind, a, b, c) = match prim.shape {
        GooShape::RoundRect { rect, radius } => (0, half(rect), [radius, 0.0, 0.0, 0.0], [0.0; 4]),
        GooShape::Capsule { a, b, radius } => (1, [a.x, a.y, b.x, b.y], [radius, 0.0, 0.0, 0.0], [0.0; 4]),
        GooShape::Arc { center, radius, start, sweep, thickness } => {
            (2, [center.x, center.y, radius, thickness], [start, sweep, 0.0, 0.0], [0.0; 4])
        }
        GooShape::Circle { center, radius } => (3, [center.x, center.y, radius, 0.0], [0.0; 4], [0.0; 4]),
        GooShape::Notched { rect, radius, notch, notch_radius } => {
            (4, half(rect), [radius, notch_radius, radius * 0.8, 0.0], half(notch))
        }
    };
    let bounds = prim.shape.bounds();
    GpuPrim {
        kind,
        flags: if prim.ghost { GHOST } else { 0 } | if prim.line { LINE } else { 0 },
        neighbor_start,
        neighbor_count,
        a,
        b,
        c,
        reach: prim.reach().to_array(),
        top: prim.top.to_array(),
        bottom: prim.bottom.to_array(),
        style: [prim.smooth, prim.opacity, bounds.y, bounds.h],
    }
}

fn lighting(style: &GooStyle) -> GpuLighting {
    GpuLighting {
        highlight: style.highlight.to_array(),
        shadow: style.shadow.to_array(),
        params: [style.shadow_offset, style.shadow_blur, 0.0, 0.0],
        _pad: [0.0; 52],
    }
}

#[derive(Debug)]
pub(crate) struct GooPipeline {
    fill: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    prims: GrowableBuffer,
    neighbors: GrowableBuffer,
    lights: GrowableBuffer,
    bind_group: Option<wgpu::BindGroup>,
    ranges: Vec<Range<u32>>,
}

impl GooPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("goo"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, false),
                storage_entry(1),
                storage_entry(2),
                uniform_entry(3, wgpu::ShaderStages::VERTEX_FRAGMENT, true),
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/goo.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("goo"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |vs: &str, fs: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &blend_target(format),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        Self {
            fill: make("vs_fill", "fs_fill"),
            shadow: make("vs_shadow", "fs_shadow"),
            prims: GrowableBuffer::new(device, "goo prims", storage),
            neighbors: GrowableBuffer::new(device, "goo neighbors", storage),
            lights: GrowableBuffer::new(device, "goo lighting", wgpu::BufferUsages::UNIFORM),
            layout,
            bind_group: None,
            ranges: Vec::new(),
        }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, globals: &wgpu::Buffer, layers: &[Layer]) {
        let mut prims = Vec::new();
        let mut neighbors: Vec<u32> = Vec::new();
        let mut lights = Vec::new();
        self.ranges.clear();
        for layer in layers {
            let base = prims.len() as u32;
            for (prim, list) in layer.goo.iter().zip(neighbor_lists(&layer.goo)) {
                let start = neighbors.len() as u32;
                neighbors.extend(list.iter().map(|j| j + base));
                prims.push(encode(prim, start, list.len() as u32));
            }
            self.ranges.push(base..prims.len() as u32);
            lights.push(lighting(&layer.goo_style));
        }
        let mut rebuild = self.bind_group.is_none();
        rebuild |= self.prims.write(device, queue, bytemuck::cast_slice(&prims));
        rebuild |= self.neighbors.write(device, queue, bytemuck::cast_slice(&neighbors));
        rebuild |= self.lights.write(device, queue, bytemuck::cast_slice(&lights));
        if rebuild {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("goo"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.prims.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.neighbors.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: self.lights.buffer(),
                            offset: 0,
                            size: wgpu::BufferSize::new(LIGHTING_STRIDE),
                        }),
                    },
                ],
            }));
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize) {
        let (Some(group), Some(range)) = (&self.bind_group, self.ranges.get(index)) else { return };
        if range.is_empty() {
            return;
        }
        let offset = (index as u64 * LIGHTING_STRIDE) as u32;
        for pipeline in [&self.shadow, &self.fill] {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[offset]);
            pass.draw(0..6, range.clone());
        }
    }
}
