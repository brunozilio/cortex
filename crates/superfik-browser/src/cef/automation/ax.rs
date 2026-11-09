//! Native accessibility nodes, stable document-scoped refs, bounded output.
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(super) struct Node {
    pub backend: u64,
    pub session: String,
    pub frame: String,
}

#[derive(Default)]
pub(super) struct References {
    next: u32,
    nodes: HashMap<u32, Node>,
    reverse: HashMap<(String, u64), u32>,
    seen: HashSet<u32>,
}

impl References {
    pub fn begin(&mut self) {
        self.seen.clear();
    }
    pub fn invalidate(&mut self) {
        self.nodes.clear();
        self.reverse.clear();
    }
    pub fn node(&self, reference: u32) -> Result<Node, String> {
        self.nodes
            .get(&reference)
            .cloned()
            .ok_or_else(|| format!("Stale ref={reference}; take a new browser_snapshot."))
    }
    fn assign(&mut self, node: Node) -> Option<u32> {
        let key = (node.session.clone(), node.backend);
        let reference = if let Some(reference) = self.reverse.get(&key) {
            *reference
        } else {
            if self.nodes.len() >= 32_768 {
                return None;
            }
            self.next = self.next.checked_add(1)?;
            self.reverse.insert(key, self.next);
            self.nodes.insert(self.next, node);
            self.next
        };
        self.seen.insert(reference);
        Some(reference)
    }
    pub fn finish(&mut self) {
        if self.nodes.len() >= 32_768 {
            self.nodes.retain(|id, _| self.seen.contains(id));
            self.reverse.retain(|_, id| self.seen.contains(id));
        }
    }
    pub fn visible(&self, reference: u32) -> bool {
        self.seen.contains(&reference)
    }
    pub fn append(&mut self, result: &Value, session: &str, frame: &str, lines: &mut Vec<String>) {
        for node in result["nodes"].as_array().into_iter().flatten() {
            if node["ignored"].as_bool() == Some(true) {
                continue;
            }
            let role = node["role"]["value"].as_str().unwrap_or("");
            let name = node["name"]["value"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
            if role == "InlineTextBox" || (name.is_empty() && matches!(role, "generic" | "none")) {
                continue;
            }
            let actionable = matches!(
                role,
                "button"
                    | "link"
                    | "textbox"
                    | "searchbox"
                    | "combobox"
                    | "checkbox"
                    | "radio"
                    | "switch"
                    | "tab"
                    | "menuitem"
                    | "option"
                    | "slider"
                    | "spinbutton"
                    | "treeitem"
            );
            let reference = actionable
                .then(|| node["backendDOMNodeId"].as_u64())
                .flatten()
                .and_then(|backend| self.assign(Node { backend, session: session.into(), frame: frame.into() }));
            let label = serde_json::to_string(&name).unwrap_or_default();
            let mut line = match reference {
                Some(id) => format!("[ref={id} {role} {label}]"),
                None => format!("{role} {label}"),
            };
            for property in node["properties"].as_array().into_iter().flatten() {
                if matches!(
                    property["name"].as_str(),
                    Some("disabled" | "checked" | "selected" | "expanded" | "required")
                ) {
                    line.push_str(&format!(
                        " {}={}",
                        property["name"].as_str().unwrap_or(""),
                        property["value"]["value"]
                    ));
                }
            }
            lines.push(line);
        }
    }
}

pub(super) fn window(lines: &[String], offset: usize) -> String {
    let all = lines.join("\n");
    let total = all.chars().count();
    let start = offset.min(total);
    let content: String = all.chars().skip(start).take(crate::agent::SNAPSHOT_LIMIT).collect();
    let end = start + content.chars().count();
    if end < total {
        format!("{content}\n[More content: browser_snapshot offset={end}; total={total}]")
    } else {
        content
    }
}
