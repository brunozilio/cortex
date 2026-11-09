//! Atlas and image textures.

use std::collections::HashMap;

use crate::atlas::{AtlasSlot, ShelfPacker};
use crate::scene::ImageId;

pub(crate) fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn create_texture(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, slot: AtlasSlot, bytes_per_pixel: u32, data: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: slot.x, y: slot.y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(slot.w * bytes_per_pixel), rows_per_image: None },
        wgpu::Extent3d { width: slot.w, height: slot.h, depth_or_array_layers: 1 },
    );
}

#[derive(Debug)]
pub(crate) struct AtlasTexture {
    texture: wgpu::Texture,
    packer: ShelfPacker,
    bytes_per_pixel: u32,
    pub bind_group: wgpu::BindGroup,
}

impl AtlasTexture {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        size: u32,
        format: wgpu::TextureFormat,
        padding: u32,
    ) -> Self {
        let texture = create_texture(device, size, size, format);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bytes_per_pixel = format.block_copy_size(None).unwrap_or(4);
        Self {
            bind_group: bind(device, layout, &view, sampler),
            texture,
            packer: ShelfPacker::new(size, padding),
            bytes_per_pixel,
        }
    }

    pub fn size(&self) -> f32 {
        self.packer.size() as f32
    }

    pub fn insert(&mut self, queue: &wgpu::Queue, w: u32, h: u32, data: &[u8]) -> Option<AtlasSlot> {
        let slot = self.packer.allocate(w, h)?;
        if w > 0 && h > 0 {
            upload(queue, &self.texture, slot, self.bytes_per_pixel, data);
        }
        Some(slot)
    }

    pub fn clear(&mut self, queue: &wgpu::Queue) {
        self.packer.clear();
        let size = self.packer.size();
        let zeros = vec![0u8; (size * size * self.bytes_per_pixel) as usize];
        upload(queue, &self.texture, AtlasSlot { x: 0, y: 0, w: size, h: size }, self.bytes_per_pixel, &zeros);
    }
}

#[derive(Debug)]
pub(crate) struct ImageTextures {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    images: HashMap<ImageId, wgpu::BindGroup>,
    next: u32,
}

impl ImageTextures {
    pub fn new(layout: wgpu::BindGroupLayout, sampler: wgpu::Sampler) -> Self {
        Self { layout, sampler, images: HashMap::new(), next: 0 }
    }

    pub fn insert(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], w: u32, h: u32) -> ImageId {
        let (layout, sampler) = (&self.layout, &self.sampler);
        let texture = create_texture(device, w, h, wgpu::TextureFormat::Rgba8Unorm);
        upload(queue, &texture, AtlasSlot { x: 0, y: 0, w, h }, 4, rgba);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let id = ImageId(self.next);
        self.next += 1;
        self.images.insert(id, bind(device, layout, &view, sampler));
        id
    }

    pub fn show(&mut self, device: &wgpu::Device, id: Option<ImageId>, view: &wgpu::TextureView) -> ImageId {
        let id = id.unwrap_or_else(|| {
            let id = ImageId(self.next);
            self.next += 1;
            id
        });
        self.images.insert(id, bind(device, &self.layout, view, &self.sampler));
        id
    }

    pub fn remove(&mut self, id: ImageId) {
        self.images.remove(&id);
    }

    pub fn get(&self, id: ImageId) -> Option<&wgpu::BindGroup> {
        self.images.get(&id)
    }
}
