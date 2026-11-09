//! The real local provider: `MlxRuntime` installing and starting
//! `mlx_lm.server` on demand for models under the app's models folder.
//! `cargo run -p superfik-gateway --example mlx -- <local data folder>`,
//! then point `claude` at the printed env with a downloaded model's id.

use std::path::PathBuf;
use std::sync::Arc;

use superfik_core::ModelProvider;
use superfik_gateway::local::ChatUpstream;
use superfik_gateway::mlx::{MlxRuntime, SystemClock, SystemLauncher, downloaded};
use superfik_gateway::server::{Gateway, Upstreams};

fn main() -> std::io::Result<()> {
    let Some(root) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: mlx <local data folder>");
        std::process::exit(2);
    };
    let models = root.join("models").join("mlx");
    for model in downloaded(&models, superfik_core::EngineKind::Native) {
        eprintln!("downloaded: {} ({})", model.id, model.label);
    }
    let launcher = Arc::new(SystemLauncher::new(&root.join("mlx")));
    let runtime = Arc::new(MlxRuntime::new(models, launcher, Arc::new(SystemClock::default())));
    let upstream = ChatUpstream::new(runtime);
    let gateway = Gateway::start(Upstreams::new().with(ModelProvider::Local, Arc::new(upstream)))?;
    let access = gateway.admit(ModelProvider::Local);
    println!("ANTHROPIC_BASE_URL={} ANTHROPIC_AUTH_TOKEN={}", access.base_url, access.token);
    loop {
        std::thread::park();
    }
}
