//! What the `LSP` tool asks, and the protocol request that asks it.

use serde_json::{Value, json};

/// The questions the agent can ask a language server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Definition,
    References,
    Hover,
    DocumentSymbol,
    WorkspaceSymbol,
    Implementation,
    IncomingCalls,
    OutgoingCalls,
}

/// The tool's input, checked: lines and columns count from 1, columns in
/// characters as `Read` shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub operation: Operation,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub query: String,
}

impl Query {
    pub fn parse(input: &Value) -> Result<Self, String> {
        let text = |name: &str| input.get(name).and_then(Value::as_str);
        let name = text("operation").unwrap_or_default();
        let operation = Operation::named(name).ok_or_else(|| {
            format!("Unknown operation `{name}`; use one of: {}.", Operation::NAMES.map(|(n, _)| n).join(", "))
        })?;
        let file = text("file_path").ok_or("Missing required parameter `file_path`.")?.to_owned();
        let query = text("query").unwrap_or_default().to_owned();
        if operation == Operation::WorkspaceSymbol && query.trim().is_empty() {
            return Err("workspaceSymbol needs a `query`: the name to search for.".into());
        }
        let (line, column) = if operation.at_position() {
            let number = |name: &str| -> Result<u32, String> {
                let value =
                    input.get(name).and_then(Value::as_u64).ok_or(format!("{name} needs `line` and `character`."))?;
                u32::try_from(value).ok().filter(|n| *n >= 1).ok_or(format!("`{name}` counts from 1."))
            };
            (number("line")?, number("character")?)
        } else {
            (1, 1)
        };
        Ok(Self { operation, file, line, column, query })
    }

    /// The method and params to send; `line_text` is the file's line at
    /// `self.line`, used to count the column in UTF-16 as LSP does.
    pub fn request(&self, uri: &str, line_text: &str) -> (&'static str, Value) {
        let document = json!({"uri": uri});
        let character: usize = line_text.chars().take(self.column as usize - 1).map(char::len_utf16).sum();
        let at = json!({"textDocument": document, "position": {"line": self.line - 1, "character": character}});
        match self.operation {
            Operation::Definition => ("textDocument/definition", at),
            Operation::Implementation => ("textDocument/implementation", at),
            Operation::Hover => ("textDocument/hover", at),
            Operation::References => {
                let mut params = at;
                params["context"] = json!({"includeDeclaration": false});
                ("textDocument/references", params)
            }
            Operation::IncomingCalls | Operation::OutgoingCalls => ("textDocument/prepareCallHierarchy", at),
            Operation::DocumentSymbol => ("textDocument/documentSymbol", json!({"textDocument": document})),
            Operation::WorkspaceSymbol => ("workspace/symbol", json!({"query": self.query})),
        }
    }
}

impl Query {
    /// Calls are asked in two steps: after `prepareCallHierarchy` answers
    /// `prepared`, the calls of its first item.
    pub fn follow_up(&self, prepared: &Value) -> Option<(&'static str, Value)> {
        let method = match self.operation {
            Operation::IncomingCalls => "callHierarchy/incomingCalls",
            Operation::OutgoingCalls => "callHierarchy/outgoingCalls",
            _ => return None,
        };
        let item = prepared.as_array()?.first()?;
        Some((method, json!({"item": item})))
    }
}

impl Operation {
    pub const NAMES: [(&'static str, Self); 8] = [
        ("goToDefinition", Self::Definition),
        ("findReferences", Self::References),
        ("hover", Self::Hover),
        ("documentSymbol", Self::DocumentSymbol),
        ("workspaceSymbol", Self::WorkspaceSymbol),
        ("goToImplementation", Self::Implementation),
        ("incomingCalls", Self::IncomingCalls),
        ("outgoingCalls", Self::OutgoingCalls),
    ];

    fn named(name: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(n, _)| *n == name).map(|(_, operation)| *operation)
    }

    fn at_position(self) -> bool {
        !matches!(self, Self::DocumentSymbol | Self::WorkspaceSymbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_counted_from_one_in_characters_is_sent_from_zero_in_utf16() {
        let query = Query::parse(&json!({"operation": "hover", "file_path": "src/a.rs", "line": 42, "character": 8}))
            .expect("query");
        let (method, params) = query.request("file:///w/src/a.rs", "x(\"😀\", foo)");
        assert_eq!(method, "textDocument/hover");
        assert_eq!(
            params,
            json!({"textDocument": {"uri": "file:///w/src/a.rs"}, "position": {"line": 41, "character": 8}})
        );
    }

    #[test]
    fn references_leave_out_the_declaration_and_symbols_need_no_position() {
        let at = |operation: &str| json!({"operation": operation, "file_path": "a.rs", "line": 1, "character": 1});
        let references = Query::parse(&at("findReferences")).expect("references");
        assert_eq!(references.request("file:///a.rs", "f").1["context"], json!({"includeDeclaration": false}));
        let symbols = Query::parse(&json!({"operation": "documentSymbol", "file_path": "a.rs"})).expect("symbols");
        assert_eq!(symbols.request("file:///a.rs", "").0, "textDocument/documentSymbol");
        let search = json!({"operation": "workspaceSymbol", "file_path": "a.rs", "query": "Tool"});
        let search = Query::parse(&search).expect("workspace symbols");
        assert_eq!(search.request("file:///a.rs", ""), ("workspace/symbol", json!({"query": "Tool"})));
        assert_eq!(
            Query::parse(&at("incomingCalls")).expect("calls").request("u", "f").0,
            "textDocument/prepareCallHierarchy"
        );
    }

    #[test]
    fn calls_are_asked_about_the_first_prepared_item() {
        let at = |operation: &str| json!({"operation": operation, "file_path": "a.rs", "line": 3, "character": 4});
        let item = json!({"name": "main", "kind": 12, "uri": "file:///a.rs"});
        let incoming = Query::parse(&at("incomingCalls")).expect("incoming");
        assert_eq!(incoming.follow_up(&json!([item])), Some(("callHierarchy/incomingCalls", json!({"item": item}))));
        let outgoing = Query::parse(&at("outgoingCalls")).expect("outgoing");
        assert_eq!(outgoing.follow_up(&json!([item])).map(|(method, _)| method), Some("callHierarchy/outgoingCalls"));
        assert_eq!(outgoing.follow_up(&json!(null)), None);
        assert_eq!(Query::parse(&at("hover")).expect("hover").follow_up(&json!([item])), None);
    }

    #[test]
    fn a_question_missing_what_it_needs_says_so() {
        let error = |input: Value| Query::parse(&input).expect_err("rejected");
        assert!(error(json!({"operation": "hover", "file_path": "a.rs"})).contains("line"));
        assert!(
            error(json!({"operation": "hover", "file_path": "a.rs", "line": 0, "character": 1})).contains("from 1")
        );
        assert!(error(json!({"operation": "workspaceSymbol", "file_path": "a.rs"})).contains("query"));
        assert!(error(json!({"operation": "rename", "file_path": "a.rs"})).contains("goToDefinition"));
        assert!(error(json!({"operation": "hover"})).contains("file_path"));
    }
}
