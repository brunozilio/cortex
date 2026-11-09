//! A browser card's page as a client and a server speak of it: the input
//! it takes, the profile it browses in, the calls that drive it and what
//! it reports, and its frames.
//!
//! On the desktop in local mode the page runs in the app itself
//! (`superfik-browser`). Otherwise it runs on the server (ADR 0010): the
//! client sends [`PageCall`]s, the server answers with [`PageReport`]s and
//! streams [`PageFrame`]s. Pure.

mod cursor;
mod frame;
mod input;
mod profile;
mod remote;

pub use cursor::PageCursor;
pub use frame::{FrameTile, PageFrame, TileCodec};
pub use input::{EditCommand, KeyInput, KeyPhase, Modifiers, PageInput, PointerButton};
pub use profile::Profile;
pub use remote::{PageCall, PageReport};
