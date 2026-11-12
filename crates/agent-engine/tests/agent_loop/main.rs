mod compaction;
mod custom_agents;
mod interception;
mod model_hooks;
mod permission_rules;
mod permissions;
mod plan_mode;
mod repeats;
mod safeguards;
mod script_skills;
mod scripts;
mod send_message;
mod steering;
mod sub_agent_fallbacks;
mod sub_agents;
mod turns;
mod watch;
mod watch_hooks;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use agent_engine::agent::{Agent, Ending};
use agent_engine::control::Control;
use agent_engine::model::TurnRequest;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{self, Workspace};
use agent_engine::transcript::{Block, Message};
use domain::{Effort, EngineEvent, PermissionMode};

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
        self.run_with(model, history, Vec::new())
    }

    fn run_with(
        &self,
        model: &ScriptedModel,
        history: &mut Vec<Message>,
        extra: Vec<Box<dyn tools::Tool>>,
    ) -> Result<(), Ending> {
        let mut tools = tools::standard();
        tools.extend(extra);
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
            turn_limit: agent_engine::agent::MAX_TURNS,
        };
        agent.run(history).map(|_| ())
    }

    fn events(&self) -> Vec<EngineEvent> {
        self.events.lock().expect("events").clone()
    }
}
