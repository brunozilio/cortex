use std::path::Path;

use crate::api::features::flags::{FlagsReply, FlagsRequest, ListedFlag};
use agent_engine::prompt::{owner_settings, settings};

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: FlagsRequest) -> Result<FlagsReply, String> {
    let home = context.services.home().ok_or("no home folder to keep your settings in")?;
    let cwd = match request {
        FlagsRequest::List { cwd } => cwd,
        FlagsRequest::Set { cwd, name, on } => {
            owner_settings::keep_feature(&home, &name, on)?;
            cwd
        }
    };
    listed(context, &cwd, &home)
}

pub fn forget(_client: u64) {}

fn listed(context: &Context<'_>, cwd: &Path, home: &Path) -> Result<FlagsReply, String> {
    let settings = settings::load(&context.services.extensions.trusts.gate(cwd, Some(home)), &[])?;
    context.services.off.follow(&settings.features);
    let flags = settings.features.listed().into_iter().map(|flag| {
        let available = !flag.name.eq(domain::features::SANDBOX) || platform::sandbox::supported();
        ListedFlag { flag, available }
    });
    Ok(FlagsReply { flags: flags.collect() })
}

#[cfg(test)]
mod tests {
    use crate::api::features::suggested_rules::SuggestedRulesRequest;
    use crate::api::features::workspace::{WorkspaceOp, WorkspaceReply, WorkspaceRequest};
    use crate::api::{FeatureError, FeatureName, FeatureReply, FeatureRequest};

    use super::*;
    use crate::features::context::testing;

    fn at_home(home: &Path) -> crate::LocalServices {
        let mut services = testing::services(None);
        services.homes = std::sync::Arc::new(crate::OwnerHome::at(home));
        services
    }

    #[test]
    fn a_flag_is_listed_and_turned_off_in_the_owner_s_settings() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let services = at_home(home.path());
        let cwd = repo.path().to_path_buf();
        let flags = |request: FlagsRequest| testing::with(1, &services, |context| handle(context, request));

        let listed = flags(FlagsRequest::List { cwd: cwd.clone() }).expect("listed").flags;
        assert_eq!((listed[0].flag.name.as_str(), listed[0].flag.on), (domain::features::SANDBOX, true));
        assert_eq!(listed[0].available, platform::sandbox::supported());

        let off = FlagsRequest::Set { cwd: cwd.clone(), name: domain::features::SANDBOX.into(), on: false };
        assert!(!flags(off).expect("turned off").flags[0].flag.on);
        let kept = std::fs::read_to_string(owner_settings::path(home.path())).expect("settings");
        assert!(kept.contains("\"experimental-sandbox\": false"), "{kept}");

        assert!(flags(FlagsRequest::Set { cwd, name: "nothing".into(), on: false }).is_err());
        assert!(testing::local(1, |context| handle(context, FlagsRequest::List { cwd: "/".into() })).is_err());
    }

    #[test]
    fn a_flag_turned_off_while_the_program_runs_turns_its_feature_off_in_the_backend_at_once() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let services = at_home(home.path());
        let cwd = repo.path().to_path_buf();
        let ask = |request: FeatureRequest| testing::with(1, &services, |c| crate::features::handle(c, request));
        let rules = || ask(FeatureRequest::SuggestedRules(SuggestedRulesRequest::List { cwd: cwd.clone() }));
        let offer = || {
            let request = WorkspaceRequest { path: cwd.clone(), op: WorkspaceOp::GitOffer };
            ask(FeatureRequest::Workspace(request))
        };
        let set = |name: &str, on: bool| {
            let request = FlagsRequest::Set { cwd: cwd.clone(), name: name.to_owned(), on };
            ask(FeatureRequest::Flags(request)).expect("set")
        };
        let offered = |offer: bool| Ok(FeatureReply::Workspace(WorkspaceReply::GitOffer { offer }));

        assert!(rules().is_ok(), "on until turned off");
        assert_eq!(offer(), offered(true));
        set(domain::features::SUGGESTED_RULES, false);
        set(domain::features::GIT_OFFER, false);
        assert_eq!(rules(), Err(FeatureError::FeatureOff { feature: FeatureName::SuggestedRules }));
        assert_eq!(offer(), offered(false), "what a feature asks about another flag follows it too");
        set(domain::features::SUGGESTED_RULES, true);
        assert!(rules().is_ok(), "and back on without a restart");
        assert_eq!(offer(), offered(false), "the other flag stays as it was set");
    }
}
