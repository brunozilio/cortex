use std::collections::BTreeMap;

use serde_json::Value;

pub const SANDBOX: &str = "experimental-sandbox";

pub const REDACT_SECRETS: &str = "redact-secrets";

const SECURITY: &[&str] = &[SANDBOX, REDACT_SECRETS];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    pub name: &'static str,
    pub about: &'static str,
}

pub const KNOWN: &[Flag] = &[
    Flag { name: SANDBOX, about: "Confine Bash and Monitor commands to the project's folders (macOS)" },
    Flag {
        name: REDACT_SECRETS,
        about: "Hide tokens, keys and the project's .env values in tool output before the model and the log see them",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Listed {
    pub name: String,
    pub about: String,
    pub on: bool,
    pub experimental: bool,
    pub security: bool,
}

pub fn toggle(argument: Option<&str>) -> Result<Option<(String, bool)>, String> {
    let words: Vec<&str> = argument.unwrap_or_default().split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(None),
        [name, "on"] => Ok(Some(((*name).to_owned(), true))),
        [name, "off"] => Ok(Some(((*name).to_owned(), false))),
        _ => Err("Use /features to list them, or /features <name> on|off.".into()),
    }
}

pub fn known(name: &str) -> Option<Flag> {
    KNOWN.iter().copied().find(|flag| flag.name == name)
}

pub fn with_flag(mut settings: Value, name: &str, on: bool) -> Result<Value, String> {
    let root = settings.as_object_mut().ok_or("the settings are not a JSON object")?;
    let features = root.entry("features").or_insert_with(|| Value::Object(Default::default()));
    let features = features.as_object_mut().ok_or("features must be an object of true or false")?;
    features.insert(name.to_owned(), Value::Bool(on));
    Ok(settings)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Features {
    set: BTreeMap<String, bool>,
}

impl Features {
    pub fn on(&self, feature: &str) -> bool {
        self.set.get(feature).copied().unwrap_or(true)
    }

    pub fn listed(&self) -> Vec<Listed> {
        KNOWN
            .iter()
            .map(|flag| Listed {
                name: flag.name.to_owned(),
                about: flag.about.to_owned(),
                on: self.on(flag.name),
                experimental: flag.name.starts_with("experimental-"),
                security: SECURITY.contains(&flag.name),
            })
            .collect()
    }

    pub fn off(&self) -> impl Iterator<Item = &str> {
        self.set.iter().filter(|(_, on)| !**on).map(|(name, _)| name.as_str())
    }

    pub fn layered(mut self, value: &Value, owners: bool) -> Result<Self, String> {
        let flags = value.as_object().ok_or("features must be an object of true or false")?;
        for (feature, flag) in flags {
            let flag = flag.as_bool().ok_or_else(|| format!("features.{feature} must be true or false"))?;
            if owners || flag == SECURITY.contains(&feature.as_str()) {
                self.set.insert(feature.clone(), flag);
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn every_feature_is_on_until_the_owner_turns_it_off() {
        let features = Features::default();
        assert!(features.on(SANDBOX) && features.on("insights"));

        let features = features.layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");
        assert!(!features.on(SANDBOX) && !features.on("insights"));
    }

    #[test]
    fn a_checked_in_file_only_restricts() {
        let owners = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");
        let back_on = owners.clone().layered(&json!({ "insights": true }), false).expect("flags");
        assert!(!back_on.on("insights"), "a project cannot turn on what the owner turned off");

        let project = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), false).expect("flags");
        assert!(project.on(SANDBOX), "a project cannot turn a security feature off");
        assert!(!project.on("insights"));

        let enforced = owners.layered(&json!({ SANDBOX: true }), false).expect("flags");
        assert!(enforced.on(SANDBOX), "a project may ask for a security feature");
    }

    #[test]
    fn the_known_flags_are_listed_with_their_state() {
        let features = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");

        let listed = features.listed();
        let sandbox = listed.iter().find(|flag| flag.name == SANDBOX).expect("sandbox is known");
        assert!(!sandbox.on && sandbox.experimental && sandbox.security);
        assert!(!listed.iter().any(|flag| flag.name == "insights"), "only known flags are listed");
        assert_eq!(features.off().collect::<Vec<_>>(), ["experimental-sandbox", "insights"]);
    }

    #[test]
    fn a_flag_is_written_into_the_settings_keeping_the_rest() {
        let settings = json!({ "model": "x", "features": { "insights": false } });

        let written = with_flag(settings, SANDBOX, false).expect("written");

        assert_eq!(written, json!({ "model": "x", "features": { "insights": false, SANDBOX: false } }));
        assert!(with_flag(json!([]), SANDBOX, true).is_err());
        assert!(with_flag(json!({ "features": 1 }), SANDBOX, true).is_err());
        assert_eq!(known(SANDBOX).map(|flag| flag.name), Some(SANDBOX));
        assert!(known("nothing").is_none());
    }

    #[test]
    fn the_command_lists_or_turns_one_flag_on_or_off() {
        assert_eq!(toggle(None), Ok(None));
        assert_eq!(toggle(Some("  ")), Ok(None));
        assert_eq!(toggle(Some("experimental-sandbox off")), Ok(Some((SANDBOX.into(), false))));
        assert_eq!(toggle(Some("insights on")), Ok(Some(("insights".into(), true))));
        assert!(toggle(Some("insights maybe")).is_err());
        assert!(toggle(Some("insights")).is_err());
    }

    #[test]
    fn a_flag_that_is_not_true_or_false_is_refused() {
        let error = Features::default().layered(&json!({ SANDBOX: "no" }), true).expect_err("refused");
        assert_eq!(error, "features.experimental-sandbox must be true or false");
        assert!(Features::default().layered(&json!([]), true).is_err());
    }
}
