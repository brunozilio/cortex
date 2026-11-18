use std::path::PathBuf;

use domain::provider::ModelProvider;
use domain::{CustomAgent, EngineEvent, ModelOption, RunId, Skill};
use serde::{Deserialize, Serialize};

use crate::api::features::memory::MemoryApplied;
use crate::api::{FeatureError, FeatureReply, ProviderEvent, RequestId};

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
    Settings {
        settings: Option<serde_json::Value>,
    },
    Skills(Vec<Skill>),
    Agents(Vec<CustomAgent>),
    McpServers {
        path: PathBuf,
        servers: Vec<domain::mcp_config::McpServerInfo>,
        #[serde(default)]
        unreadable: Vec<domain::mcp_config::McpUnreadable>,
    },
    McpServer(domain::mcp_config::McpEntry),
    McpAuthorizeAt {
        name: String,
        url: String,
    },
    Notices(Notices),
    Feature(FeatureReply),
    FeatureFailed(FeatureError),
    Permissions {
        path: PathBuf,
        permissions: domain::permission_rules::PermissionsInfo,
    },
    Trust(domain::trust::TrustReport),
    Providers(ProviderEvent),
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notices {
    pub for_user: Vec<String>,
    pub for_agent: Vec<domain::inbox::Notice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub teammate_asks: Vec<domain::team::TeammateAsk>,
}

impl Notices {
    pub fn is_empty(&self) -> bool {
        self.for_user.is_empty() && self.for_agent.is_empty() && self.teammate_asks.is_empty()
    }
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
    MemoryApplied(MemoryApplied),
    MemoryFailed { path: PathBuf, message: String },
    CatalogFailed { provider: ModelProvider, message: String },
}

#[cfg(test)]
mod tests {
    use domain::provider::PortHolder;

    use crate::api::features::memory::MemoryListing;

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
            Reply::McpServers { path: "/repo".into(), servers: Vec::new(), unreadable: Vec::new() },
            Reply::McpServers {
                path: "/repo".into(),
                servers: Vec::new(),
                unreadable: vec![domain::mcp_config::McpUnreadable::broken(
                    "/home/mcp.json".into(),
                    domain::mcp_config::McpOrigin::User,
                    "EOF".into(),
                )],
            },
            Reply::McpServer(domain::mcp_config::McpEntry {
                info: domain::mcp_config::McpServerInfo {
                    name: "jira".into(),
                    origin: domain::mcp_config::McpOrigin::Project,
                    target: "jira-mcp".into(),
                    state: domain::mcp_config::McpState::Untrusted,
                },
                file: Some("/repo/mcp.json".into()),
                entry: Some(serde_json::json!({"command": "jira-mcp"})),
            }),
            Reply::McpAuthorizeAt { name: "linear".into(), url: "https://auth.test".into() },
            Reply::Notices(Notices {
                for_user: vec!["keys cannot be kept".into()],
                for_agent: vec![domain::inbox::Notice::new("monitor bash_1", "ready")],
                teammate_asks: vec![domain::team::TeammateAsk {
                    team: "team-1".into(),
                    teammate: "tester".into(),
                    request_id: "p1".into(),
                    asked: domain::team::Asked::Permission {
                        tool_name: "Bash".into(),
                        description: "npm test".into(),
                        rememberable: true,
                        rule: None,
                    },
                }],
            }),
            Reply::Feature(FeatureReply::Sessions(crate::api::features::sessions::SessionsReply::Length(7))),
            Reply::Feature(FeatureReply::Trace(crate::api::features::trace::TraceReply::Report(
                serde_json::json!({ "sessions": [] }),
            ))),
            Reply::FeatureFailed(FeatureError::FeatureOff { feature: crate::api::FeatureName::Memory }),
            Reply::FeatureFailed(FeatureError::Failed {
                feature: crate::api::FeatureName::Memory,
                message: "nothing to undo".into(),
            }),
            Reply::Permissions { path: "/repo".into(), permissions: Default::default() },
            Reply::Trust(domain::trust::TrustReport {
                root: "/repo".into(),
                trust: domain::trust::Trust::Untrusted,
                refusal: Some(domain::trust::Refusal::Home),
                brings: {
                    let mut brings = domain::trust::Contributions::default();
                    brings.add(domain::trust::ContributionKind::HookScript, "hooks/guard.js");
                    brings
                },
            }),
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
            ServerEvent::Unsolicited(Unsolicited::MemoryApplied(MemoryApplied {
                optimizing: false,
                added: 1,
                changed: 0,
                forgotten: 0,
                listing: MemoryListing { path: "/repo".into(), memories: Vec::new(), forgotten: Vec::new() },
            })),
            ServerEvent::Unsolicited(Unsolicited::MemoryFailed {
                path: "/repo".into(),
                message: "could not save memory".into(),
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
