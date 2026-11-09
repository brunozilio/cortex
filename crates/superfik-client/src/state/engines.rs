//! Engines and models: which can be picked, how the picker groups them,
//! and what to tell the user when an engine cannot run.

use superfik_core::{Availability, ConnectionId, EngineInfo, EngineKind, EngineProblem, ModelOption, ModelProvider};

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRow {
    Engine { kind: EngineKind, availability: Availability },
    Account { provider: ModelProvider, account: Option<ConnectionId> },
    Model(usize),
}

impl Session {
    pub fn model_rows(&self) -> Vec<ModelRow> {
        let selectable = |model: &ModelOption| self.engine_info(model.engine).is_none_or(EngineInfo::is_selectable);
        let mut groups: Vec<(ModelProvider, Option<ConnectionId>, Vec<usize>)> = Vec::new();
        for (index, model) in self.models.iter().enumerate().filter(|(_, model)| selectable(model)) {
            match groups.iter_mut().find(|(p, a, _)| (*p, *a) == (model.provider, model.account)) {
                Some((_, _, indices)) => indices.push(index),
                None => groups.push((model.provider, model.account, vec![index])),
            }
        }
        let mut rows = Vec::new();
        for (provider, account, indices) in groups {
            rows.push(ModelRow::Account { provider, account });
            rows.extend(indices.into_iter().map(ModelRow::Model));
        }
        let soon = self.engines.iter().filter(|info| !info.is_selectable());
        rows.extend(soon.map(|info| ModelRow::Engine { kind: info.kind, availability: info.availability() }));
        rows
    }

    pub fn account_title(&self, provider: ModelProvider, account: Option<ConnectionId>) -> String {
        if provider == ModelProvider::Local {
            return self.strings().t(superfik_i18n::Text::AiLocalModels);
        }
        let label = account
            .and_then(|id| self.plugins.connections.iter().find(|c| c.id == id))
            .map(|c| c.label.trim())
            .filter(|label| !label.is_empty());
        match label {
            Some(label) => format!("{label} · {}", provider.name()),
            None => provider.name().to_owned(),
        }
    }

    pub(super) fn replace_models(&mut self, models: Vec<ModelOption>) {
        for conversation in &mut self.conversations {
            let key = conversation.model_key(&self.models);
            match super::history::find_model(&models, &key) {
                Some(index) => {
                    conversation.model = index;
                    conversation.wanted_model = None;
                }
                None => {
                    conversation.model = 0;
                    conversation.wanted_model = (!key.0.is_empty()).then_some(key);
                }
            }
        }
        self.models = models;
    }

    pub fn is_model_selectable(&self, index: usize) -> bool {
        let Some(model) = self.models.get(index) else { return false };
        self.engine_info(model.engine).is_none_or(EngineInfo::is_selectable)
            && self.organization.policy.allows_model(&model.id)
    }

    fn engine_info(&self, kind: EngineKind) -> Option<&EngineInfo> {
        self.engines.iter().find(|info| info.kind == kind)
    }

    pub(super) fn set_engines(&mut self, engines: Vec<EngineInfo>) {
        let models: Vec<_> =
            engines.iter().filter(|info| info.is_selectable()).flat_map(|info| info.models.clone()).collect();
        self.replace_models(models);
        self.engines = engines;
        let kind = self.model().engine;
        if self.engine_info(kind).is_some_and(|info| info.availability() == Availability::NotInstalled) {
            self.notice = Some(super::Notice::Engine(EngineProblem::NotInstalled, kind));
        }
    }

