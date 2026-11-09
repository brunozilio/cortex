//! The client's [`Backend`](superfik_protocol::Backend) over a WebSocket to
//! an organization's server (`superfik_server::net`, ADR 0010): the
//! desktop's [`RemoteBackend`] (a thread over tungstenite, the session
//! cookie set from the sign-in token) and, in the browser, [`WebBackend`]
//! (the page's own WebSocket; the cookie goes with it). Both reconnect on
//! their own, greet the server again and resend what waited.

#[cfg(not(target_arch = "wasm32"))]
mod link;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the browser's transport uses it; tested here"))]
mod queue;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{Credentials, RemoteBackend, RemoteError};
#[cfg(target_arch = "wasm32")]
pub use web::WebBackend;
