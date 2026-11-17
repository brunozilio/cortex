use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sandbox::Confinement;

const PROGRAM: &str = "/usr/bin/sandbox-exec";

const DEVICES: &str = "(allow file-write* (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/tty\") \
                       (literal \"/dev/ptmx\") (literal \"/dev/dtracehelper\") (regex #\"^/dev/ttys[0-9]+$\") \
                       (regex #\"^/dev/fd/\"))";

pub fn confined(inner: &Command, confinement: &Confinement) -> Command {
    let mut outer = Command::new(PROGRAM);
    outer.arg("-p").arg(profile(confinement)).arg(inner.get_program()).args(inner.get_args());
    for (key, value) in inner.get_envs() {
        match value {
            Some(value) => outer.env(key, value),
            None => outer.env_remove(key),
        };
    }
    outer.envs(confinement.environment.iter().map(|(key, value)| (key, value)));
    if let Some(dir) = inner.get_current_dir() {
        outer.current_dir(dir);
    }
    outer
}

const HANDING_OFF: [&str; 3] = [
    "(deny appleevent-send)",
    "(deny mach-lookup (global-name \"com.apple.coreservices.launchservicesd\") (global-name \
     \"com.apple.coreservices.quarantine-resolver\"))",
    "(deny process-exec (literal \"/bin/launchctl\"))",
];

pub fn profile(confinement: &Confinement) -> String {
    let mut lines = vec!["(version 1)".to_owned(), "(allow default)".to_owned(), "(deny file-write*)".to_owned()];
    lines.push(DEVICES.to_owned());
    let writable: Vec<PathBuf> = confinement.writable.iter().map(|path| real(path)).collect();
    let protected: Vec<PathBuf> = confinement.protected.iter().map(|path| real(path)).collect();
    let inside =
        |path: &PathBuf, folders: &[PathBuf]| folders.iter().any(|folder| path != folder && path.starts_with(folder));
    let reopened: Vec<PathBuf> = writable.iter().filter(|path| inside(path, &protected)).cloned().collect();
    lines.extend(writable.iter().map(|path| format!("(allow file-write* {})", subpath(path))));
    lines.extend(protected.iter().map(|path| format!("(deny file-write* {})", subpath(path))));
    lines.extend(reopened.iter().map(|path| format!("(allow file-write* {})", subpath(path))));
    let within_reopened = protected.iter().filter(|path| inside(path, &reopened));
    lines.extend(within_reopened.map(|path| format!("(deny file-write* {})", subpath(path))));
    lines.extend(HANDING_OFF.map(str::to_owned));
    if !confinement.network {
        lines.push("(deny network*)".to_owned());
    }
    lines.join("\n")
}

fn subpath(path: &Path) -> String {
    let real = real(path);
    let text = real.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    format!("(subpath \"{text}\")")
}

