//! A gateway whose local provider is a Chat Completions server already
//! running: `cargo run -p superfik-gateway --example local -- <url> <model>`,
//! then point `claude` at the printed env.

use std::sync::Arc;

use superfik_core::ModelProvider;
use superfik_gateway::local::{ChatUpstream, Endpoint, Endpoints};
use superfik_gateway::server::{Gateway, UpstreamError, Upstreams};

struct Fixed(Endpoint);

impl Endpoints for Fixed {
    fn endpoint(&self, _: &str) -> Result<Endpoint, UpstreamError> {
        Ok(self.0.clone())
    }
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(base_url), Some(model)) = (args.next(), args.next()) else {
        eprintln!("usage: local <base url> <model>");
        std::process::exit(2);
    };
    let upstream = ChatUpstream::new(Arc::new(Fixed(Endpoint { base_url, model })));
    let gateway = Gateway::start(Upstreams::new().with(ModelProvider::Local, Arc::new(upstream)))?;
    let access = gateway.admit(ModelProvider::Local);
    println!("ANTHROPIC_BASE_URL={} ANTHROPIC_AUTH_TOKEN={}", access.base_url, access.token);
    loop {
        std::thread::park();
    }
}
