use std::path::Path;
use std::process::Stdio;

use super::shells::{self, Watch};
use crate::tools::Workspace;

pub fn start(workspace: &Workspace, path: &Path) -> String {
    if !workspace.policy.affected_tests || !path.starts_with(&workspace.cwd) {
        return String::new();
    }
    let exists = |at: &Path| at.exists();
    let read = |at: &Path| std::fs::read_to_string(at).ok();
    let Some(run) = domain::affected::tests_for(path, &workspace.cwd, &exists, &read) else {
        return String::new();
    };
    let client = workspace.credentials.client;
    if shells::list(client).iter().any(|(_, command, running)| *running && *command == run.command) {
        return format!(
            "\n\n<tests>{} is already running in the background; its result will reach you.</tests>",
            run.command
        );
    }
    let mut shell = workspace.confined(platform::shell(&run.command));
    shell.current_dir(&run.folder).envs(run.env.iter().cloned()).stdin(Stdio::null());
    match shells::start(workspace, &run.command, &mut shell) {
        Ok(id) => {
            shells::watch(id.clone(), client, Watch::End);
            format!(
                "\n\n<tests>Started {} as {id} for this change; its result will reach you when it ends.</tests>",
                run.command
            )
        }
        Err(error) => {
            tracing::warn!(%error, "the affected tests did not start");
            String::new()
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    #[test]
    fn a_change_starts_its_tests_once_while_they_run() {
        let dir = tempfile::tempdir().expect("dir");
        let pkg = dir.path().join("pkg");
        std::fs::create_dir(&pkg).expect("pkg");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 9_301;
        let file = pkg.join("store.go");

        assert_eq!(start(&workspace, &file), "", "nothing runs with the flag off");
        workspace.policy.affected_tests = true;
        let first = start(&workspace, &file);
        let second = start(&workspace, &file);

        assert!(first.contains("Started go test . as bash_"), "{first}");
        assert!(second.contains("already running") || second.contains("Started"), "{second}");
        assert_eq!(start(&workspace, &dir.path().join("README.md")), "");
        assert_eq!(start(&workspace, Path::new("/elsewhere/x.go")), "", "only files in the project");
        shells::kill_client(9_301);
    }
}
