use std::path::Path;

pub use domain::changes::{Changes, DiffScope as Scope};

use super::app::{App, Msg};

pub fn changes(client: &app::Client, cwd: &Path) -> Result<Changes, String> {
    changes_in(client, cwd, Scope::Uncommitted)
}

pub fn changes_in(client: &app::Client, cwd: &Path, scope: Scope) -> Result<Changes, String> {
    client.changes(cwd, scope).map_err(|error| error.to_string())
}

pub(super) fn open_diffs(app: &mut App) {
    app.overlays.diffs = Some(super::diff_view::DiffView::new(Scope::Uncommitted));
    load_diffs(app, Scope::Uncommitted);
}

pub(super) fn show_changes(app: &App) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| Msg::Changes(changes(client, &cwd)));
}

pub(super) fn load_diffs(app: &App, scope: Scope) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| Msg::Diffs(scope, changes_in(client, &cwd, scope)));
}

pub(super) fn shown(app: &mut App, changes: Result<Changes, String>) {
    match changes {
        Ok(Changes { diff, untracked }) if diff.trim().is_empty() && untracked.is_empty() => {
            app.transcript.notice("No uncommitted changes.");
        }
        Ok(Changes { diff, untracked }) => app.transcript.push(super::transcript::Entry::Diff { diff, untracked }),
        Err(error) => app.transcript.error(format!("/diff needs a git repository: {error}")),
    }
}
