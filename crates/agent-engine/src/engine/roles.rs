use domain::model_roles::{ModelRole, ModelRoles};

use crate::prompt::settings::Settings;

pub fn merged(asked: &ModelRoles, settings: &Settings) -> ModelRoles {
    let mut roles = ModelRoles::default();
    roles.set(ModelRole::Small, settings.small_model.clone());
    roles.set(ModelRole::Review, settings.judge.clone());
    roles.set(ModelRole::Advisor, settings.advisor.clone());
    for (role, model) in asked.iter() {
        roles.set(role, Some(model.to_owned()));
    }
    roles
}

pub fn fallbacks(session: &domain::SessionConfig, settings: &[String]) -> Vec<String> {
    match (&session.fallback_model, session.fallbacks.is_empty()) {
        (Some(model), _) => vec![model.clone()],
        (None, false) => session.fallbacks.clone(),
        (None, true) => settings.to_vec(),
    }
}

pub fn elsewhere(model: &str, current: domain::ModelProvider) -> bool {
    let known: Vec<&str> = crate::providers::all().iter().map(|provider| provider.id()).collect();
    domain::routing::route(model, &known).provider.is_some_and(|provider| provider != current.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_session_asks_for_wins_over_the_settings_files() {
        let settings = Settings {
            small_model: Some("haiku".into()),
            judge: Some("judge".into()),
            advisor: Some("advisor".into()),
            ..Settings::default()
        };
        let mut asked = ModelRoles::default();
        asked.set(ModelRole::Small, Some("grok-mini".into()));
        asked.set(ModelRole::Plan, Some("opus".into()));
        let roles = merged(&asked, &settings);
        let all: Vec<(ModelRole, &str)> = roles.iter().collect();
        assert_eq!(
            all,
            [
                (ModelRole::Plan, "opus"),
                (ModelRole::Small, "grok-mini"),
                (ModelRole::Review, "judge"),
                (ModelRole::Advisor, "advisor")
            ]
        );
    }
}
