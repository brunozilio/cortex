use superfik_core::{Activity, AgentRole, AgentStatus, Command, ContextSlice, Effort, EngineKind, EngineProblem};

use crate::Text;

pub trait Catalog: Sync {
    fn text(&self, text: Text<'_>) -> String;

    fn activity(&self, activity: &Activity, elapsed: &str) -> String;

    fn today(&self) -> &'static str;
    fn yesterday(&self) -> &'static str;
    fn weekday(&self, index: usize) -> &'static str;
    fn date(&self, month: u32, day: u32, year: Option<i64>) -> String;

    fn effort(&self, effort: Effort) -> &'static str;
    fn command(&self, command: Command) -> &'static str;
    fn role(&self, role: AgentRole) -> &'static str;
    fn idle_summary(&self, role: AgentRole) -> &'static str;
    fn context_slice(&self, slice: &ContextSlice) -> Option<&'static str>;
    fn waiting_turn(&self) -> &'static str;
    fn status(&self, status: &AgentStatus) -> Option<&'static str>;
    fn step_verb(&self, verb: &str) -> Option<&'static str>;
    fn engine_problem(&self, problem: EngineProblem, engine: EngineKind) -> String;
}
