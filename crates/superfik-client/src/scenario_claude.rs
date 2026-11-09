//! Snapshot scenario for the Claude Code integration: a reply with its
//! steps expanded, the engine-grouped model picker, custom agents and a
//! notice (`--scenario claude`), the picker open (`claude-models`), or a
//! sub-agent's session open beside its card (`claude-agent`).

use std::path::PathBuf;

use superfik_core::{
    Activity, AgentStatus, CustomAgent, EngineInfo, EngineKind, EngineProblem, ModelOption, SubAgent, ToolCall, Turn,
    TurnResponse, UserMessage,
};

use super::scenarios::SyntheticImage;
use crate::state::{Notice, PopoverKind, Session};

const ROOT: &str = "/work/superfik";

fn engine(kind: EngineKind, installed: bool, implemented: bool, models: Vec<ModelOption>) -> EngineInfo {
    EngineInfo { kind, installed, implemented, models }
}

fn claude_models() -> Vec<ModelOption> {
    [("claude-opus-5-5", "Opus 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5"), ("claude-haiku-5-5", "Haiku 5.5")]
        .into_iter()
        .map(|(id, label)| ModelOption::new(EngineKind::Native, id, label))
        .collect()
}

fn reply() -> TurnResponse {
    let tools = [
        ("Read", "/work/superfik/crates/superfik-app/src/state/run.rs"),
        ("Grep", "engine_session"),
        ("Edit", "/work/superfik/crates/superfik-app/src/state/server.rs"),
        ("Bash", "cargo test --workspace"),
        ("WebSearch", "claude stream-json resume"),
        ("Agent", "Review the reducer change"),
        ("Skill", "motion"),
    ];
    TurnResponse {
        activity: Activity::Done,
        text: "Follow-ups now resume the same Claude Code session, so context carries over.".into(),
        tools: tools
            .iter()
            .enumerate()
            .map(|(i, (name, summary))| ToolCall::new(i.to_string(), *name, *summary))
            .collect(),
        elapsed_ms: 41_000,
        failure: None,
        usage: Vec::new(),
        structured: None,
    }
}

pub fn claude() -> (Session, Vec<SyntheticImage>) {
    let mut s = Session::new(Some(PathBuf::from(ROOT)));
    s.engines = vec![
        engine(EngineKind::Native, true, true, claude_models()),
        engine(EngineKind::Codex, true, false, Vec::new()),
        engine(EngineKind::Kiro, false, false, Vec::new()),
        engine(EngineKind::Copilot, false, false, Vec::new()),
    ];
    s.models = claude_models();
    s.available_agents = vec![CustomAgent { name: "code-reviewer".into(), description: "Reviews diffs".into() }];
    let reviewer = SubAgent::spawned(
        0,
        "code-reviewer",
        "Review the follow-up change",
        "Review the change that resumes the engine session on follow-ups. Look for prompts that lose \
         their context and for runs that forget the session id.",
        &s.available_agents,
    );
    let explorer = SubAgent::spawned(1, "Explore", "Map how runs resume", "", &s.available_agents);
    let c = s.conversation_mut();
    c.agents = vec![reviewer, explorer];
    c.agents[0].summary = "No regressions found".into();
    if let Some(transcript) = c.agents[0].transcript.as_mut() {
        transcript.response.tools = [
            ("Grep", "engine_session"),
            ("Read", "crates/superfik-app/src/state/run.rs"),
            ("Read", "crates/superfik-native/src/agent.rs"),
            ("Bash", "cargo test -p superfik-app"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, summary))| ToolCall::new(format!("r{i}"), name, summary))
        .collect();
        transcript.response.text = "No regressions found.\n\n- Follow-ups pass `--resume` with the kept session id.\n\
             - Switching engines forgets the session, so nothing stale is resumed."
            .into();
    }
    c.agents[0].set_status(AgentStatus::Done { seconds: 12 });
    c.agents[1].status = AgentStatus::Running { progress: 0.6 };
    c.agents[1].summary = "Reading state/run.rs".into();
    let mut turn = Turn::new(UserMessage {
        text: "Make follow-ups keep their context.".into(),
        images: Vec::new(),
        branch_caption: "superfik/main".into(),
    });
    turn.response = reply();
    c.title = Some("Make follow-ups keep their context.".into());
    c.started_unix = Some(0);
    c.open_steps.insert(turn.id, 0.0);
    c.turns.push(turn);
    s.notice = Some(Notice::Engine(EngineProblem::NotLoggedIn, EngineKind::Native));
    (s, Vec::new())
}

pub fn claude_picker() -> (Session, Vec<SyntheticImage>) {
    use superfik_core::{Connection, ConnectionId, ModelProvider, PluginId};
    let (mut s, images) = claude();
    let account = |plugin: &str, name: &str, label: &str| Connection {
        id: ConnectionId::new(),
        plugin: PluginId(plugin.into()),
        plugin_name: name.into(),
        label: label.into(),
        icon: String::new(),
    };
    let work = account("claude-subscription", "Claude Subscription", "Work");
    let home = account("claude-subscription", "Claude Subscription", "Personal");
    let openai = account("openai", "OpenAI API", "");
    let model = |provider: ModelProvider, account: Option<&Connection>, id: &str, label: &str| {
        let model = ModelOption::new(EngineKind::Native, id, label).served_by(provider);
        match account {
            Some(account) => model.of_account(account.id),
            None => model,
        }
    };
    let claude = ModelProvider::ClaudeSubscription;
    s.models = vec![
        model(ModelProvider::OpenAi, Some(&openai), "openai:gpt-5.5", "gpt-5.5"),
        model(claude, Some(&work), "claude-subscription:claude-opus-5-5", "Opus 5.5"),
        model(claude, Some(&work), "claude-subscription:claude-sonnet-5-5", "Sonnet 5.5"),
        model(claude, Some(&home), "claude-subscription:claude-sonnet-5-5", "Sonnet 5.5"),
        model(ModelProvider::Local, None, "mlx-community/Qwen3.8-27B-8bit", "Qwen3.8 27B 8bit"),
    ];
    s.engines[0].models = s.models.clone();
    s.plugins.connections = vec![openai, work, home];
    s.conversation_mut().model = 1;
    s.popover = Some(PopoverKind::Model);
    s.notice = None;
    (s, images)
}

pub fn claude_agent() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = claude();
    s.notice = None;
    s.conversation_mut().open_agent = Some(0);
    (s, images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_shows_steps_engines_and_notice() {
        let (s, _) = claude();
        assert_eq!(s.steps_opened_at(0), Some(0.0));
        assert_eq!(
            s.conversation().turns[0].response.step_list(s.root.as_deref())[0].target,
            "crates/superfik-app/src/state/run.rs"
        );
        assert_eq!(s.model_rows().len(), 7, "one account, three models, three engines to come");
        assert_eq!(s.conversation().agents[0].label(), "code-reviewer");
        assert!(s.notice.is_some());
    }
}
