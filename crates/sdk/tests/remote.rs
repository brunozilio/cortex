#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, LocalBackend, LocalServices};
use domain::SessionId;
use sdk::protocol::{Backend, BackendError, ClientCommand, Envelope, ServerEvent};
use sdk::{EngineEvent, Error, McpServer, Model, Sdk};

struct OverTheWire(LocalBackend);

fn through_json<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    let text = serde_json::to_string(value).expect("serialize");
    serde_json::from_str(&text).expect("deserialize")
}

impl Backend for OverTheWire {
    fn send(&self, command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        self.0.send(through_json(&command))
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.0.try_recv().map(|event| through_json(&event))
    }
}

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn services(engine: &FakeEngine) -> LocalServices {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.data = None;
    services.off.insert("memory_search".into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    services
}

#[test]
fn a_session_runs_over_a_backend_that_only_sees_serialized_messages() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(
        vec![model()],
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::TextDelta("Hello from afar".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ],
    );
    let services = services(&engine);
    let sdk = Sdk::builder()
        .backend(move |wake| Ok(Box::new(OverTheWire(LocalBackend::new(services, wake)?)) as Box<dyn Backend>))
        .build()
        .expect("sdk");
    assert_eq!(sdk.models().expect("models").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["scripted"]);
    let session = sdk.session(data.path(), model());
    assert_eq!(session.send("hi").expect("run").text().expect("text"), "Hello from afar");
}

#[test]
fn in_process_servers_need_the_local_backend() {
    let built = Sdk::builder().mcp(McpServer::new("tools")).backend(|_| Err(BackendError::Disconnected)).build();
    assert!(matches!(built, Err(Error::Backend(message)) if message.contains("only the local backend")));
}
