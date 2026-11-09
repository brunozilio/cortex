//! The desktop's clipboard connection, shared by fields, pages and effects.
//! Keep it alive: on X11 the application owns the data it copied. Wayland
//! uses the window's core data-device protocol, including on GNOME and Weston.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use winit::window::Window;

#[derive(Clone)]
pub struct Clipboard(Rc<RefCell<Backend>>);

enum Backend {
    Native(arboard::Clipboard),
    Unavailable(String),
    #[cfg(target_os = "linux")]
    Wayland(Wayland),
}

#[cfg(target_os = "linux")]
struct Wayland {
    clipboard: smithay_clipboard::Clipboard,
    _window: Arc<Window>,
}

impl std::fmt::Debug for Clipboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clipboard").finish_non_exhaustive()
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        let backend = match arboard::Clipboard::new() {
            Ok(clipboard) => Backend::Native(clipboard),
            Err(error) => Backend::Unavailable(error.to_string()),
        };
        Self(Rc::new(RefCell::new(backend)))
    }
}

impl Clipboard {
    pub fn attach(&self, window: Arc<Window>) {
        #[cfg(target_os = "linux")]
        {
            use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
            if let Ok(display) = window.display_handle()
                && let RawDisplayHandle::Wayland(display) = display.as_raw()
            {
                // SAFETY: the Arc<Window> below keeps this borrowed display
                #[allow(unsafe_code)]
                let clipboard = unsafe { smithay_clipboard::Clipboard::new(display.display.as_ptr()) };
                *self.0.borrow_mut() = Backend::Wayland(Wayland { clipboard, _window: window });
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = window;
    }

    pub fn get_text(&self) -> Result<String, String> {
        match &mut *self.0.borrow_mut() {
            Backend::Native(clipboard) => clipboard.get_text().map_err(|error| error.to_string()),
            Backend::Unavailable(error) => Err(error.clone()),
            #[cfg(target_os = "linux")]
            Backend::Wayland(wayland) => wayland.clipboard.load().map_err(|error| error.to_string()),
        }
    }

    pub fn set_text(&self, text: impl Into<String>) -> Result<(), String> {
        let text = text.into();
        match &mut *self.0.borrow_mut() {
            Backend::Native(clipboard) => clipboard.set_text(text).map_err(|error| error.to_string()),
            Backend::Unavailable(error) => Err(error.clone()),
            #[cfg(target_os = "linux")]
            Backend::Wayland(wayland) => {
                wayland.clipboard.store(text);
                Ok(())
            }
        }
    }
}
