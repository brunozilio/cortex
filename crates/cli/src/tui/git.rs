use std::path::Path;

pub use domain::changes::{Changes, DiffScope as Scope};
use serde_json::json;

pub fn changes(sdk: &sdk::Sdk, cwd: &Path) -> Result<Changes, String> {
    changes_in(sdk, cwd, Scope::Uncommitted)
}

pub fn changes_in(sdk: &sdk::Sdk, cwd: &Path, scope: Scope) -> Result<Changes, String> {
    let asked = sdk.feature("workspace", json!({ "op": "changes", "path": cwd, "scope": scope }));
    let value = asked.map_err(|error| error.to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

impl super::app::App {
    pub(super) fn load_diffs(&self, scope: Scope) {
        let cwd = self.cwd.clone();
        self.spawn(move |sdk| super::app::Msg::Diffs(scope, changes_in(sdk, &cwd, scope)));
    }

    pub(super) fn changes(&mut self, changes: Result<Changes, String>) {
        match changes {
            Ok(Changes { diff, untracked }) if diff.trim().is_empty() && untracked.is_empty() => {
                self.transcript.notice("No uncommitted changes.");
            }
            Ok(Changes { diff, untracked }) => self.transcript.push(super::transcript::Entry::Diff { diff, untracked }),
            Err(error) => self.transcript.error(format!("/diff needs a git repository: {error}")),
        }
    }
}
