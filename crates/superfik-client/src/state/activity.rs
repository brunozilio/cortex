//! Which context nodes the running agent is using right now, from its tool
//! calls: the repository lights up while files are read, research while the
//! web is searched, skills while a skill is loaded. Pure and tested.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use superfik_core::{EngineEvent, ToolCall};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextNode {
    Repository,
    Skills,
    References,
    Research,
    Connections,
}

const LINGER: f64 = 0.9;

#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub references: &'a [PathBuf],
}

pub fn touched(call: &ToolCall, context: Context) -> Vec<ContextNode> {
    if call.is_web() {
        return vec![ContextNode::Research];
    }
    if call.name == "Skill" {
        return vec![ContextNode::Skills];
    }
    if call.name.starts_with("mcp__")
        && !call.name.starts_with(&format!("mcp__{}__", superfik_core::memory::MEMORY_SERVER))
    {
        return vec![ContextNode::Connections];
    }
    if !matches!(call.name.as_str(), "Read" | "Glob" | "Grep" | "LS" | "NotebookRead") {
        return Vec::new();
    }
    let path = call.summary.as_str();
    let mut nodes = vec![ContextNode::Repository];
    if path.ends_with("SKILL.md") || path.contains("/skills/") {
        nodes.push(ContextNode::Skills);
    }
    if context.references.iter().any(|r| Path::new(path) == r || path.ends_with(&*r.to_string_lossy())) {
        nodes.push(ContextNode::References);
    }
    nodes
}

#[derive(Debug, Clone, Default)]
pub struct Activity {
    open: HashMap<String, Vec<ContextNode>>,
    released: HashMap<ContextNode, f64>,
}

impl Activity {
    pub fn observe(&mut self, event: &EngineEvent, context: Context, now: f64) {
        match event {
            EngineEvent::ToolStarted(call) => {
                let nodes = touched(call, context);
                if !nodes.is_empty() {
                    self.open.insert(call.id.clone(), nodes);
                }
            }
            EngineEvent::ToolFinished { id } => {
                for node in self.open.remove(id).unwrap_or_default() {
                    self.released.insert(node, now);
                }
            }
            EngineEvent::ResearchFound(_) => {
                self.released.insert(ContextNode::Research, now);
            }
            event if event.is_terminal() => self.clear(),
            _ => {}
        }
    }

    pub fn is_active(&self, node: ContextNode, now: f64) -> bool {
        self.open.values().any(|nodes| nodes.contains(&node))
            || self.released.get(&node).is_some_and(|at| now - at < LINGER)
    }

    pub fn is_open(&self, node: ContextNode) -> bool {
        self.open.values().any(|nodes| nodes.contains(&node))
    }

    pub fn clear(&mut self) {
        self.open.clear();
        self.released.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(id: &str, name: &str, summary: &str) -> EngineEvent {
        EngineEvent::ToolStarted(ToolCall::new(id, name, summary))
    }

    fn context(references: &[PathBuf]) -> Context<'_> {
        Context { references }
    }

    #[test]
    fn reads_light_the_repository_and_matching_context() {
        let refs = [PathBuf::from("/tmp/sky.png")];
        let c = context(&refs);
        assert_eq!(touched(&ToolCall::new("1", "Read", "/repo/README.md"), c), vec![ContextNode::Repository]);
        assert!(touched(&ToolCall::new("3", "Read", "/tmp/sky.png"), c).contains(&ContextNode::References));
        assert!(
            touched(&ToolCall::new("4", "Read", "/repo/.claude/skills/motion/SKILL.md"), c)
                .contains(&ContextNode::Skills)
        );
        assert_eq!(touched(&ToolCall::new("5", "Bash", "cargo test"), c), Vec::<ContextNode>::new());
    }

    #[test]
    fn web_and_skill_tools_map_to_their_nodes() {
        let c = context(&[]);
        assert_eq!(touched(&ToolCall::new("1", "WebSearch", "night skies"), c), vec![ContextNode::Research]);
        assert_eq!(touched(&ToolCall::new("2", "Skill", "motion"), c), vec![ContextNode::Skills]);
        assert_eq!(touched(&ToolCall::new("3", "mcp__gmail-work__send", ""), c), vec![ContextNode::Connections]);
        let memory = format!("mcp__{}__memory_search", superfik_core::memory::MEMORY_SERVER);
        assert!(touched(&ToolCall::new("4", &memory, ""), c).is_empty(), "memory is not an account");
    }

    #[test]
    fn nodes_stay_lit_while_open_and_linger_briefly() {
        let mut activity = Activity::default();
        let c = context(&[]);
        activity.observe(&call("t1", "Read", "/repo/a.rs"), c, 1.0);
        assert!(activity.is_active(ContextNode::Repository, 5.0), "open calls stay lit");
        assert!(activity.is_open(ContextNode::Repository));
        activity.observe(&EngineEvent::ToolFinished { id: "t1".into() }, c, 5.0);
        assert!(activity.is_active(ContextNode::Repository, 5.5));
        assert!(!activity.is_active(ContextNode::Repository, 6.0));
        assert!(!activity.is_open(ContextNode::Repository));
        assert!(!activity.is_active(ContextNode::Research, 5.0));
    }

    #[test]
    fn the_end_of_a_run_clears_everything() {
        let mut activity = Activity::default();
        let c = context(&[]);
        activity.observe(&call("t1", "WebSearch", "q"), c, 1.0);
        activity.observe(&EngineEvent::Completed { duration_ms: 10, cost_usd: None }, c, 2.0);
        assert!(!activity.is_active(ContextNode::Research, 2.0));
    }
}
