//! Language servers (LSP): which server reads which file, from which
//! project root, and how its answers read for the agent. Pure: the engine
//! starts the processes and speaks the protocol.

pub mod answer;
pub mod diagnostics;
pub mod operation;
pub mod roots;
pub mod servers;
pub mod uri;

pub use operation::{Operation, Query};
pub use roots::{root_for, server_for};
pub use servers::{ServerConfig, ServerSpec, defaults, servers};
