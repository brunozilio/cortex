mod channels;
mod code_map;
pub mod context;
mod declared_providers;
mod flags;
pub(crate) mod memory;
mod output_savings;
mod plugins;
mod schedules;
mod sessions;
mod shell;
mod suggested_rules;
pub(crate) mod trace;
mod workspace;

use crate::api::{FeatureError, FeatureName, FeatureReply, FeatureRequest};
use domain::{PromptRequest, RunId};

use crate::features::context::{Context, Finish};

type Forget = fn(u64);
type Prepare = fn(&Context<'_>, RunId, PromptRequest) -> (PromptRequest, Option<Finish>);

const ALL: [(FeatureName, Forget); 13] = [
    (FeatureName::Channels, channels::forget),
    (FeatureName::CodeMap, code_map::forget),
    (FeatureName::DeclaredProviders, declared_providers::forget),
    (FeatureName::Flags, flags::forget),
    (FeatureName::Memory, memory::forget),
    (FeatureName::OutputSavings, output_savings::forget),
    (FeatureName::Plugins, plugins::forget),
    (FeatureName::Schedules, schedules::forget),
    (FeatureName::Sessions, sessions::forget),
    (FeatureName::Shell, shell::forget),
    (FeatureName::SuggestedRules, suggested_rules::forget),
    (FeatureName::Trace, trace::forget),
    (FeatureName::Workspace, workspace::forget),
];

const PREPARING: [(FeatureName, Prepare); 3] = [
    (FeatureName::Channels, channels::run::prepare),
    (FeatureName::CodeMap, code_map::run::prepare),
    (FeatureName::Memory, memory::run::prepare),
];

fn turned_off(context: &Context<'_>, feature: FeatureName) -> bool {
    feature.switch().is_some_and(|flag| !context.on(flag))
}

pub fn handle(context: &Context<'_>, request: FeatureRequest) -> Result<FeatureReply, FeatureError> {
    let feature = request.feature();
    if turned_off(context, feature) {
        return Err(FeatureError::FeatureOff { feature });
    }
    answer(context, request).map_err(|message| FeatureError::Failed { feature, message })
}

fn answer(context: &Context<'_>, request: FeatureRequest) -> Result<FeatureReply, String> {
    match request {
        FeatureRequest::Channels(request) => channels::handle(context, request).map(FeatureReply::Channels),
        FeatureRequest::CodeMap(request) => code_map::handle(context, request).map(FeatureReply::CodeMap),
        FeatureRequest::DeclaredProviders(request) => {
            declared_providers::handle(context, request).map(FeatureReply::DeclaredProviders)
        }
        FeatureRequest::Flags(request) => flags::handle(context, request).map(FeatureReply::Flags),
        FeatureRequest::Memory(request) => memory::handle(context, request).map(FeatureReply::Memory),
        FeatureRequest::OutputSavings => output_savings::handle(context).map(FeatureReply::OutputSavings),
        FeatureRequest::Plugins(request) => plugins::handle(context, request).map(FeatureReply::Plugins),
        FeatureRequest::Schedules(request) => schedules::handle(context, request).map(FeatureReply::Schedules),
        FeatureRequest::Sessions(request) => sessions::handle(context, request).map(FeatureReply::Sessions),
        FeatureRequest::Shell(request) => shell::handle(context, request).map(FeatureReply::Shell),
        FeatureRequest::SuggestedRules(request) => {
            suggested_rules::handle(context, request).map(FeatureReply::SuggestedRules)
        }
        FeatureRequest::Trace(request) => trace::handle(context, request).map(FeatureReply::Trace),
        FeatureRequest::Workspace(request) => workspace::handle(context, request).map(FeatureReply::Workspace),
    }
}

pub fn forget(client: u64) {
    for (_, forget) in ALL {
        forget(client);
    }
}

pub fn prepare(context: &Context<'_>, run: RunId, request: PromptRequest) -> (PromptRequest, Vec<Finish>) {
    let mut request = request;
    let mut finishes = Vec::new();
    for (feature, prepare) in PREPARING {
        if !turned_off(context, feature) {
            let (prepared, finish) = prepare(context, run, request);
            request = prepared;
            finishes.extend(finish);
        }
    }
    (request, finishes)
}

#[cfg(test)]
mod requests_tests;
#[cfg(test)]
mod switch_tests;

#[cfg(test)]
mod tests {
    use super::{ALL, PREPARING};

    #[test]
    fn the_registered_features_are_these_in_this_order() {
        let features: Vec<crate::api::FeatureName> = ALL.iter().map(|(feature, _)| *feature).collect();
        assert_eq!(features, crate::api::FeatureName::ALL, "one entry per feature the protocol names");
        let names: Vec<&str> = features.iter().map(|feature| feature.name()).collect();
        assert_eq!(
            names,
            [
                "channels",
                "code_map",
                "declared_providers",
                "flags",
                "memory",
                "output_savings",
                "plugins",
                "schedules",
                "sessions",
                "shell",
                "suggested_rules",
                "trace",
                "workspace",
            ]
        );
    }

    #[test]
    fn these_features_prepare_a_run_in_this_order() {
        let names: Vec<&str> = PREPARING.iter().map(|(feature, _)| feature.name()).collect();
        assert_eq!(names, ["channels", "code_map", "memory"]);
    }
}
