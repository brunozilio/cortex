use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{BoxFuture, CancellationToken, ModelApi, ModelError, Searched, Searcher};
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use agent_engine::{ApiSource, Engine};
use domain::{EngineEvent, PromptRequest};
use llm_providers::web_search::Hit;
use serde_json::json;

use crate::{finish, model, start};

struct Account;

impl Searcher for Account {
    fn search<'a>(&'a self, query: &'a str, _cancel: &'a CancellationToken) -> BoxFuture<'a, Searched> {
        Box::pin(std::future::ready(Some(Ok(vec![Hit {
            title: format!("About {query}"),
            url: "https://example.org/found".into(),
            snippet: String::new(),
        }]))))
    }
}

struct Searching(Arc<ScriptedModel>);

impl ApiSource for Searching {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(self.0.clone())
    }

    fn searcher(&self, _request: &PromptRequest) -> Option<Arc<dyn Searcher>> {
        Some(Arc::new(Account))
    }
}

#[test]
fn a_run_searches_the_web_through_its_account() {
    let dir = tempfile::tempdir().expect("dir");
    let search = Block::ToolUse { id: "s".into(), name: "WebSearch".into(), input: json!({"query": "kiro mcp"}) };
    let agent = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![search]),
        ScriptedModel::reply(vec![Block::Text("Found it.".into())]),
    ]));
    let engine = Engine::new(Arc::new(Searching(agent.clone())), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "search the web for kiro mcp", model());
    request.policy.allowed_tools = vec!["WebSearch".into()];

    let events = finish(&start(&engine, &request));

    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let seen = format!("{:?}", agent.asked()[1].messages);
    assert!(seen.contains("[About kiro mcp](https://example.org/found)"), "{seen}");
}
