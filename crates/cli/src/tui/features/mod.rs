use std::any::Any;
use std::collections::HashMap;

use super::app::App;

pub type Run = fn(&mut App, Option<String>);

pub struct Slash {
    pub names: &'static [&'static str],
    pub about: &'static str,
    pub run: Run,
}

pub struct Feature {
    pub commands: Vec<Slash>,
    pub tick: Option<fn(&mut App)>,
    pub prefix: Option<(char, Run)>,
}

include!(concat!(env!("OUT_DIR"), "/features.rs"));

pub fn command(text: &str) -> Option<(Run, Option<String>)> {
    let text = text.trim().strip_prefix('/')?;
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let rest = rest.trim();
    let argument = (!rest.is_empty()).then(|| rest.to_owned());
    all()
        .into_iter()
        .flat_map(|feature| feature.commands)
        .find(|slash| slash.names.iter().any(|known| known.eq_ignore_ascii_case(name)))
        .map(|slash| (slash.run, argument))
}

pub fn prefixed(text: &str) -> Option<(Run, Option<String>)> {
    let first = text.chars().next()?;
    let rest = text[first.len_utf8()..].trim();
    let (_, run) = all().into_iter().filter_map(|feature| feature.prefix).find(|(sign, _)| *sign == first)?;
    (!rest.is_empty()).then(|| (run, Some(rest.to_owned())))
}

pub fn help() -> Vec<(&'static str, &'static str)> {
    all()
        .into_iter()
        .flat_map(|feature| feature.commands)
        .filter_map(|slash| slash.names.first().map(|name| (*name, slash.about)))
        .collect()
}

pub fn tick(app: &mut App) {
    for tick in all().into_iter().filter_map(|feature| feature.tick) {
        tick(app);
    }
}

#[derive(Default)]
#[allow(dead_code, reason = "the feature folders use it, and any of them may be removed")]
pub struct States(HashMap<&'static str, Box<dyn Any>>);

impl std::fmt::Debug for States {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

#[allow(dead_code, reason = "the feature folders use it, and any of them may be removed")]
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
    fn a_feature_command_is_found_by_any_of_its_names_with_its_argument() {
        for (name, _) in help() {
            let (_, argument) = command(&format!("/{name}  some words ")).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(argument.as_deref(), Some("some words"));
        }
        assert!(command("/no-such-command").is_none());
        assert!(command("plain text").is_none());
    }
}
