use domain::command::RoleChange;

use super::app::App;

pub fn saved(settings: &serde_json::Value) -> Vec<String> {
    serde_json::from_value(settings["fallbacks"].clone()).unwrap_or_default()
}

impl App {
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
        self.fallbacks = chain;
        if let Some(session) = &mut self.session {
            session.set_fallbacks(self.fallbacks.clone());
        }
        self.saver.remember_value("fallbacks", serde_json::json!(self.fallbacks));
        self.show_fallbacks();
    }

    fn show_fallbacks(&mut self) {
        if self.fallbacks.is_empty() {
            return self.transcript.notice(
                "Fallbacks: none from this session (the settings' fallbackModel still applies). \
                 /model fallbacks <model> [model…] sets the chain tried when a model is busy, signed out or unreachable.",
            );
        }
        let labels: Vec<String> = self
            .fallbacks
            .iter()
            .map(|id| self.catalog.models.iter().find(|m| &m.id == id).map_or_else(|| id.clone(), |m| m.label.clone()))
            .collect();
        self.transcript.notice(format!("Fallbacks: {}", labels.join(" → ")));
    }
}
