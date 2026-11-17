use std::path::Path;

use agent_engine::prompt::{owner_settings, settings};
use serde_json::Value;

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let cwd = input["cwd"].as_str().ok_or("the flags feature needs the project's folder")?;
    let home = context.services.home().ok_or("no home folder to keep your settings in")?;
    let home = home.as_path();
    if let Some(name) = input["name"].as_str() {
        let on = input["on"].as_bool().ok_or("say whether the feature is on or off")?;
        owner_settings::keep_feature(home, name, on)?;
    }
    listed(Path::new(cwd), home)
}

pub fn forget(_client: u64) {}

fn listed(cwd: &Path, home: &Path) -> Result<Value, String> {
    let settings = settings::load(cwd, Some(home), &[])?;
    let listed = settings.features.listed();
    let listed = listed.into_iter().map(|flag| {
        let available = !flag.name.eq(domain::features::SANDBOX) || platform::sandbox::SUPPORTED;
        let mut flag = serde_json::to_value(flag).unwrap_or(Value::Null);
        flag["available"] = Value::Bool(available);
        flag
    });
    Ok(Value::Array(listed.collect()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::feature::testing;

    #[test]
    fn a_flag_is_listed_and_turned_off_in_the_owner_s_settings() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let mut services = testing::services(None);
        services.homes = std::sync::Arc::new(crate::OwnerHome::at(home.path()));
        let cwd = repo.path().display().to_string();

        let listed = testing::with(1, &services, |context| handle(context, json!({ "cwd": cwd }))).expect("listed");
        assert_eq!(listed[0]["name"], domain::features::SANDBOX);
        assert_eq!(listed[0]["on"], true);

        let set = json!({ "cwd": cwd, "name": domain::features::SANDBOX, "on": false });
        let listed = testing::with(1, &services, |context| handle(context, set)).expect("turned off");
        assert_eq!(listed[0]["on"], false);
        let kept = std::fs::read_to_string(owner_settings::path(home.path())).expect("settings");
        assert!(kept.contains("\"experimental-sandbox\": false"), "{kept}");

        let unknown = json!({ "cwd": cwd, "name": "nothing", "on": false });
        assert!(testing::with(1, &services, |context| handle(context, unknown)).is_err());
    }
}
