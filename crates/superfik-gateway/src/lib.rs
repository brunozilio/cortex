//! Superfik's model gateway:Anthropic's Messages API on a
//! loopback port, so Claude Code runs with models other providers serve.
//! The translations are pure; the server and the upstreams are the IO.

pub mod chat;
pub mod local;
pub mod messages;
pub mod mlx;
pub mod server;
pub mod sse;
pub mod stream;
