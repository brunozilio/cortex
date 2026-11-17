mod branches;
mod budget;
mod cache_warning;
mod docs;
mod factory_reset;
mod flags;
mod learning;
mod learning_check;
mod memory;
mod output_savings;
mod outside_runs;
mod remote;
mod schedules;
mod shell;
mod suggested_rules;
pub(crate) mod trace;
pub(in crate::tui) mod trust;
mod verification;

use std::any::Any;
use std::collections::HashMap;

use std::sync::OnceLock;

use super::app::{App, Msg};

pub type Run = fn(&mut App, Option<String>);

pub struct Reply(Box<dyn FnOnce(&mut App) + Send>);

impl Reply {
    pub fn apply(self, app: &mut App) {
        (self.0)(app);
    }
}

fn reply(apply: impl FnOnce(&mut App) + Send + 'static) -> Msg {
    Msg::Feature(Reply(Box::new(apply)))
}

#[derive(Clone, Copy)]
pub struct Slash {
    pub names: &'static [&'static str],
    pub about: &'static str,
    pub run: Run,
}

#[derive(Default)]
pub struct Feature {
    pub commands: Vec<Slash>,
    pub tick: Option<fn(&mut App)>,
    pub prefix: Option<(char, Run)>,
    pub status: Option<fn(&App) -> Option<String>>,
    pub setting: Option<Setting>,
    pub answer: Option<fn(&mut App, &app::Decision) -> bool>,
}

pub fn answered(app: &mut App, decision: &app::Decision) -> bool {
    all().iter().filter_map(|feature| feature.answer).any(|answer| answer(app, decision))
}

#[derive(Clone, Copy)]
pub struct Setting {
    pub name: &'static str,
    pub value: fn(&App) -> String,
    pub run: Run,
}

pub fn status(app: &App) -> Vec<String> {
    all().iter().filter_map(|feature| feature.status).filter_map(|status| status(app)).collect()
}

pub fn on(app: &App, flag: &str) -> bool {
    !app.session.turned_off.contains(flag)
}

pub fn settings() -> Vec<Setting> {
    all().iter().filter_map(|feature| feature.setting).collect()
}

pub fn all() -> &'static [Feature] {
    static ALL: OnceLock<Vec<Feature>> = OnceLock::new();
    ALL.get_or_init(registered)
}

fn registered() -> Vec<Feature> {
    vec![
        branches::feature(),
        budget::feature(),
        cache_warning::feature(),
        docs::feature(),
        factory_reset::feature(),
        flags::feature(),
        learning::feature(),
        learning_check::feature(),
        memory::feature(),
        output_savings::feature(),
        outside_runs::feature(),
        remote::feature(),
        schedules::feature(),
        shell::feature(),
        suggested_rules::feature(),
        trace::feature(),
        trust::feature(),
        verification::feature(),
    ]
}

pub fn command(text: &str) -> Option<(Run, Option<String>)> {
    let text = text.trim().strip_prefix('/')?;
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let rest = rest.trim();
    let argument = (!rest.is_empty()).then(|| rest.to_owned());
    all()
        .iter()
        .flat_map(|feature| &feature.commands)
        .find(|slash| slash.names.iter().any(|known| known.eq_ignore_ascii_case(name)))
        .map(|slash| (slash.run, argument))
}

pub fn prefixed(text: &str) -> Option<(Run, Option<String>)> {
    let first = text.chars().next()?;
    let rest = text[first.len_utf8()..].trim();
    let (_, run) = all().iter().filter_map(|feature| feature.prefix).find(|(sign, _)| *sign == first)?;
    (!rest.is_empty()).then(|| (run, Some(rest.to_owned())))
}

pub fn help() -> Vec<(&'static str, &'static str)> {
    commands().into_iter().filter_map(|(names, about)| names.first().map(|name| (*name, about))).collect()
}

pub fn commands() -> Vec<(&'static [&'static str], &'static str)> {
    all().iter().flat_map(|feature| &feature.commands).map(|slash| (slash.names, slash.about)).collect()
}

pub fn tick(app: &mut App) {
    for tick in all().iter().filter_map(|feature| feature.tick) {
        tick(app);
    }
}

#[derive(Default)]
pub struct States(HashMap<&'static str, Box<dyn Any>>);

impl std::fmt::Debug for States {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl States {
    pub fn get<T: Default + 'static>(&mut self, key: &'static str) -> Option<&mut T> {
        let slot = self.0.entry(key).or_insert_with(|| Box::new(T::default()));
        if !slot.is::<T>() {
            *slot = Box::new(T::default());
        }
        slot.downcast_mut::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_is_kept_per_feature_and_type() {
        let mut states = States::default();
        if let Some(count) = states.get::<u32>("a") {
            *count += 2;
        }
        assert_eq!(states.get::<u32>("a").copied(), Some(2));
        assert_eq!(states.get::<String>("a").cloned(), Some(String::new()), "another type starts over");
        assert_eq!(states.get::<u32>("b").copied(), Some(0));
    }

    #[test]
    fn the_registered_features_bring_these_commands_in_this_order() {
        let listed: Vec<String> = all()
            .iter()
            .map(|feature| feature.commands.iter().map(|slash| slash.names.join("|")).collect::<Vec<_>>().join(" "))
            .collect();
        assert_eq!(
            listed,
            [
                "branch",
                "budget",
                "",
                "docs",
                "factory-reset",
                "features",
                "learn",
                "check",
                "memory",
                "gain",
                "",
                "remote",
                "schedules loop",
                "bashes",
                "rules",
                "trace|console",
                "trust",
                "verification",
            ],
            "one entry per feature folder, empty for cache_warning and outside_runs, which bring no command"
        );
    }

    #[test]
    fn a_feature_command_is_found_by_any_of_its_names_with_its_argument() {
        for (name, _) in help() {
            let (_, argument) = command(&format!("/{name}  some words ")).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(argument.as_deref(), Some("some words"));
        }
        assert!(command("/no-such-command").is_none());
        assert!(command("plain text").is_none());
    }
}