    pub(super) fn dismiss_notice(&mut self) -> Vec<Effect> {
        self.notice = None;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::ModelOption;

    use super::*;
    use crate::state::Action;
    use superfik_protocol::ServerEvent;

    fn info(kind: EngineKind, installed: bool, implemented: bool, models: &[&str]) -> EngineInfo {
        let models = models.iter().map(|id| ModelOption::new(kind, *id, *id)).collect();
        EngineInfo { kind, installed, implemented, models }
    }

    fn engines() -> Vec<EngineInfo> {
        vec![
            info(EngineKind::Native, true, true, &["opus", "haiku"]),
            info(EngineKind::Codex, true, false, &["default"]),
            info(EngineKind::Kiro, false, false, &["default"]),
        ]
    }

    #[test]
    fn picker_groups_models_and_lists_soon_engines() {
        let mut s = Session::default();
        s.set_engines(engines());
        assert_eq!(
            s.model_rows(),
            vec![
                ModelRow::Account { provider: ModelProvider::Anthropic, account: None },
                ModelRow::Model(0),
                ModelRow::Model(1),
                ModelRow::Engine { kind: EngineKind::Codex, availability: Availability::Soon { installed: true } },
                ModelRow::Engine { kind: EngineKind::Kiro, availability: Availability::Soon { installed: false } },
            ]
        );
        assert_eq!(s.models.len(), 2);
        assert!(s.notice.is_none());
    }

    fn connection(plugin: &str, label: &str) -> superfik_core::Connection {
        superfik_core::Connection {
            id: ConnectionId::new(),
            plugin: superfik_core::PluginId(plugin.into()),
            plugin_name: plugin.into(),
            label: label.into(),
            icon: String::new(),
        }
    }

    fn of(provider: ModelProvider, account: Option<ConnectionId>, id: &str) -> ModelOption {
        let model = ModelOption::new(EngineKind::Native, id, id).served_by(provider);
        match account {
            Some(account) => model.of_account(account),
            None => model,
        }
    }

    fn accounts() -> (Session, ConnectionId, ConnectionId, ConnectionId) {
        let (work, home, api) = (
            connection("claude-subscription", "Work"),
            connection("claude-subscription", "Home"),
            connection("openai", ""),
        );
        let mut s = Session::default();
        s.plugins.connections = vec![work.clone(), home.clone(), api.clone()];
        s.apply(
            Action::Server(ServerEvent::Models(vec![
                of(ModelProvider::OpenAi, Some(api.id), "openai:gpt-5"),
                of(ModelProvider::ClaudeSubscription, Some(work.id), "claude-subscription:opus"),
                of(ModelProvider::ClaudeSubscription, Some(work.id), "claude-subscription:sonnet"),
                of(ModelProvider::ClaudeSubscription, Some(home.id), "claude-subscription:opus"),
                of(ModelProvider::Local, None, "mlx-community/Qwen"),
            ])),
            0.0,
        );
        (s, work.id, home.id, api.id)
    }

    #[test]
    fn the_picker_lists_each_account_with_its_own_models() {
        let (s, work, home, api) = accounts();
        let claude = ModelProvider::ClaudeSubscription;
        assert_eq!(
            s.model_rows(),
            vec![
                ModelRow::Account { provider: ModelProvider::OpenAi, account: Some(api) },
                ModelRow::Model(0),
                ModelRow::Account { provider: claude, account: Some(work) },
                ModelRow::Model(1),
                ModelRow::Model(2),
                ModelRow::Account { provider: claude, account: Some(home) },
                ModelRow::Model(3),
                ModelRow::Account { provider: ModelProvider::Local, account: None },
                ModelRow::Model(4),
            ]
        );
        assert_eq!(s.account_title(claude, Some(work)), "Work · Claude Subscription");
        assert_eq!(s.account_title(ModelProvider::OpenAi, Some(api)), "OpenAI API", "unnamed: the provider");
        assert_eq!(s.account_title(ModelProvider::Local, None), "Local models");
    }

    #[test]
    fn a_model_runs_on_the_account_it_was_picked_from() {
        let (mut s, _, home, _) = accounts();
        s.root = Some("/repo".into());
        s.apply(Action::SelectModel(3), 0.0);
        assert_eq!(s.model().account, Some(home));
        let models = s.models.iter().rev().cloned().collect();
        s.apply(Action::Server(ServerEvent::Models(models)), 0.0);
        assert_eq!((s.model().id.as_str(), s.model().account), ("claude-subscription:opus", Some(home)));
    }

    #[test]
    fn nothing_connected_offers_no_model_and_says_so_on_send() {
        let mut s = Session::new(Some("/repo".into()));
        s.apply(Action::Server(ServerEvent::Models(Vec::new())), 0.0);
        assert!(s.model_rows().is_empty());
        s.conversation_mut().composer = superfik_ui::TextEdit::new("hello");
        let effects = s.apply(Action::Submit, 0.0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Command(_))));
        assert_eq!(s.notice, Some(crate::state::Notice::NoModels));
    }

    #[test]
    fn a_kept_session_waits_for_its_account_s_model() {
        let (mut listed, _, home, _) = accounts();
        listed.apply(Action::SelectModel(3), 0.0);
        listed.conversation_mut().started_unix = Some(1);
        let record = listed.conversation().record(&listed.models).expect("started");
        assert_eq!(record.model_account, Some(home));
        let mut s = Session::launching();
        let conversation = crate::state::Conversation::from_record(record, &s.models);
        assert!(conversation.record(&s.models).is_some_and(|r| r.model_account == Some(home)), "kept while waiting");
        let id = conversation.id;
        s.conversations.push(conversation);
        s.apply(Action::Server(ServerEvent::Models(listed.models.clone())), 0.0);
        let kept = s.conversations.iter().find(|c| c.id == id).expect("kept");
        assert_eq!(s.models[kept.model].account, Some(home));
    }

    #[test]
    fn unimplemented_engines_cannot_be_selected() {
        let mut s = Session { engines: engines(), ..Session::default() };
        s.models.push(ModelOption::new(EngineKind::Codex, "default", "Codex"));
        let codex = s.models.len() - 1;
        assert!(!s.is_model_selectable(codex));
        s.apply(Action::SelectModel(codex), 1.0);
        assert_ne!(s.model().engine, EngineKind::Codex);
        assert!(s.is_model_selectable(0));
    }

    #[test]
    fn an_unavailable_native_runtime_raises_a_notice_up_front() {
        let mut s = Session::default();
        s.set_engines(vec![info(EngineKind::Native, false, true, &["opus"])]);
        assert!(s.notice_text().as_deref().is_some_and(|n| n.contains("Restart Superfik")));
        assert!(s.is_model_selectable(0));
        s.apply(Action::DismissNotice, 0.0);
        assert!(s.notice.is_none());
    }

    #[test]
    fn grouped_by_account_before_engines_are_known() {
        let s = Session::default();
        assert_eq!(s.model_rows().len(), s.models.len() + 1, "one account, its models");
        assert!(s.is_model_selectable(0));
    }
}
