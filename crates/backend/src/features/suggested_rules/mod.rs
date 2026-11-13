use std::path::Path;

use agent_engine::prompt::{permissions, settings};
use agent_engine::sessions::{FileSessions, records};
use domain::permission_rules::RuleKind;
use domain::suggested_rules::{Suggestion, suggestions};
use domain::trace::Record;
use domain::{ModelOption, Principal, PromptRequest, SessionId};
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let cwd = Path::new(input["cwd"].as_str().ok_or("suggested rules need the project's folder")?);
    if let Some(rule) = input["keep"]["rule"].as_str() {
        let kind = if input["keep"]["kind"] == "deny" { RuleKind::Deny } else { RuleKind::Allow };
        permissions::keep_rule(cwd, kind, rule)?;
        return Ok(json!([]));
    }
    let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
    let store = FileSessions::new(data.join(crate::native::SESSIONS));
    let owner_settings = context.services.settings.load(context.principal).unwrap_or_default();
    let sessions = recent(&owner_settings, cwd);
    let kept = settings::load(cwd, context.services.home.as_deref(), &[])?.permissions.listed();
    serde_json::to_value(suggested(&store, context.principal, cwd, &sessions, &kept)).map_err(|e| e.to_string())
}

pub fn forget(_client: u64) {}

fn recent(owner_settings: &Value, cwd: &Path) -> Vec<SessionId> {
    let kept = owner_settings["conversations"][cwd.to_string_lossy().as_ref()].as_array();
    kept.into_iter().flatten().filter_map(|c| c["session"].as_str().map(|id| SessionId(id.to_owned()))).collect()
}

fn suggested(
    store: &FileSessions,
    owner: &Principal,
    cwd: &Path,
    sessions: &[SessionId],
    kept: &[(RuleKind, String)],
) -> Vec<Suggestion> {
    let mut request = PromptRequest::plain(cwd.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
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
        let mut request = PromptRequest::plain(dir.path().into(), "", ModelOption::new("", ""));
        request.credentials.owner = Some(Principal::LocalOwner);
        let (one, two) = (SessionId("one".into()), SessionId("two".into()));
        for at in 0..3 {
            answered(&store, &request, &one, at, "Bash(cargo test:*)");
        }
        for at in 3..5 {
            answered(&store, &request, &two, at, "Bash(cargo test:*)");
        }
        let settings =
            json!({ "conversations": { dir.path().to_string_lossy(): [{ "session": "one" }, { "session": "two" }] } });

        let sessions = recent(&settings, dir.path());
        let found = suggested(&store, &Principal::LocalOwner, dir.path(), &sessions, &[]);

        assert_eq!(found, [Suggestion { rule: "Bash(cargo test:*)".into(), kind: RuleKind::Allow, times: 5 }]);
    }

    #[test]
    fn a_kept_suggestion_lands_in_the_project_s_local_settings() {
        let dir = tempfile::tempdir().expect("dir");
        let input = json!({ "cwd": dir.path(), "keep": { "rule": "Bash(git push:*)", "kind": "deny" } });

        crate::feature::testing::local(1, |context| handle(context, input)).expect("kept");

        let kept = std::fs::read_to_string(dir.path().join(brand::DIR).join("settings.local.json")).expect("settings");
        let kept: Value = serde_json::from_str(&kept).expect("json");
        assert_eq!(kept["permissions"]["deny"], json!(["Bash(git push:*)"]));
    }
}
