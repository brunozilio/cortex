use std::path::Path;

use crate::api::features::suggested_rules::{SuggestedRulesReply, SuggestedRulesRequest};
use agent_engine::prompt::{permissions, settings};
use agent_engine::sessions::{FileSessions, records};
use domain::permission_rules::RuleKind;
use domain::suggested_rules::{Suggestion, suggestions};
use domain::trace::Record;
use domain::{ModelOption, PromptRequest, SessionId};

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: SuggestedRulesRequest) -> Result<SuggestedRulesReply, String> {
    match request {
        SuggestedRulesRequest::List { cwd } => {
            let store = context.services.sessions.as_ref();
            let owner_settings = context.services.settings.load().unwrap_or_default();
            let sessions = crate::settings::recent_sessions(&owner_settings, &cwd);
            let kept = settings::load(&cwd, context.services.home().as_deref(), &[])?.permissions.listed();
            Ok(SuggestedRulesReply::Suggested(suggested(store, &cwd, &sessions, &kept)))
        }
        SuggestedRulesRequest::KeepRule { cwd, kind, rule } => {
            permissions::keep_rule(&cwd, kind, &rule)?;
            Ok(SuggestedRulesReply::RuleKept)
        }
        SuggestedRulesRequest::KeepInstruction { cwd, line } => {
            keep_instruction(&cwd, &line).map(|file| SuggestedRulesReply::InstructionKept { file })
        }
    }
}

pub fn forget(_client: u64) {}

fn keep_instruction(cwd: &Path, line: &str) -> Result<std::path::PathBuf, String> {
    let root = cwd.join(brand::INSTRUCTIONS);
    let nested = cwd.join(brand::DIR).join(brand::INSTRUCTIONS);
    let file = if !root.exists() && nested.exists() { nested } else { root };
    let current = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", file.display())),
    };
    let updated = domain::learning::with_learned(&current, line)?;
    std::fs::write(&file, updated).map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(file)
}

fn suggested(store: &FileSessions, cwd: &Path, sessions: &[SessionId], kept: &[(RuleKind, String)]) -> Vec<Suggestion> {
    let request = PromptRequest::plain(cwd.into(), "", ModelOption::new("", ""));
    let events: Vec<_> = sessions
        .iter()
        .filter_map(|id| store.entries(&request, id).ok())
        .flat_map(records)
        .filter_map(|record| match record {
            Record::Event { event, .. } => Some(event),
            _ => None,
        })
        .collect();
    suggestions(&events, kept)
}

#[cfg(test)]
mod tests {
    use agent_engine::sessions::{Entry, Sessions};
    use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent};

    use super::*;

    fn answered(store: &FileSessions, request: &PromptRequest, session: &SessionId, at: u64, rule: &str) {
        let id = AskId(format!("ask-{at}"));
        let kind = AskKind::Permission {
            tool: "Bash".into(),
            subject: "x".into(),
            rememberable: true,
            rule: Some(rule.into()),
        };
        let asked = EngineEvent::Asked(AgentAsk { id: id.clone(), kind });
        let answer = EngineEvent::Answered { ask: id, decision: Decision::Allow };
        for event in [asked, answer] {
            store.record(request, session, &Entry::Event { at_ms: at, event }).expect("record");
        }
    }

    #[test]
    fn approvals_across_the_project_s_sessions_become_a_suggestion() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FileSessions::new(dir.path().join("sessions"));
        let request = PromptRequest::plain(dir.path().into(), "", ModelOption::new("", ""));
        let (one, two) = (SessionId("one".into()), SessionId("two".into()));
        for at in 0..3 {
            answered(&store, &request, &one, at, "Bash(cargo test:*)");
        }
        for at in 3..5 {
            answered(&store, &request, &two, at, "Bash(cargo test:*)");
        }
        let settings = serde_json::json!({
            "conversations": { dir.path().to_string_lossy(): [{ "session": "one" }, { "session": "two" }] }
        });

        let sessions = crate::settings::recent_sessions(&settings, dir.path());
        let found = suggested(&store, dir.path(), &sessions, &[]);

        assert_eq!(found, [Suggestion { rule: "Bash(cargo test:*)".into(), kind: RuleKind::Allow, times: 5 }]);
    }

    #[test]
    fn a_kept_suggestion_lands_in_the_project_s_local_settings() {
        let dir = tempfile::tempdir().expect("dir");
        let keep = |kind: RuleKind, rule: &str| {
            let request = SuggestedRulesRequest::KeepRule { cwd: dir.path().to_path_buf(), kind, rule: rule.into() };
            crate::features::context::testing::local(1, |context| handle(context, request))
        };

        assert_eq!(keep(RuleKind::Deny, "Bash(git push:*)"), Ok(SuggestedRulesReply::RuleKept));
        keep(RuleKind::Allow, "Bash(ls:*)").expect("kept");
        keep(RuleKind::Ask, "Bash(rm:*)").expect("kept");

        let kept = std::fs::read_to_string(dir.path().join(brand::DIR).join("settings.local.json")).expect("settings");
        let kept: serde_json::Value = serde_json::from_str(&kept).expect("json");
        let expected = serde_json::json!({
            "deny": ["Bash(git push:*)"],
            "allow": ["Bash(ls:*)"],
            "ask": ["Bash(rm:*)"],
        });
        assert_eq!(kept["permissions"], expected, "each kind lands in its own list");
    }

    #[test]
    fn a_project_with_no_answers_yet_has_nothing_to_suggest() {
        let dir = tempfile::tempdir().expect("dir");
        let request = SuggestedRulesRequest::List { cwd: dir.path().to_path_buf() };
        let listed = crate::features::context::testing::local(1, |context| handle(context, request));
        assert_eq!(listed, Ok(SuggestedRulesReply::Suggested(Vec::new())));
    }

    #[test]
    fn a_kept_instruction_lands_in_the_project_s_instructions() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join(brand::INSTRUCTIONS), "# Project\n").expect("instructions");
        let keep = |line: &str| {
            let request = SuggestedRulesRequest::KeepInstruction { cwd: dir.path().to_path_buf(), line: line.into() };
            crate::features::context::testing::local(1, |context| handle(context, request))
        };

        let written = keep("Use pnpm.");
        let refused = keep("password=hunter2");

        let file = dir.path().join(brand::INSTRUCTIONS);
        assert_eq!(written, Ok(SuggestedRulesReply::InstructionKept { file }));
        let kept = std::fs::read_to_string(dir.path().join(brand::INSTRUCTIONS)).expect("instructions");
        assert_eq!(kept, "# Project\n\n## Learned\n- Use pnpm.\n");
        assert!(refused.is_err());
    }
}
