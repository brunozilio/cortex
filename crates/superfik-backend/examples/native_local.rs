//! Runs one prompt on Superfik's own engine with a local model served by
//! MLX, printing the run's events: the real end-to-end path, no CLI.
//!
//! `cargo run -p superfik-backend --example native_local -- [model] [prompt]`
//! (the model is a folder under `<local data>/models/mlx`, by default
//! `mlx-community/Qwen3.8-27B-8bit`; the run works in a fresh temp dir).

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use superfik_core::{EngineEvent, EngineKind, ModelOption, ModelProvider, PermissionMode, PromptRequest, RunId};
use superfik_engine::AgentEngine;
use superfik_gateway::mlx::{MlxRuntime, SystemClock, SystemLauncher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model = args.next().unwrap_or_else(|| "mlx-community/Qwen3.8-27B-8bit".to_owned());
    let prompt = args.next().unwrap_or_else(|| "Create notes.md saying hello, then list the files here.".to_owned());
    let root = superfik_backend::local_data_dir().ok_or("no local data folder")?;
    let runtime = Arc::new(MlxRuntime::new(
        root.join("models").join("mlx"),
        Arc::new(SystemLauncher::new(&root.join("mlx"))),
        Arc::new(SystemClock::default()),
    ));
    let work = tempfile::tempdir()?;
    let engine = superfik_backend::native_engine(work.path(), Some(runtime));
    let option = ModelOption::new(EngineKind::Native, model.clone(), model).served_by(ModelProvider::Local);
    let mut request = PromptRequest::plain(work.path().to_path_buf(), prompt, option);
    request.permission = PermissionMode::Bypass;
    let (send, events) = channel();
    let sink = Arc::new(move |_: RunId, event: EngineEvent| {
        let _ = send.send(event);
    });
    engine.start(RunId::new(), &request, sink)?;
    while let Ok(event) = events.recv_timeout(Duration::from_secs(600)) {
        match &event {
            EngineEvent::TextDelta(text) => print!("{text}"),
            other => println!("\n[{other:?}]"),
        }
        if event.is_terminal() {
            break;
        }
    }
    println!("\nfiles: {:?}", std::fs::read_dir(work.path())?.flatten().map(|e| e.file_name()).collect::<Vec<_>>());
    Ok(())
}
