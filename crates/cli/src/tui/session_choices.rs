use app::{Effort, Model, PermissionMode};
use domain::command::{RoleChange, mode_label};
use domain::model_roles::{ModelRole, ModelRoles};

use super::app::App;
use super::picker::{Picker, PickerKind};
use super::settings::AUTO_EFFORT;

const SESSION_MODEL: &str = "Session model";

pub fn saved_roles(settings: &serde_json::Value) -> ModelRoles {
    serde_json::from_value(settings["roles"].clone()).unwrap_or_default()
}

pub fn saved_fallbacks(settings: &serde_json::Value) -> Vec<String> {
    serde_json::from_value(settings["fallbacks"].clone()).unwrap_or_default()
}

pub fn output_styles() -> Vec<(String, String)> {
    let mut styles = vec![("default".to_owned(), format!("{}'s usual way: works, then reports", brand::NAME))];
    for (path, _) in content::builtin_files("styles/") {
        let Some(id) = path.strip_prefix("styles/").and_then(|name| name.strip_suffix(".js")) else { continue };
        let spec = content::value(path, "spec").unwrap_or_default();
        let about = spec["description"].as_str().unwrap_or_default().trim().to_owned();
        styles.push((id.to_owned(), about));
    }
    styles
}

fn mode_about(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "Ask before edits and commands",
        PermissionMode::AcceptEdits => "Edit files freely, ask before commands",
        PermissionMode::Plan => "Read and plan only",
        PermissionMode::Bypass => "Never ask (a safety check still grades risky actions)",
    }
}

fn effort_about(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "Quickest answers, least reasoning",
        Effort::Medium => "Balanced",
        Effort::High => "More reasoning for harder work",
        Effort::XHigh => "Extended reasoning",
        Effort::Max => "As much reasoning as the model allows",
    }
}

