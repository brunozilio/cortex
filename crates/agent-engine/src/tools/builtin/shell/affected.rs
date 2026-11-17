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
    let stale = shells::list(client).into_iter().filter(|(_, command, _)| *command == label);
    for (id, _, _) in stale.collect::<Vec<_>>() {
        shells::kill(client, &id);
        crate::tools::inbox::withdraw(client, &shells::ended_source(&id));
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
        workspace.credentials.client = 9_351;
        let file = pkg.join("store.go");

        assert_eq!(start(&workspace, &file), "", "nothing runs with the flag off");
        workspace.policy.affected_tests = true;
        let first = start(&workspace, &file);
        assert!(first.contains("Started go test . in pkg as bash_"), "{first}");
        assert_eq!(start(&workspace, &dir.path().join("README.md")), "");
        assert_eq!(start(&workspace, Path::new("/elsewhere/x.go")), "", "only files in the project");
        shells::kill_client(9_351);
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
        workspace.credentials.client = 9_352;
        workspace.policy.affected_tests = true;
        let store = dir.path().join("store/db.go");

        let first = start(&workspace, &store);
        let again = start(&workspace, &store);
        let api = start(&workspace, &dir.path().join("api/server.go"));

        let id = |text: &str| text.split(" as ").nth(1).and_then(|rest| rest.split(' ').next()).map(str::to_owned);
        assert!(again.contains("Started go test . in store"), "{again}");
        assert_ne!(id(&first), id(&again), "the stale run gives way to a fresh one");
        assert!(api.contains("Started go test . in api"), "{api}");
        let listed: Vec<String> = shells::list(9_352).into_iter().map(|(id, _, _)| id).collect();
        assert!(!listed.contains(&id(&first).unwrap_or_default()), "the replaced run is gone: {listed:?}");
        let mut going = running(9_352);
        going.sort();
        assert!(going.len() <= 2, "{going:?}");
        shells::kill_client(9_352);
    }

    #[test]
    fn a_finished_result_not_yet_delivered_is_withdrawn_when_a_newer_change_comes() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join("store")).expect("package");
        let mut workspace = Workspace::new(dir.path());
        workspace.credentials.client = 9_353;
        workspace.policy.affected_tests = true;
        let store = dir.path().join("store/db.go");

        let first = start(&workspace, &store);
        let first_id =
            first.split(" as ").nth(1).and_then(|rest| rest.split(' ').next()).unwrap_or_default().to_owned();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !running(9_353).is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::thread::sleep(std::time::Duration::from_millis(1_200));
        start(&workspace, &store);

        let notices = crate::tools::inbox::take(9_353);
        let stale = format!("background {first_id}");
        assert!(notices.iter().all(|notice| notice.source != stale), "{notices:?}");
        shells::kill_client(9_353);
        crate::tools::inbox::forget_client(9_353);
    }
}
