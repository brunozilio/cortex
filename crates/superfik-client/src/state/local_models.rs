//! Local models: MLX checkpoints downloaded from Hugging Face by the
//! connected backend (`<org>/<name>`), then offered in the picker.
use super::{Effect, Notice, Session};
use superfik_protocol::{ClientCommand, DownloadState};

pub const LOCAL_SUGGESTIONS: [(&str, &str); 4] = [
    ("mlx-community/Qwen3-8B-4bit", "Qwen 3 · 8B"),
    ("mlx-community/Llama-3.2-3B-Instruct-4bit", "Llama 3.2 · 3B"),
    ("mlx-community/gemma-3-4b-it-4bit", "Gemma 3 · 4B"),
    ("mlx-community/DeepSeek-R1-0528-Qwen3-8B-4bit", "DeepSeek R1 · 8B"),
];

pub fn hugging_face_id(typed: &str) -> Option<String> {
    let id = typed.trim();
    let id = ["https://huggingface.co/", "http://huggingface.co/", "huggingface.co/", "hf.co/"]
        .iter()
        .find_map(|prefix| id.strip_prefix(prefix))
        .unwrap_or(id)
        .trim_end_matches('/');
    let parts: Vec<_> = id.split('/').collect();
    let part = |p: &&str| {
        !p.is_empty() && *p != "." && *p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    };
    (parts.len() == 2 && parts.iter().all(part)).then(|| id.to_owned())
}

impl Session {
    pub fn local_model_id(&self) -> Option<String> {
        hugging_face_id(self.plugins.local_model.text())
    }

    pub fn local_model_installed(&self) -> bool {
        let Some(id) = self.local_model_id() else { return false };
        self.models.iter().any(|m| m.provider == superfik_core::ModelProvider::Local && m.id == id)
    }

    pub fn local_model_downloadable(&self) -> bool {
        self.local_model_id().is_some_and(|id| {
            !self.local_model_installed() && !self.plugins.downloads.get(&id).is_some_and(DownloadState::is_running)
        })
    }

