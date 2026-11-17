use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, Extensions, PromptRequest};

use crate::{Source, finish, model, start};

#[test]
fn a_user_prompt_submit_hook_rewrites_the_prompt_the_model_reads() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    let hooks = home.path().join(brand::DIR).join("hooks");
    std::fs::create_dir_all(&hooks).expect("hooks");
    std::fs::write(
        hooks.join("rewrite.js"),
        r#"export const event = "UserPromptSubmit";
export default (input) => ({ hookSpecificOutput: { updatedPrompt: input.prompt.replace("password 1234", "password [hidden]") } });"#,
    )
    .expect("hook");
    std::fs::write(
        hooks.join("rewrite-again.js"),
        r#"export const event = "UserPromptSubmit";
export default (input) => ({ hookSpecificOutput: { updatedPrompt: `${input.prompt} [hidden]` } });"#,
    )
    .expect("hook");
    std::fs::write(
        hooks.join("team.js"),
        r#"export const event = "SessionStart";
export default () => "Team: ACME";"#,
    )
    .expect("hook");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Ok".into())])]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(repo.path().into(), "my password 1234 leaks", model());
    request.host.extensions = Some(Extensions { home: Some(home.path().into()), ..Extensions::default() });

    let events = finish(&start(&engine, &request));

    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let sent = model_api.asked()[0].messages.last().expect("user").text();
    assert!(sent.contains("my password [hidden] leaks"), "{sent}");
    assert!(!sent.contains("1234"), "{sent}");
    assert_eq!(sent.matches("Team: ACME").count(), 1, "the session's context stays once: {sent}");
    assert_eq!(sent.matches("[hidden]").count(), 2, "the second hook saw the first one's prompt: {sent}");
}
