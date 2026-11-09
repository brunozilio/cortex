//! Pages streamed from the server, on the GPU: one texture per page, each
//! frame's tiles written where they go, shown to the renderer as an image.

use std::collections::HashMap;

use superfik_core::BrowserId;
use superfik_render::{ImageId, Renderer};
use superfik_ui::components::PageImage;

use crate::client::PagePaint;

struct Streamed {
    texture: wgpu::Texture,
    image: ImageId,
    size: (u32, u32),
}

#[derive(Default)]
pub(crate) struct StreamedPages {
    pages: HashMap<BrowserId, Streamed>,
    images: HashMap<BrowserId, PageImage>,
}

impl StreamedPages {
    pub fn paint(&mut self, renderer: &mut Renderer, paint: PagePaint, scale: f32) -> bool {
        let (w, h) = paint.size;
        let fits = self.pages.get(&paint.id).is_some_and(|page| page.size == paint.size);
        if !fits {
            if !paint.whole {
                return false;
            }
            let (device, _) = renderer.device();
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("streamed page"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let image = renderer.show_texture(self.pages.get(&paint.id).map(|page| page.image), &view);
            self.pages.insert(paint.id, Streamed { texture, image, size: paint.size });
        }
        let Some(page) = self.pages.get(&paint.id) else { return false };
        let (_, queue) = renderer.device();
        for (x, y, tile) in &paint.tiles {
            if x + tile.w > w || y + tile.h > h {
                continue;
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &page.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: *x, y: *y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &tile.rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(tile.w * 4), rows_per_image: None },
                wgpu::Extent3d { width: tile.w, height: tile.h, depth_or_array_layers: 1 },
            );
        }
        let size = (w as f32 / scale, h as f32 / scale);
        self.images.insert(paint.id, PageImage { image: page.image, size, fading: None });
        true
    }

    pub fn retain(&mut self, renderer: &mut Renderer, open: impl Fn(BrowserId) -> bool) {
        let closed: Vec<BrowserId> = self.pages.keys().copied().filter(|id| !open(*id)).collect();
        for id in closed {
            if let Some(page) = self.pages.remove(&id) {
                renderer.release_image(page.image);
            }
            self.images.remove(&id);
        }
    }

    pub fn images(&self) -> &HashMap<BrowserId, PageImage> {
        &self.images
    }
}
