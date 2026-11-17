use std::path::PathBuf;

use domain::provider::ModelProvider;
use domain::{CustomAgent, EngineEvent, ModelOption, RunId, Skill};
use serde::{Deserialize, Serialize};

use crate::{ProviderEvent, RequestId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerEvent {
    Run { run: RunId, event: EngineEvent },
    Reply { id: RequestId, reply: Reply },
    Unsolicited(Unsolicited),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Reply {
    Done,
    Models(Vec<ModelOption>),
    Settings { settings: Option<serde_json::Value> },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    McpServers { path: PathBuf, servers: Vec<domain::mcp_config::McpServerInfo> },
    McpAuthorizeAt { name: String, url: String },
    Notices(Vec<domain::inbox::Notice>),
    Feature { feature: String, output: Result<serde_json::Value, String> },
    Permissions { path: PathBuf, permissions: domain::permission_rules::PermissionsInfo },
    Providers(ProviderEvent),
    Error { message: String },
}

impl Reply {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }

    pub fn done_or(result: Result<(), String>) -> Self {
        match result {
            Ok(()) => Self::Done,
            Err(message) => Self::Error { message },
        }
    }

    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::McpAuthorizeAt { .. } | Self::Providers(ProviderEvent::AuthorizeAt { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Unsolicited {
    Feature { feature: String, output: Result<serde_json::Value, String> },
    CatalogFailed { provider: ModelProvider, message: String },
}

#[cfg(test)]
mod tests {
    use domain::provider::PortHolder;

    use super::*;

    fn provider_events() -> Vec<ProviderEvent> {
        let provider = ModelProvider::new("browser-sub");
        vec![
            ProviderEvent::Listed { providers: Vec::new() },
            ProviderEvent::AuthorizeAt { provider, url: "https://auth.test".into(), code: None },
            ProviderEvent::AuthorizeAt { provider, url: "https://auth.test".into(), code: Some("AB12".into()) },
            ProviderEvent::Failed { provider, message: "closed the tab".into() },
            ProviderEvent::PortBusy { provider, port: 49153, holder: None },
            ProviderEvent::PortBusy {
                provider,
                port: 49153,
                holder: Some(PortHolder { pid: 998, app: "other".into() }),
            },
        ]
    }

    fn replies() -> Vec<Reply> {
        let mut replies = vec![
            Reply::Done,
            Reply::Models(vec![ModelOption::new("m", "M")]),
            Reply::Settings { settings: None },
            Reply::Settings { settings: Some(serde_json::json!({ "model": "m" })) },
            Reply::Skills(Vec::new()),
            Reply::Agents(Vec::new()),
            Reply::McpServers { path: "/repo".into(), servers: Vec::new() },
            Reply::McpAuthorizeAt { name: "linear".into(), url: "https://auth.test".into() },
            Reply::Notices(vec![domain::inbox::Notice::new("secrets", "keys cannot be kept")]),
            Reply::Feature { feature: "memory".into(), output: Ok(serde_json::json!({ "memories": [] })) },
            Reply::Feature { feature: "memory".into(), output: Err("nothing to undo".into()) },
            Reply::Permissions { path: "/repo".into(), permissions: Default::default() },
            Reply::error("could not save the settings"),
        ];
        replies.extend(provider_events().into_iter().map(Reply::Providers));
        replies
    }

    fn round_trip<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        assert_eq!(&serde_json::from_str::<T>(&json).expect("deserialize"), value, "{json}");
    }

    #[test]
    fn every_provider_event_round_trips() {
        for event in provider_events() {
            round_trip(&event);
        }
    }

    #[test]
    fn every_server_event_round_trips() {
        let mut events = vec![
            ServerEvent::Run { run: RunId::new(), event: EngineEvent::TextDelta("hi".into()) },
            ServerEvent::Run { run: RunId::new(), event: EngineEvent::Cancelled },
            ServerEvent::Unsolicited(Unsolicited::Feature {
                feature: "memory".into(),
                output: Ok(serde_json::json!({ "memories": [] })),
            }),
            ServerEvent::Unsolicited(Unsolicited::CatalogFailed {
                provider: ModelProvider::new("keyed"),
                message: "401".into(),
            }),
        ];
        events.extend(replies().into_iter().map(|reply| ServerEvent::Reply { id: RequestId(7), reply }));
        for event in events {
            round_trip(&event);
        }
    }

    #[test]
    fn a_reply_names_the_request_it_answers() {
        let json = serde_json::to_value(ServerEvent::Reply { id: RequestId(7), reply: Reply::Done }).expect("json");
        assert_eq!(json, serde_json::json!({ "type": "reply", "data": { "id": 7, "reply": { "type": "done" } } }));
    }

    #[test]
    fn only_progress_leaves_a_request_open() {
        let open: Vec<Reply> = replies().into_iter().filter(|reply| !reply.is_terminal()).collect();
        assert!(open.iter().all(|reply| matches!(
            reply,
            Reply::McpAuthorizeAt { .. } | Reply::Providers(ProviderEvent::AuthorizeAt { .. })
        )));
        assert_eq!(open.len(), 3);
        assert!(Reply::done_or(Ok(())).is_terminal() && Reply::done_or(Err("no".into())).is_terminal());
    }
}