impl App {
    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.session.set_mode(mode);
        if let Some(run) = &self.turn.run {
            let _ = run.set_permission(mode);
        }
        self.saver.remember("mode", mode_label(mode));
    }

    pub(super) fn mode_picker(&mut self) {
        let modes = PermissionMode::ALL;
        let cursor = modes.iter().position(|mode| *mode == self.mode()).unwrap_or(0);
        let items = modes.iter().map(|mode| (mode_label(*mode).to_owned(), mode_about(*mode).to_owned())).collect();
        self.overlays.picker = Some(Picker { kind: PickerKind::Mode, title: "Permission mode".into(), items, cursor });
    }

    pub(super) fn switch_mode(&mut self, mode: PermissionMode) {
        self.set_mode(mode);
        self.transcript.notice(format!("Mode: {}", mode_label(mode)));
    }

    pub(super) fn model_picker(&mut self) {
        if self.catalog.models.is_empty() {
            return self.transcript.notice(concat!(
                "No models yet. Run `",
                brand::command!(),
                " login` to connect a provider."
            ));
        }
        let current = self.model().map(|m| m.id.clone());
        let models = &self.catalog.models;
        let items = models.iter().map(|m| (m.label.clone(), m.id.clone())).collect();
        let cursor = models.iter().position(|m| Some(&m.id) == current.as_ref()).unwrap_or(0);
        self.overlays.picker = Some(Picker { kind: PickerKind::Model, title: "Model".into(), items, cursor });
    }

    pub(super) fn set_model_named(&mut self, wanted: &str) {
        match self.catalog.models.iter().find(|m| m.is_named(wanted)).cloned() {
            Some(model) => self.set_model(model),
            None => self.transcript.error(format!("No model named {wanted}. /model lists them.")),
        }
    }

    pub fn set_model(&mut self, model: Model) {
        self.saver.remember("model", &model.id);
        self.transcript.notice(format!("Model: {}", model.label));
        match self.session.live_mut() {
            Some(session) => session.set_model(model),
            None => self.start_session(model),
        }
    }

    pub(super) fn effort_picker(&mut self) {
        let items = Effort::ALL.iter().map(|e| (e.cli_value().to_owned(), effort_about(*e).to_owned())).collect();
        let cursor = Effort::ALL.iter().position(|e| *e == self.session.effort()).unwrap_or(0);
        self.overlays.picker = Some(Picker { kind: PickerKind::Effort, title: "Effort".into(), items, cursor });
    }

    pub(super) fn set_effort_from(&mut self, effort: Option<Effort>) {
        let Some(effort) = effort else {
            return self.transcript.notice(format!("Effort: {}", self.effort_label()));
        };
        self.session.set_effort(effort);
        self.saver.remember("effort", effort.cli_value());
        self.transcript.notice(format!("Effort: {}", effort.cli_value()));
    }

    pub(super) fn set_auto_effort(&mut self) {
        self.session.set_auto_effort();
        self.saver.remember("effort", AUTO_EFFORT);
        self.transcript.notice(
            "Effort: auto. Each request picks the model and effort it needs; /effort <level> goes back to one level.",
        );
    }

    pub(super) fn output_style(&mut self, named: Option<String>) {
        let Some(name) = named else { return self.output_style_picker() };
        self.set_output_style(name.to_lowercase());
    }

    fn output_style_picker(&mut self) {
        let current = self.session.output_style().unwrap_or("default");
        let items = output_styles();
        let cursor = items.iter().position(|(id, _)| id == current).unwrap_or(0);
        self.overlays.picker =
            Some(Picker { kind: PickerKind::OutputStyle, title: "Output style".into(), items, cursor });
    }

    pub(super) fn set_output_style(&mut self, style: String) {
        self.saver.remember("outputStyle", &style);
        self.transcript.notice(format!("Output style: {style}, from the next message."));
        self.session.set_output_style(style);
    }

    pub(super) fn change_role(&mut self, role: ModelRole, change: RoleChange) {
        match change {
            RoleChange::Pick => self.role_picker(role),
            RoleChange::Clear => self.set_role(role, None),
            RoleChange::Set(wanted) => match self.catalog.models.iter().find(|m| m.is_named(&wanted)) {
                Some(model) => self.set_role(role, Some(model.id.clone())),
                None => self.transcript.error(format!("No model named {wanted}. /model lists them.")),
            },
        }
    }

    pub(super) fn role_picked(&mut self, role: ModelRole, cursor: usize) {
        let model = cursor.checked_sub(1).and_then(|index| self.catalog.models.get(index)).map(|m| m.id.clone());
        self.set_role(role, model);
    }

    pub(super) fn list_roles(&mut self) {
        let lines: Vec<String> = ModelRole::ALL
            .iter()
            .map(|role| format!("{}: {} · {}", role.name(), self.role_label(*role), role.about()))
            .collect();
        self.transcript.notice(format!("Model roles (/model <role> <model> sets one):\n{}", lines.join("\n")));
    }

    fn role_picker(&mut self, role: ModelRole) {
        let current = self.session.roles().get(role);
        let mut items = vec![(SESSION_MODEL.to_owned(), "use the session's model".to_owned())];
        items.extend(self.catalog.models.iter().map(|m| (m.label.clone(), m.id.clone())));
        let cursor = current.and_then(|id| self.catalog.models.iter().position(|m| m.id == id)).map_or(0, |at| at + 1);
        let title = format!("Model for {}: {}", role.name(), role.about());
        self.overlays.picker = Some(Picker { kind: PickerKind::Role(role), title, items, cursor });
    }

    fn set_role(&mut self, role: ModelRole, model: Option<String>) {
        self.session.set_role(role, model);
        let roles = serde_json::to_value(self.session.roles()).unwrap_or_default();
        self.saver.remember_value("roles", roles);
        self.transcript.notice(format!("{}: {}", role.name(), self.role_label(role)));
    }

    fn role_label(&self, role: ModelRole) -> String {
        match self.session.roles().get(role) {
            Some(id) => {
                self.catalog.models.iter().find(|m| m.id == id).map_or_else(|| id.to_owned(), |m| m.label.clone())
            }
            None => SESSION_MODEL.to_lowercase(),
        }
    }

    pub(super) fn change_fallbacks(&mut self, change: RoleChange) {
        match change {
            RoleChange::Pick => self.show_fallbacks(),
            RoleChange::Clear => self.set_fallbacks(Vec::new()),
            RoleChange::Set(wanted) => {
                let mut chain = Vec::new();
                for name in wanted.split_whitespace() {
                    match self.catalog.models.iter().find(|m| m.is_named(name)) {
                        Some(model) => chain.push(model.id.clone()),
                        None => return self.transcript.error(format!("No model named {name}. /model lists them.")),
                    }
                }
                self.set_fallbacks(chain);
            }
        }
    }

    fn set_fallbacks(&mut self, chain: Vec<String>) {
        self.session.set_fallbacks(chain);
        self.saver.remember_value("fallbacks", serde_json::json!(self.session.fallbacks()));
        self.show_fallbacks();
    }

    fn show_fallbacks(&mut self) {
        if self.session.fallbacks().is_empty() {
            return self.transcript.notice(
                "Fallbacks: none from this session (the settings' fallbackModel still applies). \
                 /model fallbacks <model> [model…] sets the chain tried when a model is busy, signed out or unreachable.",
            );
        }
        let models = &self.catalog.models;
        let label = |id: &String| models.iter().find(|m| &m.id == id).map_or_else(|| id.clone(), |m| m.label.clone());
        let labels: Vec<String> = self.session.fallbacks().iter().map(label).collect();
        self.transcript.notice(format!("Fallbacks: {}", labels.join(" → ")));
    }
}
