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
    let label = run.label(&workspace.cwd);
    let stale = shells::list(client).into_iter().filter(|(_, command, running)| *running && *command == label);
    for (id, _, _) in stale.collect::<Vec<_>>() {
        shells::kill(client, &id);
    }
    let mut shell = workspace.confined(platform::shell(&run.command));
    shell.current_dir(&run.folder).envs(run.env.iter().cloned()).stdin(Stdio::null());
    match shells::start(workspace, &label, &mut shell) {
        Ok(id) => {
            shells::watch(id.clone(), client, Watch::End);
            format!(
                "\n\n<tests>Started {label} as {id} for this change; its result will reach you when it ends.</tests>"
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
    fn a_change_in_the_project_starts_its_package_s_tests() {
        let dir = tempfile::tempdir().expect("dir");
        let pkg = dir.path().join("pkg");
        std::fs::create_dir(&pkg).expect("pkg");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 9_301;
        let file = pkg.join("store.go");

        assert_eq!(start(&workspace, &file), "", "nothing runs with the flag off");
        workspace.policy.affected_tests = true;
        let first = start(&workspace, &file);
        assert!(first.contains("Started go test . in pkg as bash_"), "{first}");
        assert_eq!(start(&workspace, &dir.path().join("README.md")), "");
        assert_eq!(start(&workspace, Path::new("/elsewhere/x.go")), "", "only files in the project");
        shells::kill_client(9_301);
    }

    fn running(client: u64) -> Vec<String> {
        shells::list(client).into_iter().filter(|(_, _, running)| *running).map(|(_, command, _)| command).collect()
    }

    #[test]
    fn a_newer_change_replaces_the_run_its_package_had_going_and_other_packages_run_beside_it() {
        let dir = tempfile::tempdir().expect("dir");
        for package in ["store", "api"] {
            std::fs::create_dir(dir.path().join(package)).expect("package");
        }
        std::fs::write(dir.path().join("go.mod"), "module x\n").expect("go.mod");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 9_302;
        workspace.policy.affected_tests = true;
        let store = dir.path().join("store/db.go");

        let first = start(&workspace, &store);
        let again = start(&workspace, &store);
        let api = start(&workspace, &dir.path().join("api/server.go"));

        let id = |text: &str| text.split(" as ").nth(1).and_then(|rest| rest.split(' ').next()).map(str::to_owned);
        assert!(again.contains("Started go test . in store"), "{again}");
        assert_ne!(id(&first), id(&again), "the stale run gives way to a fresh one");
        assert!(api.contains("Started go test . in api"), "{api}");
        let listed: Vec<String> = shells::list(9_302).into_iter().map(|(id, _, _)| id).collect();
        assert!(!listed.contains(&id(&first).unwrap_or_default()), "the replaced run is gone: {listed:?}");
        let mut going = running(9_302);
        going.sort();
        assert!(going.len() <= 2, "{going:?}");
        shells::kill_client(9_302);
    }
}
