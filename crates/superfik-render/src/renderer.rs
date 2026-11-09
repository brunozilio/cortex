//! Frame orchestration: turns a [`Scene`] into GPU work.

use std::collections::HashMap;
use std::path::Path;

use cosmic_text::{CacheKey, SwashCache, SwashContent};
use thiserror::Error;

use crate::atlas::AtlasSlot;
use crate::gpu::{
    AtlasTexture, Globals, GooPipeline, GpuContext, GpuError, ImageTextures, QuadPipeline, SpriteInstance, SpriteKind,
    SpritePipeline,
};
use crate::icons;
use crate::pictures::{Pictures, device_size};
use crate::scene::{GlyphSprite, IconSprite, ImageId, ImageSprite, PictureSprite, Scene};
use crate::text::TextSystem;

const ATLAS_SIZE: u32 = 2048;
const ATLAS_PADDING: u32 = 8;
const MIN_BLUR: f32 = 0.25;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error(transparent)]
    Gpu(#[from] GpuError),
    #[error("could not read the frame back from the GPU: {0}")]
    Readback(String),
    #[error("could not read image {path}: {message}")]
    Image { path: String, message: String },
}

#[derive(Debug, Clone, Copy)]
struct GlyphEntry {
    slot: AtlasSlot,
    left: i32,
    top: i32,
    color: bool,
}

pub struct Renderer {
    gpu: GpuContext,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    goo: GooPipeline,
    quads: QuadPipeline,
    sprites: SpritePipeline,
    mask_atlas: AtlasTexture,
    color_atlas: AtlasTexture,
    images: ImageTextures,
    glyphs: HashMap<CacheKey, Option<GlyphEntry>>,
    icon_slots: HashMap<(&'static str, u32, u32), AtlasSlot>,
    pictures: Pictures,
    atlas_cleared: bool,
    swash: SwashCache,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").field("glyphs", &self.glyphs.len()).finish()
    }
}

