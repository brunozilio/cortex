//! Per-frame uniforms shared by every pipeline.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Globals {
    pub viewport: [f32; 2],
    pub scale: f32,
    pub time: f32,
    pub tones: [[f32; 4]; 6],
}

impl Globals {
    pub fn new(viewport: [f32; 2], scale: f32, time: f32, tones: &crate::PatternTones) -> Self {
        let rgba = |c: crate::Color| [c.r, c.g, c.b, c.a];
        let [a, b] = tones.meter_checker;
        Self {
            viewport,
            scale,
            time,
            tones: [
                rgba(tones.meter_lit),
                rgba(a),
                rgba(b),
                rgba(tones.ultra_off),
                rgba(tones.sparkle_ink),
                [tones.shimmer, 0.0, 0.0, 0.0],
            ],
        }
    }
}

pub(crate) fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("globals"),
        entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, false)],
    })
}

pub(crate) fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(crate) fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(crate) fn blend_target(format: wgpu::TextureFormat) -> [Option<wgpu::ColorTargetState>; 1] {
    [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })]
}
