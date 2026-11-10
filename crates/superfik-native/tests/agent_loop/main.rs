//! The agent loop against a scripted model and real tools in a temp dir.

mod custom_agents;
mod permissions;
mod plan_mode;
mod send_message;
mod sub_agents;
mod turns;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use superfik_core::{Effort, EngineEvent, PermissionMode};
use superfik_native::agent::{Agent, Ending};
use superfik_native::control::Control;
use superfik_native::model::TurnRequest;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::tools::{self, Workspace};
use superfik_native::transcript::{Block, Message};

fn request() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: "s".into(),
        messages: Vec::new(),
        tools: Agent::specs(&tools::standard()),
        effort: Effort::Medium,
        max_tokens: 1000,
    }
}

fn tool_use(id: &str, name: &str, input: serde_json::Value) -> Block {
    Block::ToolUse { id: id.into(), name: name.into(), input }
}

struct Rig {
    dir: tempfile::TempDir,
    workspace: Workspace,
    control: Arc<Control>,
    events: Arc<Mutex<Vec<EngineEvent>>>,
}

impl Rig {
    fn new(mode: PermissionMode) -> Self {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").expect("write");
        let workspace = Workspace::new(dir.path());
        let control = Arc::new(Control::new(mode, Arc::new(AtomicBool::new(false))));
        Self { dir, workspace, control, events: Arc::default() }
    }

    fn run(&self, model: &ScriptedModel, history: &mut Vec<Message>) -> Result<(), Ending> {
        let tools = tools::standard();
        let events = Arc::clone(&self.events);
        let emit = move |event: EngineEvent| events.lock().expect("events").push(event);
        let agent = Agent {
            api: model,
            tools: &tools,
            workspace: &self.workspace,
            control: &self.control,
            emit: &emit,
            request: TurnRequest { tools: Agent::specs(&tools), ..request() },
            agents: &[],
            depth: 0,
            turn_limit: superfik_native::agent::MAX_TURNS,
        };
        agent.run(history).map(|_| ())
    }

    fn events(&self) -> Vec<EngineEvent> {
        self.events.lock().expect("events").clone()
    }
}
