use domain::command::RoleChange;
use domain::model_roles::{ModelRole, ModelRoles};

use super::app::App;
use super::choices::{Picker, PickerKind};

const SESSION_MODEL: &str = "Session model";

pub fn saved(settings: &serde_json::Value) -> ModelRoles {
    serde_json::from_value(settings["roles"].clone()).unwrap_or_default()
}

impl App {
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
        let current = self.roles.get(role);
        let mut items = vec![(SESSION_MODEL.to_owned(), "use the session's model".to_owned())];
        items.extend(self.catalog.models.iter().map(|m| (m.label.clone(), m.id.clone())));
        let cursor = current.and_then(|id| self.catalog.models.iter().position(|m| m.id == id)).map_or(0, |at| at + 1);
        let title = format!("Model for {}: {}", role.name(), role.about());
        self.overlays.picker = Some(Picker { kind: PickerKind::Role(role), title, items, cursor });
    }

    fn set_role(&mut self, role: ModelRole, model: Option<String>) {
        self.roles.set(role, model);
        if let Some(session) = &mut self.session {
            session.set_roles(self.roles.clone());
        }
        let roles = serde_json::to_value(&self.roles).unwrap_or_default();
        self.saver.remember_value("roles", roles);
        self.transcript.notice(format!("{}: {}", role.name(), self.role_label(role)));
    }

    fn role_label(&self, role: ModelRole) -> String {
        match self.roles.get(role) {
            Some(id) => {
                self.catalog.models.iter().find(|m| m.id == id).map_or_else(|| id.to_owned(), |m| m.label.clone())
            }
            None => SESSION_MODEL.to_lowercase(),
        }
    }
}