fn real(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(found) = existing.canonicalize() {
            return missing.iter().rev().fold(found, |whole, name| whole.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::process::Stdio;

    use super::*;

    fn runs(code: &str, dir: &Path, confinement: &Confinement) -> bool {
        let mut shell = Command::new("/bin/sh");
        shell.args(["-c", code]).current_dir(dir);
        let mut confined = confined(&shell, confinement);
        confined.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        confined.status().expect("the sandbox starts").success()
    }

    fn within(project: &Path) -> Confinement {
        Confinement { writable: vec![project.to_path_buf()], network: true, ..Confinement::default() }
    }

    #[test]
    fn a_command_writes_inside_its_folders_and_nowhere_else() {
        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::tempdir().expect("outside");
        let stray = outside.path().join("stray.txt");

        assert!(runs("echo kept > kept.txt && cat kept.txt > /dev/null", project.path(), &within(project.path())));
        assert!(project.path().join("kept.txt").is_file());
        assert!(!runs(&format!("echo x > '{}'", stray.display()), project.path(), &within(project.path())));
        assert!(!stray.exists());
        assert!(runs(&format!("ls '{}'", outside.path().display()), project.path(), &within(project.path())));
    }

    #[test]
    fn a_protected_file_inside_a_writable_folder_stays_as_it_is() {
        let project = tempfile::tempdir().expect("project");
        let settings = project.path().join(".app/settings.json");
        std::fs::create_dir_all(project.path().join(".app")).expect("folder");
        std::fs::write(&settings, "{}").expect("settings");
        let confinement = Confinement { protected: vec![settings.clone()], ..within(project.path()) };

        assert!(!runs("echo '{\"allow\":true}' > .app/settings.json", project.path(), &confinement));
        assert!(!runs("rm .app/settings.json", project.path(), &confinement));
        assert_eq!(std::fs::read_to_string(&settings).expect("read"), "{}");
        assert!(runs("echo ok > .app/notes.txt", project.path(), &confinement));
    }

    #[test]
    fn a_folder_that_does_not_exist_yet_can_be_created() {
        let home = tempfile::tempdir().expect("home");
        std::fs::create_dir(home.path().join("cache")).expect("folder");
        let cache = home.path().join("cache/registry");
        let confinement = within(&cache);

        assert!(runs(&format!("mkdir -p '{0}' && echo x > '{0}/index'", cache.display()), home.path(), &confinement));
        assert!(!runs("echo x > beside.txt", home.path(), &confinement));
    }

    #[test]
    fn the_network_is_reached_only_when_allowed() {
        let project = tempfile::tempdir().expect("project");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let port = listener.local_addr().expect("address").port();
        let reach = format!("/usr/bin/nc -z -w 2 127.0.0.1 {port}");

        assert!(runs(&reach, project.path(), &within(project.path())));
        let offline = Confinement { network: false, ..within(project.path()) };
        assert!(!runs(&reach, project.path(), &offline));
    }

    fn guarded(project: &Path) -> Confinement {
        let protected = vec![project.join(".app"), project.join(".git/config"), project.join(".git/hooks")];
        Confinement { protected, ..within(project) }
    }

    fn git(dir: &Path) -> String {
        let identity = "git -c user.name=t -c user.email=t@t -c commit.gpgsign=false -c core.hooksPath=/dev/null";
        format!("{identity} -C '{}'", dir.display())
    }

    #[test]
    fn a_command_can_commit_but_not_change_what_git_runs_or_the_app_s_folder() {
        let project = tempfile::tempdir().expect("project");
        let dir = project.path();
        assert!(runs(&format!("{} init -q", git(dir)), dir, &within(dir)), "git init is not confined here");
        std::fs::create_dir_all(dir.join(".app")).expect("folder");
        let config = std::fs::read(dir.join(".git/config")).expect("config");
        let confinement = guarded(dir);

        assert!(!runs("echo '[core]\n\tfsmonitor = ./x.sh' >> .git/config", dir, &confinement));
        assert!(!runs("echo 'touch pwned' > .git/hooks/post-commit", dir, &confinement));
        assert!(!runs("echo '{}' > .app/settings.json", dir, &confinement));
        assert!(!runs("mkdir .app/hooks", dir, &confinement));
        assert_eq!(std::fs::read(dir.join(".git/config")).expect("config"), config);
        assert!(!dir.join(".git/hooks/post-commit").exists());

        let commit = format!("echo a > a.txt && {0} add a.txt && {0} commit -q -m one", git(dir));
        assert!(runs(&commit, dir, &confinement), "a commit writes objects, the index and refs");
        assert!(runs(&format!("{} log --oneline | grep -q one", git(dir)), dir, &confinement));
    }

    #[test]
    fn a_writable_folder_inside_a_guarded_one_stays_writable_but_its_own_guards_hold() {
        let repo = tempfile::tempdir().expect("repo");
        let worktree = repo.path().join(".app/worktrees/fix");
        std::fs::create_dir_all(worktree.join(".app")).expect("folders");
        let mut confinement = guarded(repo.path());
        confinement.writable.push(worktree.clone());
        confinement.protected.push(worktree.join(".app"));

        assert!(runs("echo ok > done.txt", &worktree, &confinement), "the worktree is the session's project");
        assert!(!runs("echo x > .app/settings.json", &worktree, &confinement), "its own folder stays guarded");
        assert!(!runs("echo x > ../other.txt", &worktree, &confinement), "the rest of the app's folder too");
        assert!(worktree.join("done.txt").is_file());
    }

    #[test]
    fn a_command_cannot_hand_work_to_launchd_or_another_app() {
        let project = tempfile::tempdir().expect("project");
        assert!(!runs("/bin/launchctl version", project.path(), &within(project.path())));
        let text = profile(&within(project.path()));
        for line in [
            "(deny appleevent-send)",
            "(global-name \"com.apple.coreservices.launchservicesd\")",
            "(global-name \"com.apple.coreservices.quarantine-resolver\")",
            "(deny process-exec (literal \"/bin/launchctl\"))",
        ] {
            assert!(text.contains(line), "{line} in {text}");
        }
        assert!(runs("git --version && cargo --version", project.path(), &within(project.path())));
    }

    #[test]
    fn a_path_cannot_break_out_of_the_profile() {
        let confinement = within(Path::new("/tmp/a\"b\\c"));
        assert!(profile(&confinement).contains(r#"(subpath "/private/tmp/a\"b\\c")"#));
    }
}
