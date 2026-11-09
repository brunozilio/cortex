//! The Superfik design system.
//!
//! Components here are stateless: they receive props, paint into a
//! [`Painter`] and return the geometry of their interactive parts. They
//! hold no business logic and never talk to engines or backends; the app
//! decides what the props are and what a click on a returned rect means.

pub mod brands;
pub mod components;
pub mod emote;
pub mod icons;
pub mod markdown;
pub mod orbs;
pub mod painter;
pub mod text_edit;
pub mod theme;

pub use painter::{Elided, Painter, Plane};
pub use text_edit::TextEdit;
