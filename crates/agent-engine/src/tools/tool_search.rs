use std::collections::BTreeSet;
use std::sync::Mutex;

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

pub const TOOL_SEARCH: &str = "ToolSearch";
const DEFER_OVER: usize = 20;
const FOUND: usize = 5;

#[derive(Debug, Default)]
pub struct Deferred {
    specs: Vec<ToolSpec>,
    loaded: Mutex<BTreeSet<String>>,
}

impl Deferred {
    pub fn loaded(&self) -> Vec<ToolSpec> {
        let loaded = self.loaded.lock().map(|loaded| loaded.clone()).unwrap_or_default();
        self.specs.iter().filter(|spec| loaded.contains(&spec.name)).cloned().collect()
    }

    pub fn only(&self, allowed: impl Fn(&str) -> bool) -> Self {
        let specs = self.specs.iter().filter(|spec| allowed(&spec.name)).cloned().collect();
        Self { specs, loaded: Mutex::default() }
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.lock().is_ok_and(|loaded| loaded.contains(name))
    }

    pub fn search(&self, query: &str) -> Vec<ToolSpec> {
        let found: Vec<&ToolSpec> = match query.strip_prefix("select:") {
            Some(names) => {
                let names: Vec<&str> = names.split(',').map(str::trim).collect();
                self.specs.iter().filter(|spec| names.contains(&spec.name.as_str())).collect()
            }
            None => {
                let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
                let mut scored: Vec<(usize, &ToolSpec)> = self
                    .specs
                    .iter()
                    .map(|spec| {
                        let text = format!("{} {}", spec.name, spec.description).to_lowercase();
                        (words.iter().filter(|word| text.contains(word.as_str())).count(), spec)
                    })
                    .filter(|(score, _)| *score > 0)
                    .collect();
                scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
                scored.into_iter().take(FOUND).map(|(_, spec)| spec).collect()
            }
        };
        if let Ok(mut loaded) = self.loaded.lock() {
            loaded.extend(found.iter().map(|spec| spec.name.clone()));
        }
        found.into_iter().cloned().collect()
    }
}

pub fn defer(specs: Vec<ToolSpec>) -> (Vec<ToolSpec>, Option<Deferred>) {
    let deferrable = |spec: &ToolSpec| {
        spec.name.starts_with("mcp__") && !spec.name.starts_with(concat!("mcp__", brand::command!(), "_"))
    };
    if specs.iter().filter(|spec| deferrable(spec)).count() <= DEFER_OVER {
        return (specs, None);
    }
    let (deferred, mut kept): (Vec<ToolSpec>, Vec<ToolSpec>) = specs.into_iter().partition(deferrable);
    let names: Vec<&str> = deferred.iter().map(|spec| spec.name.as_str()).collect();
    kept.push(ToolSearch::spec_for(&names));
    (kept, Some(Deferred { specs: deferred, loaded: Mutex::default() }))
}

#[derive(Debug)]
pub struct ToolSearch;

impl ToolSearch {
    fn spec_for(names: &[&str]) -> ToolSpec {
        ToolSpec {
            name: TOOL_SEARCH.into(),
            description: format!(
                "Load the schemas of tools that are listed here but not yet callable, then call them. Query \
                 `select:name1,name2` for tools you know by name, or keywords to find them. Not loaded yet: {}",
                names.join(", ")
            ),
            schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"],
                "additionalProperties": false
            }),
        }
    }
}

impl Tool for ToolSearch {
    fn spec(&self) -> ToolSpec {
        Self::spec_for(&[])
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "query").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let query = match required(input, "query") {
            Ok(query) => query,
            Err(error) => return error,
        };
        let Some(deferred) = &workspace.deferred else { return Outcome::error("Every tool is already loaded.") };
        let found = deferred.search(query);
        if found.is_empty() {
            return Outcome::error(format!("No tool matches \"{query}\"."));
        }
        let shown: Vec<Value> = found
            .iter()
            .map(|spec| json!({ "name": spec.name, "description": spec.description, "parameters": spec.schema }))
            .collect();
        Outcome::ok(format!(
            "Loaded; call them from now on:\n{}",
            serde_json::to_string_pretty(&shown).unwrap_or_default()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str, description: &str) -> ToolSpec {
        ToolSpec { name: name.into(), description: description.into(), schema: json!({"type": "object"}) }
    }

    #[test]
    fn many_mcp_tools_wait_until_searched_for() {
        let mut specs = vec![spec("Read", "read a file")];
        specs.extend((0..25).map(|n| spec(&format!("mcp__jira__tool{n}"), "a Jira thing")));
        specs.push(spec("mcp__jira__create_issue", "Create a Jira issue"));
        let (kept, deferred) = defer(specs);
        let deferred = deferred.expect("deferred");
        assert_eq!(kept.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Read", TOOL_SEARCH]);
        assert!(kept[1].description.contains("mcp__jira__create_issue"));
        assert!(deferred.loaded().is_empty());

        let found = deferred.search("create issue");
        assert_eq!(found.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["mcp__jira__create_issue"]);
        assert!(deferred.is_loaded("mcp__jira__create_issue"));
        assert_eq!(deferred.search("select:mcp__jira__tool3").len(), 1);
        assert_eq!(deferred.loaded().len(), 2);
    }

    #[test]
    fn a_sub_agent_loads_only_what_it_may_use_and_never_its_parent_s() {
        let mut specs = vec![spec("Read", "")];
        specs.extend((0..25).map(|n| spec(&format!("mcp__jira__tool{n}"), "")));
        specs.push(spec("mcp__github__delete_repo", "Delete a repository"));
        let (_, deferred) = defer(specs);
        let parent = deferred.expect("deferred");
        parent.search("select:mcp__github__delete_repo");
        let child = parent.only(|name| name.starts_with("mcp__jira__"));
        assert!(!child.is_loaded("mcp__github__delete_repo"), "the parent's loads stay the parent's");
        assert!(child.search("select:mcp__github__delete_repo").is_empty(), "not a tool it may use");
        assert_eq!(child.search("select:mcp__jira__tool1").len(), 1);
    }

    #[test]
    fn a_few_mcp_tools_stay_in_every_turn() {
        let specs = vec![spec("Read", ""), spec("mcp__a__b", "")];
        let (kept, deferred) = defer(specs);
        assert_eq!(kept.len(), 2);
        assert!(deferred.is_none());
    }
}
