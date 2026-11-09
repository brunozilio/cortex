//! Browser cards' pages on the server (ADR 0010).
//!
//! [`pages`] splits the work in two: [`ServerPages`] is the backend's
//! `PageService`, cheap to call from any thread, and [`PageLoop`] runs the
//! browser where it must run (CEF: the process's main thread). Each round
//! it takes the calls clients sent (only who opened a page drives it,
//! [`Owners`]), pumps the browser, sends each page's reports to its owner
//! and, when the page painted, reads its frame back from the GPU
//! ([`Gpu`]) and sends what changed ([`superfik_frames::PageEncoder`]).

pub mod egress;
mod owners;
mod page_loop;
mod readback;

pub use owners::{Admitted, Owners, profile_of};
pub use page_loop::{PageLoop, Request, ServerPages, pages};
pub use readback::{Gpu, ReadPixels};
