use std::path::Path;
use std::sync::Arc;

use crate::api::features::channels::ChannelsRequest;
use crate::api::features::code_map::CodeMapRequest;
use crate::api::features::declared_providers::DeclaredProvidersRequest;
use crate::api::features::flags::FlagsRequest;
use crate::api::features::memory::{MemoryOp, MemoryRequest};
use crate::api::features::plugins::PluginsRequest;
use crate::api::features::schedules::SchedulesRequest;
use crate::api::features::sessions::SessionsRequest;
use crate::api::features::shell::ShellRequest;
use crate::api::features::suggested_rules::SuggestedRulesRequest;
use crate::api::features::trace::TraceRequest;
use crate::api::features::workspace::{WorkspaceOp, WorkspaceRequest};
use crate::api::{FeatureError, FeatureName, FeatureReply, FeatureRequest};
use crate::features::context::testing;
use crate::services::LocalServices;

const CLIENT: u64 = 96_301;

fn a_request_to(feature: FeatureName, cwd: &Path) -> FeatureRequest {
    let path = cwd.to_path_buf();
    match feature {
        FeatureName::Channels => FeatureRequest::Channels(ChannelsRequest { path, names: Vec::new() }),
        FeatureName::CodeMap => FeatureRequest::CodeMap(CodeMapRequest { path, prompt: "retry".into() }),
        FeatureName::DeclaredProviders => FeatureRequest::DeclaredProviders(DeclaredProvidersRequest { path }),
        FeatureName::Flags => FeatureRequest::Flags(FlagsRequest::List { cwd: path }),
        FeatureName::Memory => FeatureRequest::Memory(MemoryRequest { path, op: MemoryOp::List }),
        FeatureName::OutputSavings => FeatureRequest::OutputSavings,
        FeatureName::Plugins => FeatureRequest::Plugins(PluginsRequest::List),
        FeatureName::Schedules => FeatureRequest::Schedules(SchedulesRequest::List),
        FeatureName::Sessions => FeatureRequest::Sessions(SessionsRequest::List),
        FeatureName::Shell => FeatureRequest::Shell(ShellRequest::List),
        FeatureName::SuggestedRules => FeatureRequest::SuggestedRules(SuggestedRulesRequest::List { cwd: path }),
        FeatureName::Trace => FeatureRequest::Trace(TraceRequest::EverySession),
        FeatureName::Workspace => FeatureRequest::Workspace(WorkspaceRequest { path, op: WorkspaceOp::GitOffer }),
    }
}

fn asked(services: &LocalServices, request: FeatureRequest) -> Result<FeatureReply, FeatureError> {
    testing::with(CLIENT, services, |context| super::handle(context, request))
}

fn is_off(services: &LocalServices, feature: FeatureName, cwd: &Path) -> Option<String> {
    match asked(services, a_request_to(feature, cwd)) {
        Err(off @ FeatureError::FeatureOff { .. }) => Some(off.to_string()),
        _ => None,
    }
}

fn typed(services: &LocalServices, cwd: &Path, command: &str) -> Result<(), String> {
    let argument = command.strip_prefix("/features ").ok_or_else(|| format!("{command} is not /features"))?;
    let (name, on) = domain::features::toggle(Some(argument))?.ok_or("nothing to toggle")?;
    let set = FlagsRequest::Set { cwd: cwd.to_path_buf(), name, on };
    asked(services, FeatureRequest::Flags(set)).map(|_| ()).map_err(|error| error.to_string())
}

fn the_command_in(said: &str) -> &str {
    let from = said.find("(/features ").map_or(said.len(), |at| at + 1);
    said[from..].trim_end_matches(')')
}

fn at_home(home: &Path, features: &serde_json::Value) -> LocalServices {
    std::fs::create_dir_all(home.join(brand::DIR)).expect("dir");
    let settings = serde_json::json!({ "features": features });
    std::fs::write(home.join(brand::DIR).join("settings.json"), settings.to_string()).expect("write");
    let mut services = testing::services(None);
    services.homes = Arc::new(crate::OwnerHome::at(home));
    services
}

#[test]
fn whatever_a_turned_off_feature_says_to_type_turns_it_back_on() {
    let (home, repo) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("repo"));
    let every_folder: serde_json::Map<String, serde_json::Value> =
        FeatureName::ALL.iter().map(|feature| (feature.name().to_owned(), false.into())).collect();
    let services = at_home(home.path(), &every_folder.into()).turned_off_in(repo.path());

    for feature in FeatureName::ALL {
        let Some(said) = is_off(&services, feature, repo.path()) else { continue };
        let command = the_command_in(&said);
        assert_eq!(typed(&services, repo.path(), command), Ok(()), "{feature} said: {said}");
        assert_eq!(is_off(&services, feature, repo.path()), None, "{command} did not turn {feature} back on");
    }
    super::forget(CLIENT);
}

#[test]
fn the_features_the_program_cannot_work_without_stay_on_whatever_the_settings_say() {
    let (home, repo) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("repo"));
    let named = serde_json::json!({
        "flags": false, "sessions": false, "shell": false, "workspace": false,
        "code_map": false, "code-map": false, "output_savings": false, "memory_search": false,
    });
    let services = at_home(home.path(), &named).turned_off_in(repo.path());

    for feature in FeatureName::ALL {
        assert_eq!(is_off(&services, feature, repo.path()), None, "{feature} has no switch by that name");
    }
    assert!(!services.off.contains("memory_search"), "what only the program turns off is not a setting");
    super::forget(CLIENT);
}

#[test]
fn every_declared_switch_turns_its_feature_off_and_no_other() {
    let (home, repo) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("repo"));
    let services = at_home(home.path(), &serde_json::json!({}));
    let always: Vec<FeatureName> = FeatureName::ALL.into_iter().filter(|feature| feature.switch().is_none()).collect();
    assert_eq!(
        always,
        [FeatureName::Flags, FeatureName::Memory, FeatureName::Sessions, FeatureName::Shell, FeatureName::Workspace]
    );

    for feature in FeatureName::ALL {
        let Some(switch) = feature.switch() else { continue };
        assert_eq!(typed(&services, repo.path(), &format!("/features {switch} off")), Ok(()), "{feature}");
        let off: Vec<FeatureName> =
            FeatureName::ALL.into_iter().filter(|other| is_off(&services, *other, repo.path()).is_some()).collect();
        assert_eq!(off, [feature], "{switch}");
        let said = is_off(&services, feature, repo.path()).unwrap_or_default();
        assert_eq!(said, format!("{feature} is turned off (/features {switch} on)"));
        assert_eq!(typed(&services, repo.path(), the_command_in(&said)), Ok(()));
    }
    super::forget(CLIENT);
}
