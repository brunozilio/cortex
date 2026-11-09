//! The frames browser pages show: each page's latest frame on the GPU,
//! skipping the stretched ones Chromium hands over while it resizes, and
//! the frame a resized page replaced, fading out over the new one so its
//! new layout does not snap in.

use std::collections::HashMap;
use std::time::Instant;

use superfik_browser::Frame;
use superfik_core::BrowserId;
use superfik_render::{ImageId, Renderer};
use superfik_ui::components::{FadingFrame, PageImage};
use superfik_ui::theme::motion;

pub fn settled(frame: (u32, u32), asked: (u32, u32)) -> bool {
    frame.0.abs_diff(asked.0) <= 2 && frame.1.abs_diff(asked.1) <= 2
}

pub fn fade(elapsed: f32) -> f32 {
    (1.0 - elapsed / motion::PAGE_CROSSFADE).clamp(0.0, 1.0)
}

struct Fading {
    image: ImageId,
    size: (f32, f32),
    since: Instant,
}

#[derive(Default)]
pub struct PageFrames {
    shown: HashMap<BrowserId, (ImageId, u64)>,
    ids: HashMap<BrowserId, PageImage>,
    fading: HashMap<BrowserId, Fading>,
}

impl PageFrames {
    pub fn show(
        &mut self,
        renderer: &mut Renderer,
        id: BrowserId,
        frame: &Frame,
        asked: (u32, u32),
        scale: f32,
    ) -> bool {
        let shown = self.shown.get(&id).copied();
        if shown.is_some_and(|(_, generation)| generation == frame.generation) {
            return false;
        }
        if shown.is_some() && !settled(frame.size, asked) {
            return false;
        }
        let size = (frame.size.0 as f32 / scale, frame.size.1 as f32 / scale);
        let resized = self.ids.get(&id).is_some_and(|page| !same_size(page.size, size));
        let image = match shown {
            Some((old, _)) if resized => {
                let old_size = self.ids.get(&id).map_or(size, |page| page.size);
                if let Some(previous) =
                    self.fading.insert(id, Fading { image: old, size: old_size, since: Instant::now() })
                {
                    renderer.release_image(previous.image);
                }
                renderer.show_texture(None, &frame.view)
            }
            _ => renderer.show_texture(shown.map(|(image, _)| image), &frame.view),
        };
        self.shown.insert(id, (image, frame.generation));
        self.ids.insert(id, PageImage { image, size, fading: None });
        tracing::trace!(%id, size = ?frame.size, generation = frame.generation, "page frame");
        true
    }

    pub fn tick(&mut self, renderer: &mut Renderer) -> bool {
        let now = Instant::now();
        let mut done = Vec::new();
        for (id, old) in &self.fading {
            let opacity = fade(now.duration_since(old.since).as_secs_f32());
            if let Some(page) = self.ids.get_mut(id) {
                page.fading = (opacity > 0.0).then_some(FadingFrame { image: old.image, size: old.size, opacity });
            }
            if opacity <= 0.0 {
                done.push(*id);
            }
        }
        for id in done {
            if let Some(old) = self.fading.remove(&id) {
                renderer.release_image(old.image);
            }
        }
        !self.fading.is_empty()
    }

    pub fn release_closed(&mut self, renderer: &mut Renderer, open: impl Fn(&BrowserId) -> bool) {
        let closed: Vec<BrowserId> = self.shown.keys().filter(|id| !open(id)).copied().collect();
        for id in closed {
            if let Some((image, _)) = self.shown.remove(&id) {
                renderer.release_image(image);
            }
            if let Some(old) = self.fading.remove(&id) {
                renderer.release_image(old.image);
            }
            self.ids.remove(&id);
        }
    }

    pub fn ids(&self) -> &HashMap<BrowserId, PageImage> {
        &self.ids
    }
}

fn same_size(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 2.0 && (a.1 - b.1).abs() < 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_of_another_size_than_asked_are_skipped() {
        assert!(settled((1320, 840), (1320, 840)));
        assert!(settled((1321, 839), (1320, 840)), "rounding");
        assert!(!settled((2800, 1700), (1320, 840)), "still the expanded size");
    }

    #[test]
    fn a_replaced_frame_fades_out() {
        assert_eq!(fade(0.0), 1.0);
        assert!((fade(motion::PAGE_CROSSFADE * 0.5) - 0.5).abs() < 1e-4);
        assert_eq!(fade(motion::PAGE_CROSSFADE * 2.0), 0.0);
        assert!(same_size((660.0, 420.0), (660.5, 419.0)));
        assert!(!same_size((660.0, 420.0), (1392.0, 836.0)));
    }
}
