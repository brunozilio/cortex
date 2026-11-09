//! The Superfik web app (ADR 0010, `app.superfik.com`): the client the
//! desktop shares (`superfik-client`), painted on a WebGPU canvas.
//!
//! `WebClient` is the client on the page, pure and tested natively: keys,
//! the pointer and the wheel become actions by the desktop's own rules.
//! `page` (wasm only) is the browser around it: winit for input and size,
//! the GPU, the clipboard.

mod client;
pub mod dom_keys;
mod effects;
pub mod fonts;
mod picked;

pub use client::WebClient;
pub use effects::Io;

#[cfg(target_arch = "wasm32")]
mod files;
#[cfg(target_arch = "wasm32")]
mod launch;
#[cfg(target_arch = "wasm32")]
mod page;
#[cfg(target_arch = "wasm32")]
mod streamed;
#[cfg(target_arch = "wasm32")]
mod textarea;