impl Renderer {
    pub fn new(gpu: GpuContext) -> Self {
        let device = &gpu.device;
        let format = gpu.format();
        let globals_layout = crate::gpu::globals_layout(device);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let texture_layout = crate::gpu::texture_layout(device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mask_atlas = AtlasTexture::new(
            device,
            &texture_layout,
            &sampler,
            ATLAS_SIZE,
            wgpu::TextureFormat::R8Unorm,
            ATLAS_PADDING,
        );
        let color_atlas =
            AtlasTexture::new(device, &texture_layout, &sampler, ATLAS_SIZE, wgpu::TextureFormat::Rgba8Unorm, 2);
        Self {
            goo: GooPipeline::new(device, format),
            quads: QuadPipeline::new(device, format, &globals_layout),
            sprites: SpritePipeline::new(device, format, &globals_layout, &texture_layout),
            globals,
            globals_group,
            mask_atlas,
            color_atlas,
            images: ImageTextures::new(texture_layout.clone(), sampler.clone()),
            glyphs: HashMap::new(),
            icon_slots: HashMap::new(),
            pictures: Pictures::new(),
            atlas_cleared: false,
            swash: SwashCache::new(),
            gpu,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    pub fn load_image(&mut self, rgba: &[u8], w: u32, h: u32) -> ImageId {
        let gpu = &self.gpu;
        self.images.insert(&gpu.device, &gpu.queue, rgba, w, h)
    }

    pub fn show_texture(&mut self, id: Option<ImageId>, view: &wgpu::TextureView) -> ImageId {
        self.images.show(&self.gpu.device, id, view)
    }

    pub fn release_image(&mut self, id: ImageId) {
        self.images.remove(id);
    }

    pub fn device(&self) -> (&wgpu::Device, &wgpu::Queue) {
        (&self.gpu.device, &self.gpu.queue)
    }

    pub fn load_image_file(&mut self, path: &Path, max_side: u32) -> Result<(ImageId, (u32, u32)), RenderError> {
        let error = |message: String| RenderError::Image { path: path.display().to_string(), message };
        let image = image::open(path).map_err(|e| error(e.to_string()))?;
        let max_side = max_side.min(self.gpu.device.limits().max_texture_dimension_2d);
        let image =
            if image.width().max(image.height()) > max_side { image.thumbnail(max_side, max_side) } else { image };
        let image = image.to_rgba8();
        let (w, h) = image.dimensions();
        Ok((self.load_image(image.as_raw(), w, h), (w, h)))
    }

    pub fn render(&mut self, scene: &Scene, text: &mut TextSystem, scale: f32, time: f32) -> Result<(), RenderError> {
        self.render_passes(&[(scene, scale)], text, time)
    }

    pub fn render_passes(
        &mut self,
        passes: &[(&Scene, f32)],
        text: &mut TextSystem,
        time: f32,
    ) -> Result<(), RenderError> {
        debug_assert!(!passes.is_empty(), "a frame needs at least one pass");
        let Some(surface) = &self.gpu.surface else { return Ok(()) };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let (w, h) = (self.gpu.config.width, self.gpu.config.height);
                self.gpu.resize(w, h);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_passes(&view, passes, text, time);
        self.gpu.queue.present(frame);
        Ok(())
    }

    pub fn capture(
        &mut self,
        scene: &Scene,
        text: &mut TextSystem,
        scale: f32,
        time: f32,
    ) -> Result<Vec<u8>, RenderError> {
        self.capture_passes(&[(scene, scale)], text, time)
    }

    pub fn capture_passes(
        &mut self,
        passes: &[(&Scene, f32)],
        text: &mut TextSystem,
        time: f32,
    ) -> Result<Vec<u8>, RenderError> {
        debug_assert!(!passes.is_empty(), "a capture needs at least one pass");
        let (w, h) = (self.gpu.config.width, self.gpu.config.height);
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_passes(&view, passes, text, time);
        let mut encoder =
            self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().map_err(|e| RenderError::Readback(format!("{e:?}")))?;
        let bgra = matches!(self.gpu.format(), wgpu::TextureFormat::Bgra8Unorm);
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let line = &data[y * row as usize..y * row as usize + w as usize * 4];
            for px in line.as_chunks::<4>().0 {
                if bgra {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]])
                } else {
                    pixels.extend_from_slice(px)
                }
            }
        }
        Ok(pixels)
    }

    fn draw_passes(&mut self, view: &wgpu::TextureView, passes: &[(&Scene, f32)], text: &mut TextSystem, time: f32) {
        for (index, (scene, scale)) in passes.iter().enumerate() {
            let load = if index == 0 { wgpu::LoadOp::Clear(scene.background.to_wgpu()) } else { wgpu::LoadOp::Load };
            let encoder = self.encode(view, scene, text, *scale, time, load);
            self.gpu.queue.submit([encoder.finish()]);
        }
    }

    fn encode(
        &mut self,
        view: &wgpu::TextureView,
        scene: &Scene,
        text: &mut TextSystem,
        scale: f32,
        time: f32,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> wgpu::CommandEncoder {
        let viewport = [self.gpu.config.width as f32 / scale, self.gpu.config.height as f32 / scale];
        let globals = Globals::new(viewport, scale, time, &scene.pattern_tones);
        self.gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        self.goo.prepare(&self.gpu.device, &self.gpu.queue, &self.globals, &scene.layers);
        self.quads.prepare(&self.gpu.device, &self.gpu.queue, &scene.layers);
        self.stage_sprites(scene, text, scale);
        for stale in self.pictures.sweep() {
            self.images.remove(stale);
        }
        self.sprites.upload(&self.gpu.device, &self.gpu.queue);

        let mut encoder =
            self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for index in 0..scene.layers.len() {
                self.goo.draw(&mut pass, index);
                self.quads.draw(&mut pass, &self.globals_group, index);
                let (mask, color, images) = (&self.mask_atlas, &self.color_atlas, &self.images);
                self.sprites.draw(&mut pass, &self.globals_group, index, |kind| match kind {
                    SpriteKind::Mask => Some(&mask.bind_group),
                    SpriteKind::Color => Some(&color.bind_group),
                    SpriteKind::Image(id) => images.get(id),
                });
            }
        }
        encoder
    }

    fn stage_sprites(&mut self, scene: &Scene, text: &mut TextSystem, scale: f32) {
        self.atlas_cleared = false;
        self.stage_layers(scene, text, scale);
        if self.atlas_cleared {
            self.atlas_cleared = false;
            self.stage_layers(scene, text, scale);
        }
    }

    fn stage_layers(&mut self, scene: &Scene, text: &mut TextSystem, scale: f32) {
        self.sprites.begin();
        for layer in &scene.layers {
            self.sprites.begin_layer();
            for image in &layer.images {
                self.push_image(image);
            }
            for picture in &layer.pictures {
                self.push_picture(picture, scale);
            }
            for icon in &layer.icons {
                self.push_icon(icon, scale);
            }
            for glyph in &layer.glyphs {
                self.push_glyph(glyph, text, scale);
            }
        }
    }

    fn push_image(&mut self, image: &ImageSprite) {
        let sprite = SpriteInstance {
            rect: image.rect.to_array(),
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, image.opacity],
            params: [2.0, image.blur, image.radius, 0.0],
            clip: image.clip.to_array(),
        };
        self.sprites.push(SpriteKind::Image(image.image), sprite);
    }

    fn push_picture(&mut self, sprite: &PictureSprite, scale: f32) {
        let size = device_size(sprite.rect.w, sprite.rect.h, scale);
        let (images, gpu) = (&mut self.images, &self.gpu);
        let upload = |rgba: &[u8], w, h| images.insert(&gpu.device, &gpu.queue, rgba, w, h);
        let Some(image) = self.pictures.image(&sprite.picture, size, upload) else { return };
        let PictureSprite { rect, radius, opacity, clip, .. } = *sprite;
        self.push_image(&ImageSprite { image, rect, radius, opacity, blur: 0.0, clip });
    }

    fn push_icon(&mut self, icon: &IconSprite, scale: f32) {
        let w = (icon.rect.w * scale).round().max(1.0) as u32;
        let h = (icon.rect.h * scale).round().max(1.0) as u32;
        let key = (icon.icon.name, w, h);
        let slot = match self.icon_slots.get(&key) {
            Some(slot) => *slot,
            None => {
                let Some(pixels) = icons::rasterize(&icon.icon, w, h) else { return };
                let Some(slot) = self.color_atlas.insert(&self.gpu.queue, w, h, &pixels) else {
                    self.color_atlas.clear(&self.gpu.queue);
                    self.atlas_cleared = true;
                    self.icon_slots.clear();
                    self.glyphs.retain(|_, entry| entry.is_some_and(|e| !e.color));
                    return;
                };
                self.icon_slots.insert(key, slot);
                slot
            }
        };
        let size = self.color_atlas.size();
        let color = icon.tint.unwrap_or(crate::Color::WHITE);
        let sprite = SpriteInstance {
            rect: icon.rect.to_array(),
            uv: [slot.x as f32 / size, slot.y as f32 / size, slot.w as f32 / size, slot.h as f32 / size],
            color: [color.r, color.g, color.b, color.a * icon.opacity],
            params: [1.0, 0.0, 0.0, if icon.tint.is_some() { 1.0 } else { 0.0 }],
            clip: icon.clip.to_array(),
        };
        self.sprites.push(SpriteKind::Color, sprite);
    }

    fn glyph_entry(&mut self, key: CacheKey, text: &mut TextSystem) -> Option<GlyphEntry> {
        if let Some(entry) = self.glyphs.get(&key) {
            return *entry;
        }
        let image = self.swash.get_image_uncached(text.fonts_mut(), key)?;
        let (w, h) = (image.placement.width, image.placement.height);
        let color = matches!(image.content, SwashContent::Color);
        let atlas = if color { &mut self.color_atlas } else { &mut self.mask_atlas };
        let data: Vec<u8> = match image.content {
            SwashContent::SubpixelMask => image.data.chunks(4).map(|px| px[1]).collect(),
            _ => image.data.clone(),
        };
        let entry = match atlas.insert(&self.gpu.queue, w, h, &data) {
            Some(slot) => Some(GlyphEntry { slot, left: image.placement.left, top: image.placement.top, color }),
            None => {
                atlas.clear(&self.gpu.queue);
                self.atlas_cleared = true;
                self.glyphs.clear();
                if color {
                    self.icon_slots.clear();
                }
                return None;
            }
        };
        self.glyphs.insert(key, entry);
        entry
    }

    fn push_glyph(&mut self, glyph: &GlyphSprite, text: &mut TextSystem, scale: f32) {
        let Some(entry) = self.glyph_entry(glyph.key, text) else { return };
        if entry.slot.w == 0 || entry.slot.h == 0 {
            return;
        }
        let blur = (glyph.blur * scale).min(ATLAS_PADDING as f32);
        let pad = if blur < MIN_BLUR { 0.0 } else { blur.ceil() };
        let x = (glyph.x + entry.left) as f32 - pad;
        let y = (glyph.y - entry.top) as f32 - pad;
        let (w, h) = (entry.slot.w as f32 + pad * 2.0, entry.slot.h as f32 + pad * 2.0);
        let atlas = if entry.color { &self.color_atlas } else { &self.mask_atlas };
        let size = atlas.size();
        let sprite = SpriteInstance {
            rect: [x / scale, y / scale, w / scale, h / scale],
            uv: [(entry.slot.x as f32 - pad) / size, (entry.slot.y as f32 - pad) / size, w / size, h / size],
            color: glyph.color.to_array(),
            params: [if entry.color { 1.0 } else { 0.0 }, blur, 0.0, 0.0],
            clip: glyph.clip.to_array(),
        };
        let kind = if entry.color { SpriteKind::Color } else { SpriteKind::Mask };
        self.sprites.push(kind, sprite);
    }
}