    pub fn local_model_choices(&self) -> Vec<(String, String, Option<usize>)> {
        let mut choices: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, m)| m.provider == superfik_core::ModelProvider::Local)
            .map(|(i, m)| (m.id.clone(), m.label.clone(), Some(i)))
            .collect();
        for (id, label) in LOCAL_SUGGESTIONS {
            if !choices.iter().any(|(listed, _, _)| listed == id) {
                choices.push((id.into(), label.into(), None));
            }
        }
        choices.push((String::new(), self.strings().t(superfik_i18n::Text::AiLocalOther), None));
        choices
    }

    pub(super) fn choose_local_model(&mut self, index: usize, now: f64) -> Vec<Effect> {
        let Some((id, _, installed)) = self.local_model_choices().get(index).cloned() else { return Vec::new() };
        self.plugins.local_model = superfik_ui::TextEdit::new(id);
        self.plugins.panel.local_picker = false;
        self.plugins.panel.local_focus = 0;
        if let Some(index) = installed { self.select_model(index, now) } else { Vec::new() }
    }

    pub(super) fn toggle_local_models(&mut self) -> Vec<Effect> {
        let open = !self.plugins.panel.local_models;
        self.plugins.panel = Default::default();
        self.plugins.panel.local_models = open;
        Vec::new()
    }

    pub(super) fn download_local_model(&mut self) -> Vec<Effect> {
        let Some(id) = self.local_model_id() else {
            self.notice = Some(Notice::Raw(self.strings().t(superfik_i18n::Text::AiLocalInvalidId)));
            return Vec::new();
        };
        if !self.local_model_downloadable() {
            return Vec::new();
        }
        self.plugins.downloads.insert(id.clone(), DownloadState::Started);
        vec![Effect::Command(Box::new(ClientCommand::DownloadModel { id }))]
    }

    pub(super) fn local_download_event(&mut self, id: String, state: DownloadState) -> Vec<Effect> {
        let done = state == DownloadState::Done;
        if let DownloadState::Failed(reason) = &state {
            self.notice = Some(Notice::Raw(format!("{id}: {reason}")));
        }
        self.plugins.downloads.insert(id, state);
        if done { vec![Effect::Command(Box::new(ClientCommand::ListModels))] } else { Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Action, EditKey, SettingsPane};
    use superfik_protocol::{ProviderEvent, ServerEvent};
    fn command(effects: &[Effect], check: impl Fn(&ClientCommand) -> bool) -> bool {
        effects.iter().any(|effect| matches!(effect, Effect::Command(command) if check(command)))
    }
    fn downloads(effects: &[Effect], id: &str) -> bool {
        command(effects, |c| matches!(c, ClientCommand::DownloadModel { id: sent } if sent == id))
    }

    #[test]
    fn hugging_face_ids_are_org_and_name_or_their_page() {
        assert_eq!(hugging_face_id(" mlx-community/Qwen3-8B-4bit ").as_deref(), Some("mlx-community/Qwen3-8B-4bit"));
        let page = "https://huggingface.co/mlx-community/gemma-3-4b-it-4bit/";
        assert_eq!(hugging_face_id(page).as_deref(), Some("mlx-community/gemma-3-4b-it-4bit"));
        for wrong in ["", "qwen3:8b", "deepseek-r1:8b", "org/model/extra", "../model", "org/a b", "/model"] {
            assert_eq!(hugging_face_id(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_typed_model_downloads_from_hugging_face_and_refreshes_the_models() {
        let mut s = Session::default();
        s.apply(Action::OpenSettings, 0.0);
        let effects = s.apply(Action::ShowSettingsPane(SettingsPane::Ai), 0.0);
        assert!(command(&effects, |c| matches!(c, ClientCommand::ListModels)));
        s.apply(Action::ToggleLocalModels, 0.0);
        assert!(!s.local_model_downloadable(), "nothing typed yet");
        s.apply(Action::Edit(EditKey::Insert("mlx-community/example".into())), 0.0);
        assert!(s.conversation().composer.is_empty(), "typing never reaches the conversation");
        assert!(s.local_model_downloadable());
        let effects = s.apply(Action::Submit, 0.0);
        assert!(downloads(&effects, "mlx-community/example"));
        assert!(!s.local_model_downloadable(), "downloading");
        assert!(s.apply(Action::DownloadLocalModel, 0.0).is_empty(), "do not start a duplicate download");
        for state in [DownloadState::Preparing, DownloadState::Fetching { done: 5, total: Some(10) }] {
            let going = ProviderEvent::Download { id: "mlx-community/example".into(), state: state.clone() };
            assert!(s.apply(Action::Server(ServerEvent::Providers(going)), 0.0).is_empty());
            assert_eq!(s.plugins.downloads.get("mlx-community/example"), Some(&state));
            assert!(!s.local_model_downloadable(), "still downloading");
        }
        let done = ProviderEvent::Download { id: "mlx-community/example".into(), state: DownloadState::Done };
        let effects = s.apply(Action::Server(ServerEvent::Providers(done)), 0.0);
        assert!(command(&effects, |c| matches!(c, ClientCommand::ListModels)));
    }

    #[test]
    fn a_suggestion_fills_the_field_with_its_repository() {
        let mut s = Session::default();
        let choices = s.local_model_choices();
        let index = choices.iter().position(|(id, _, _)| id == "mlx-community/Qwen3-8B-4bit").expect("suggested");
        s.apply(Action::ChooseLocalModel(index), 0.0);
        assert_eq!(s.plugins.local_model.text(), "mlx-community/Qwen3-8B-4bit");
        assert!(downloads(&s.apply(Action::DownloadLocalModel, 0.0), "mlx-community/Qwen3-8B-4bit"));
    }

    #[test]
    fn downloaded_models_are_selected_not_downloaded_again() {
        let mut s = Session::default();
        s.models.push(
            superfik_core::ModelOption::new(superfik_core::EngineKind::Native, "me/my-model-4bit", "My model")
                .served_by(superfik_core::ModelProvider::Local),
        );
        let choices = s.local_model_choices();
        let index = choices.iter().position(|(id, _, _)| id == "me/my-model-4bit").expect("offered");
        s.apply(Action::ChooseLocalModel(index), 0.0);
        assert_eq!(s.model().id, "me/my-model-4bit");
        assert!(!s.local_model_downloadable());
        assert!(s.apply(Action::DownloadLocalModel, 0.0).is_empty());
    }

    #[test]
    fn invalid_ids_are_not_sent_and_failed_downloads_can_be_retried() {
        let mut s = Session::default();
        s.plugins.local_model = superfik_ui::TextEdit::new("deepseek-r1:8b");
        assert!(!s.local_model_downloadable());
        assert!(s.apply(Action::DownloadLocalModel, 0.0).is_empty());
        assert!(s.notice.is_some(), "says what an id looks like");
        s.plugins.local_model = superfik_ui::TextEdit::new("org/model");
        s.apply(Action::DownloadLocalModel, 0.0);
        let failed =
            ProviderEvent::Download { id: "org/model".into(), state: DownloadState::Failed("disk full".into()) };
        s.apply(Action::Server(ServerEvent::Providers(failed)), 0.0);
        assert!(matches!(&s.notice, Some(Notice::Raw(message)) if message.contains("disk full")));
        assert!(downloads(&s.apply(Action::DownloadLocalModel, 0.0), "org/model"));
    }
}
