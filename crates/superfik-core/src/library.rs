//! The organization's library: skills and custom agents its admins publish
//! from their own, which every member's runs can then use (Claude Code
//! names them `org:<name>`). Pure; the server keeps it, each desktop lays
//! it out as a plugin.

use serde::{Deserialize, Serialize};

pub const LIBRARY_PLUGIN: &str = "org";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Skill,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryItem {
    pub kind: LibraryKind,
    pub name: String,
    pub body: String,
    pub author_name: String,
}

pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn publish_target(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("/publish")?;
    let mut words = rest.split_whitespace();
    let name = words.next()?.trim_start_matches('/').to_owned();
    (rest.starts_with(char::is_whitespace) && words.next().is_none()).then_some(name)
}

pub fn upsert(items: &mut Vec<LibraryItem>, item: LibraryItem) {
    items.retain(|kept| !(kept.kind == item.kind && kept.name == item.name));
    items.push(item);
    items.sort_by(|a, b| (a.kind as u8, &a.name).cmp(&(b.kind as u8, &b.name)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: LibraryKind, name: &str, body: &str) -> LibraryItem {
        LibraryItem { kind, name: name.into(), body: body.into(), author_name: "Olga".into() }
    }

    #[test]
    fn names_are_plain() {
        assert!(valid_name("release-train") && valid_name("a1"));
        assert!(!valid_name("") && !valid_name("../etc") && !valid_name("Motion") && !valid_name(&"a".repeat(65)));
    }

    #[test]
    fn publish_names_one_skill_or_agent() {
        assert_eq!(publish_target("/publish motion"), Some("motion".into()));
        assert_eq!(publish_target("  /publish /motion "), Some("motion".into()));
        assert_eq!(publish_target("/publish"), None);
        assert_eq!(publish_target("/publish a b"), None);
        assert_eq!(publish_target("/publishing x"), None);
    }

    #[test]
    fn publishing_again_replaces_and_keeps_order() {
        let mut items = vec![item(LibraryKind::Agent, "reviewer", "a")];
        upsert(&mut items, item(LibraryKind::Skill, "motion", "1"));
        upsert(&mut items, item(LibraryKind::Skill, "motion", "2"));
        let names: Vec<(&str, &str)> = items.iter().map(|i| (i.name.as_str(), i.body.as_str())).collect();
        assert_eq!(names, [("motion", "2"), ("reviewer", "a")]);
    }
}
