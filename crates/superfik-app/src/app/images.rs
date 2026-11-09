//! Loads reference images onto the GPU once and remembers their ids.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use superfik_render::Renderer;

use crate::state::is_image;
use crate::view::LoadedImage;

const THUMBNAIL: u32 = 256;
const FULL: u32 = 4096;

#[derive(Debug, Default)]
pub struct ImageCache {
    loaded: HashMap<PathBuf, LoadedImage>,
    failed: HashSet<PathBuf>,
    full: HashMap<PathBuf, LoadedImage>,
    full_failed: HashSet<PathBuf>,
}

impl ImageCache {
    pub fn ensure<'a>(&mut self, renderer: &mut Renderer, paths: impl IntoIterator<Item = (&'a Path, &'a Path)>) {
        for (path, source) in paths {
            if !is_image(path) || self.loaded.contains_key(path) || self.failed.contains(path) {
                continue;
            }
            load(renderer, path, source, THUMBNAIL, &mut self.loaded, &mut self.failed);
        }
    }

    pub fn ensure_full(&mut self, renderer: &mut Renderer, path: &Path, source: &Path) {
        if !is_image(path) || self.full.contains_key(path) || self.full_failed.contains(path) {
            return;
        }
        load(renderer, path, source, FULL, &mut self.full, &mut self.full_failed);
    }

    pub fn ids(&self) -> &HashMap<PathBuf, LoadedImage> {
        &self.loaded
    }

    pub fn full_ids(&self) -> &HashMap<PathBuf, LoadedImage> {
        &self.full
    }
}

fn load(
    renderer: &mut Renderer,
    path: &Path,
    source: &Path,
    max_side: u32,
    loaded: &mut HashMap<PathBuf, LoadedImage>,
    failed: &mut HashSet<PathBuf>,
) {
    match renderer.load_image_file(source, max_side) {
        Ok((id, (w, h))) => {
            let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };
            loaded.insert(path.to_path_buf(), LoadedImage { id, aspect });
        }
        Err(error) => {
            tracing::warn!(%error, "could not load reference image");
            failed.insert(path.to_path_buf());
        }
    }
}
