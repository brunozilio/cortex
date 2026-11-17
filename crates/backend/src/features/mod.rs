mod channels;
mod code_map;
mod declared_providers;
mod flags;
mod memory;
mod output_savings;
mod plugins;
mod schedules;
mod sessions;
mod shell;
mod suggested_rules;
mod trace;
mod workspace;

use domain::{PromptRequest, RunId};
use serde_json::Value;

use crate::feature::{Context, Finish};

type Handle = fn(&Context<'_>, Value) -> Result<Value, String>;
type Forget = fn(u64);
type Prepare = fn(&Context<'_>, RunId, PromptRequest) -> (PromptRequest, Option<Finish>);

const ALL: [(&str, Handle, Forget); 13] = [
    ("channels", channels::handle, channels::forget),
    ("code_map", code_map::handle, code_map::forget),
    ("declared_providers", declared_providers::handle, declared_providers::forget),
    ("flags", flags::handle, flags::forget),
    ("memory", memory::handle, memory::forget),
    ("output_savings", output_savings::handle, output_savings::forget),
    ("plugins", plugins::handle, plugins::forget),
    ("schedules", schedules::handle, schedules::forget),
    ("sessions", sessions::handle, sessions::forget),
    ("shell", shell::handle, shell::forget),
    ("suggested_rules", suggested_rules::handle, suggested_rules::forget),
    ("trace", trace::handle, trace::forget),
    ("workspace", workspace::handle, workspace::forget),
];

const PREPARING: [(&str, Prepare); 3] =
    [("channels", channels::run::prepare), ("code_map", code_map::run::prepare), ("memory", memory::run::prepare)];

pub fn handle(feature: &str, context: &Context<'_>, input: Value) -> Option<Result<Value, String>> {
    let (_, handle, _) = ALL.iter().find(|(name, _, _)| *name == feature && context.on(name))?;
    Some(handle(context, input))
}

pub fn forget(client: u64) {
    for (_, _, forget) in ALL {
        forget(client);
    }
}

pub fn prepare(context: &Context<'_>, run: RunId, request: PromptRequest) -> (PromptRequest, Vec<Finish>) {
    let mut request = request;
    let mut finishes = Vec::new();
    for (name, prepare) in PREPARING {
        if context.on(name) {
            let (prepared, finish) = prepare(context, run, request);
            request = prepared;
            finishes.extend(finish);
        }
    }
    (request, finishes)
}

#[cfg(test)]
mod tests {
    use super::{ALL, PREPARING};

    #[test]
    fn the_registered_features_are_these_in_this_order() {
        let names: Vec<&str> = ALL.iter().map(|(name, _, _)| *name).collect();
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
        let names: Vec<&str> = PREPARING.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["channels", "code_map", "memory"]);
    }
}
