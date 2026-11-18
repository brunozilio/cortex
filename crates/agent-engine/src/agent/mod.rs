pub mod current;
mod dispatch;
mod observers;
mod permission;
mod repl;
mod run;
mod scripts;
mod subagents;
mod team_tools;
mod turn;

use domain::EngineEvent;

use self::run::ThisRun;
use self::subagents::background::CancelOnDrop;
use crate::control::Control;
use crate::model::{ModelApi, ModelError, ToolSpec, TurnRequest};
use crate::tools::subagent::FoundAgent;
use crate::tools::{Tool, Workspace};
use crate::transcript::{Message, Usage};

pub type Keep = std::sync::Arc<dyn Fn(&[Message]) + Send + Sync>;

pub const MAX_TURNS: usize = 200;
const SHOWN_OUTPUT: usize = 64_000;

#[derive(Debug, PartialEq)]
pub enum Ending {
    Cancelled,
    Failed(ModelError),
    Refused,
    TooManyTurns,
    Moved,
}

pub trait Review: Sync {
    fn review(&self, agent: &Agent<'_>, run: &[Message], attempts: &mut u8) -> Result<Option<Message>, Ending>;
}

pub struct Agent<'a> {
    pub api: &'a dyn ModelApi,
    pub tools: &'a [Box<dyn Tool>],
    pub workspace: &'a Workspace,
    pub control: &'a Control,
    pub emit: &'a (dyn Fn(EngineEvent) + Sync),
    pub request: TurnRequest,
    pub agents: &'a [FoundAgent],
    pub depth: usize,
    pub turn_limit: usize,
}

impl std::fmt::Debug for Agent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent").field("model", &self.request.model).finish_non_exhaustive()
    }
}

impl Agent<'_> {
    pub fn specs(tools: &[Box<dyn Tool>]) -> Vec<ToolSpec> {
        tools.iter().map(|tool| tool.spec()).collect()
    }

    pub fn run(&self, history: &mut Vec<Message>) -> Result<Usage, Ending> {
        self.run_in(&ThisRun::default(), history)
    }

    pub fn run_keeping(&self, history: &mut Vec<Message>, keep: Option<Keep>) -> Result<Usage, Ending> {
        self.run_in(&ThisRun { keep, ..ThisRun::default() }, history)
    }

    fn run_in(&self, run: &ThisRun, history: &mut Vec<Message>) -> Result<Usage, Ending> {
        std::thread::scope(|scope| {
            let _cancel_background = CancelOnDrop(&run.background);
            let spawn = |job| {
                scope.spawn(job);
            };
            let outcome = self.turns(history, run, &spawn);
            if outcome.is_err() {
                run.background.cancel_all();
            }
            outcome
        })
    }
}
