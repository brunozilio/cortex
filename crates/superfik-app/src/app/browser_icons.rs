//! The browsers' own icons for Settings › Browser: turned into PNGs once,
//! off the main thread, under the app's data folder, then loaded on the
//! GPU.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use superfik_browser::import::BrowserKind;
use superfik_render::{ImageId, Renderer};

const PNG_SIZE: u32 = 128;

#[derive(Default)]
pub struct BrowserIcons {
    pending: Option<Receiver<Vec<(BrowserKind, PathBuf)>>>,
    ids: HashMap<BrowserKind, ImageId>,
}

impl BrowserIcons {
    pub fn start(kinds: Vec<BrowserKind>, dir: PathBuf) -> Self {
        let (reply, pending) = std::sync::mpsc::channel();
        let made = std::thread::Builder::new().name("superfik-browser-icons".into()).spawn(move || {
            let _ = reply.send(make(&kinds, &dir));
        });
        if let Err(error) = made {
            tracing::warn!(%error, "browser icons not made");
        }
        Self { pending: Some(pending), ids: HashMap::new() }
    }

    pub fn load(kinds: Vec<BrowserKind>, dir: PathBuf, renderer: &mut Renderer) -> Self {
        let (reply, pending) = std::sync::mpsc::channel();
        let _ = reply.send(make(&kinds, &dir));
        let mut icons = Self { pending: Some(pending), ids: HashMap::new() };
        icons.poll(renderer);
        icons
    }

    pub fn poll(&mut self, renderer: &mut Renderer) -> bool {
        let Some(made) = self.pending.as_ref().and_then(|pending| pending.try_recv().ok()) else { return false };
        self.pending = None;
        for (kind, png) in made {
            match renderer.load_image_file(&png, PNG_SIZE) {
                Ok((image, _)) => {
                    self.ids.insert(kind, image);
                }
                Err(error) => tracing::warn!(browser = kind.name(), %error, "browser icon not loaded"),
            }
        }
        true
    }

    pub fn ids(&self) -> &HashMap<BrowserKind, ImageId> {
        &self.ids
    }
}

#[cfg(target_os = "macos")]
fn make(kinds: &[BrowserKind], dir: &std::path::Path) -> Vec<(BrowserKind, PathBuf)> {
    let Some(home) = std::env::home_dir() else { return Vec::new() };
    if let Err(error) = std::fs::create_dir_all(dir) {
        tracing::warn!(%error, "no folder for browser icons");
        return Vec::new();
    }
    kinds
        .iter()
        .filter_map(|&kind| {
            let png = dir.join(format!("{}.png", kind.name().to_lowercase()));
            if !png.exists() {
                let app = kind.app(&home)?;
                if let Err(error) = superfik_browser::import::icon::icon_png(&app, PNG_SIZE, &png) {
                    tracing::warn!(browser = kind.name(), %error, "browser icon not made");
                    return None;
                }
            }
            Some((kind, png))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn make(kinds: &[BrowserKind], _dir: &std::path::Path) -> Vec<(BrowserKind, PathBuf)> {
    let roots =
        [PathBuf::from("/usr/share/icons/hicolor"), PathBuf::from("/snap/chromium/current/usr/share/icons/hicolor")];
    kinds.iter().filter_map(|&kind| Some((kind, hicolor_icon(kind, &roots, |path| path.exists())?))).collect()
}

#[cfg(any(target_os = "linux", test))]
fn hicolor_icon(kind: BrowserKind, roots: &[PathBuf], exists: impl Fn(&std::path::Path) -> bool) -> Option<PathBuf> {
    let name = match kind {
        BrowserKind::Chrome => "google-chrome",
        BrowserKind::Chromium => "chromium",
        BrowserKind::Brave => "brave-browser",
        BrowserKind::Edge => "microsoft-edge",
        BrowserKind::Vivaldi => "vivaldi",
        BrowserKind::Dia | BrowserKind::Arc => return None,
    };
    let sizes = ["256x256", "128x128", "96x96", "64x64", "48x48"];
    roots
        .iter()
        .flat_map(|root| sizes.iter().map(move |size| root.join(size).join("apps").join(format!("{name}.png"))))
        .find(|path| exists(path))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn make(_kinds: &[BrowserKind], _dir: &std::path::Path) -> Vec<(BrowserKind, PathBuf)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_takes_the_largest_hicolor_icon_up_to_256() {
        let roots = [PathBuf::from("/usr/share/icons/hicolor")];
        let have = |path: &std::path::Path| {
            path.ends_with("128x128/apps/google-chrome.png") || path.ends_with("48x48/apps/google-chrome.png")
        };
        let found = hicolor_icon(BrowserKind::Chrome, &roots, have);
        assert_eq!(found, Some(PathBuf::from("/usr/share/icons/hicolor/128x128/apps/google-chrome.png")));
        assert_eq!(hicolor_icon(BrowserKind::Arc, &roots, |_| true), None, "no Linux build");
        assert_eq!(hicolor_icon(BrowserKind::Brave, &roots, have), None, "not installed");
    }
}
