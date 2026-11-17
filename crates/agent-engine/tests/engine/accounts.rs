use std::sync::Arc;

use agent_engine::model::pooled::Pooled;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelApi, ModelError};
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use agent_engine::{ApiSource, Engine};
use domain::{EngineEvent, PermissionMode, PromptRequest};

use crate::{finish, model, start};

struct TwoAccounts(Arc<Pooled>);

impl ApiSource for TwoAccounts {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(self.0.clone())
    }
}

#[test]
fn a_run_tells_when_another_account_takes_over() {
    let busy = Arc::new(ScriptedModel::new([Err(ModelError::Overloaded("429".into()))]));
    let free = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])]));
    let pooled = Pooled::new(vec![("a@x.io".into(), busy as Arc<dyn ModelApi>), ("b@x.io".into(), free)]);
    let engine = Engine::new(Arc::new(TwoAccounts(Arc::new(pooled))), Arc::new(InMemorySessions::default()));
    let dir = tempfile::tempdir().expect("dir");
    let mut request = PromptRequest::plain(dir.path().into(), "go", model());
    request.policy.permission = PermissionMode::Bypass;
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let switched = EngineEvent::AccountSwitched { from: "a@x.io".into(), to: "b@x.io".into() };
    assert!(events.contains(&switched), "{events:?}");
}
