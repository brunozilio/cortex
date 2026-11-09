//! Text shaping and layout on top of cosmic-text, with bundled Inter and
//! fallback faces for CJK (`fallback`).

mod fallback;
mod layout;
mod spans;
pub(crate) mod system;

pub use fallback::{FallbackFaces, NoFaces, Script, SystemFaces};
pub use layout::{Flow, StyleRun, TextLayout, TextLine, TextStyle};
pub use system::{FontFamily, FontWeight, TextSystem};

pub(super) fn windows_fonts_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    return std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(|root| std::path::PathBuf::from(root).join("Fonts"));
    #[cfg(not(target_os = "windows"))]
    None
}
