use std::path::Path;

use super::Workspace;

impl Workspace {
    pub fn callers(&self, path: &Path, before: &str, after: &str) -> String {
        if !self.blast_radius || !path.starts_with(&self.cwd) {
            return String::new();
        }
        let (Some(lsp), Some(lines)) = (self.language_servers(), domain::lsp::blast::changed_lines(before, after))
        else {
            return String::new();
        };
        lsp.callers(path, after, lines, &self.cwd, &self.stop).map(|block| format!("\n\n{block}")).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_callers_of_an_edit_are_told_only_while_the_flag_is_on() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("src/client.rs");
        let symbols = serde_json::json!([{
            "name": "retry", "kind": 12,
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 3, "character": 1 } },
            "selectionRange": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 8 } }
        }]);
        let elsewhere = domain::lsp::uri::uri_of(&dir.path().join("src/a.rs"));
        let references = serde_json::json!([{ "uri": elsewhere, "range": {
            "start": { "line": 6, "character": 0 }, "end": { "line": 6, "character": 5 } } }]);
        let server = crate::lsp::fake::ScriptedServer::answering([Ok(symbols), Ok(references)]);
        let mut workspace = Workspace::new(dir.path());
        workspace.services.lsp = Some(server.lsp());
        workspace.policy.trust = domain::trust::Trust::Trusted;

        assert_eq!(workspace.callers(&file, "fn retry() {}\n", "fn retry() { x }\n"), "");

        workspace.blast_radius = true;
        let told = workspace.callers(&file, "fn retry() {}\n", "fn retry() { x }\n");
        assert!(told.contains("<callers>") && told.contains("retry (1 place): src/a.rs:7"), "{told}");
        assert_eq!(workspace.callers(&file, "same\n", "same\n"), "");
    }
}
