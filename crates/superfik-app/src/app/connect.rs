//! Which server the app talks to: exactly one (ADR 0010). Alone, the one
//! embedded in this app (ADR 0005); signed in to a team's server from
//! Settings › Server (`server_join`), that one, which then runs everything.

use superfik_backend::{Backend, LocalServices, Wake};
use superfik_core::Principal;
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION};
use superfik_remote::RemoteBackend;
use superfik_server::Server;

pub struct Connected {
    pub device: Server,
    pub backend: Box<dyn Backend>,
}

pub fn backend(wake: Wake) -> Result<Connected, Box<dyn std::error::Error>> {
    let device = Server::start(LocalServices::detect())?;
    let backend = greeted(&device, None, wake)?;
    Ok(Connected { device, backend })
}

pub fn greeted(
    device: &Server,
    remote: Option<RemoteBackend>,
    wake: Wake,
) -> Result<Box<dyn Backend>, superfik_backend::BackendError> {
    let backend: Box<dyn Backend> = match remote {
        Some(remote) => Box::new(remote),
        None => Box::new(device.connect(Principal::LocalOwner, wake)),
    };
    backend.send(Envelope::local(ClientCommand::Hello { version: PROTOCOL_VERSION }))?;
    Ok(backend)
}
