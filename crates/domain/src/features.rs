use std::collections::BTreeMap;

use serde_json::Value;

pub const SANDBOX: &str = "experimental-sandbox";

const SECURITY: &[&str] = &[SANDBOX];

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Features {
    set: BTreeMap<String, bool>,
}

impl Features {
    pub fn on(&self, feature: &str) -> bool {
        self.set.get(feature).copied().unwrap_or(true)
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
    fn a_flag_that_is_not_true_or_false_is_refused() {
        let error = Features::default().layered(&json!({ SANDBOX: "no" }), true).expect_err("refused");
        assert_eq!(error, "features.experimental-sandbox must be true or false");
        assert!(Features::default().layered(&json!([]), true).is_err());
    }
}
