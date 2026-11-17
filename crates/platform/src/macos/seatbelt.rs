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

const SERVICE_SOCKETS: [&str; 4] =
    [r"(^|/)(docker|podman|containerd|buildkitd|colima)\.sock$", r"/tmux-[0-9]+/", r"/screens/S-[^/]+/", r"/\.screen/"];

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
    let unreadable: Vec<PathBuf> = confinement.unreadable.iter().map(|path| real(path)).collect();
    let mut pinned = pinned(protected.iter().chain(&unreadable), &writable);
    let movable = |path: &&PathBuf| writable.iter().any(|open| path.starts_with(open)) && !pinned.contains(path);
    let shut_and_movable: Vec<PathBuf> = unreadable.iter().filter(movable).cloned().collect();
    pinned.extend(shut_and_movable);
    lines.extend(pinned.iter().map(|path| format!("(deny file-write-unlink (literal \"{}\"))", quoted(path))));
    lines.extend(unreadable.iter().map(|path| format!("(deny file-read-data {})", subpath(path))));
    lines.extend(confinement.readable.iter().map(|path| format!("(allow file-read-data {})", subpath(path))));
    lines.extend(HANDING_OFF.map(str::to_owned));
    lines.push("(deny user-preference-write)".to_owned());
    lines.extend(
        SERVICE_SOCKETS
            .map(|pattern| format!("(deny network-outbound (remote unix-socket (path-regex #\"{pattern}\")))")),
    );
    lines.extend(confinement.sockets.iter().map(|socket| {
        let real = quoted(&real(socket));
        format!("(allow network-outbound (remote unix-socket (path-literal \"{real}\")))")
    }));
    if !confinement.network {
        lines.push("(deny network*)".to_owned());
    }
    lines.join("\n")
}

fn pinned<'a>(guarded: impl Iterator<Item = &'a PathBuf>, writable: &[PathBuf]) -> Vec<PathBuf> {
    let mut pinned: Vec<PathBuf> = Vec::new();
    for path in guarded {
        let movable = path.ancestors().skip(1).filter(|folder| writable.iter().any(|open| folder.starts_with(open)));
        for folder in movable {
            if !pinned.iter().any(|known| known == folder) {
                pinned.push(folder.to_path_buf());
            }
        }
    }
    pinned
}

fn quoted(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
}

fn subpath(path: &Path) -> String {
    format!("(subpath \"{}\")", quoted(&real(path)))
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

    fn repository(dir: &Path) {
        let made = format!("{0} init -q && echo a > a.txt && {0} add a.txt && {0} commit -q -m one", git(dir));
        assert!(runs(&made, dir, &within(dir)), "a repository to work in");
    }

    fn guarded_repository(dir: &Path) -> Confinement {
        let protected = [".app", ".git/config", ".git/config.worktree", ".git/hooks", ".git/commondir"]
            .map(|name| dir.join(name))
            .to_vec();
        Confinement { protected, ..within(dir) }
    }

    #[test]
    fn moving_the_git_folder_aside_to_plant_a_hook_and_moving_it_back_is_refused() {
        let project = tempfile::tempdir().expect("project");
        let dir = project.path();
        repository(dir);
        std::fs::create_dir_all(dir.join(".app")).expect("folder");
        let confinement = guarded_repository(dir);

        for trick in [
            "mv .git .real && echo 'touch pwned' > .real/hooks/post-commit && mv .real .git",
            "mv .git .real",
            "mv .app .moved",
            "echo /elsewhere > .git/commondir",
            "ln .git/config linked && echo '[core] fsmonitor = x' >> linked",
        ] {
            assert!(!runs(trick, dir, &confinement), "{trick} was allowed");
        }
        assert!(dir.join(".git/HEAD").is_file() && dir.join(".app").is_dir(), "both stay where they were");
        assert!(!dir.join(".git/hooks/post-commit").exists() && !dir.join(".real").exists());
        assert!(!dir.join(".git/commondir").exists());

        let everyday = format!(
            "echo b > b.txt && {0} add b.txt && {0} commit -q -m two && {0} checkout -q -b topic && echo c > a.txt \
             && {0} stash -q && {0} stash pop -q && {0} checkout -q main && {0} branch -D -q topic",
            git(dir)
        );
        assert!(runs(&everyday, dir, &confinement), "commits, branches and stashes still work");

        let config = std::fs::read(dir.join(".git/config")).expect("config");
        let rebuilt = "cp -R .git .copy && echo 'touch pwned' > .copy/hooks/post-commit && rm -rf .git; mv .copy .git";
        let _ = runs(rebuilt, dir, &confinement);
        assert!(dir.join(".git").is_dir() && dir.join(".git/hooks").is_dir(), "the folder itself stays");
        assert_eq!(std::fs::read(dir.join(".git/config")).expect("config"), config);
        assert!(!dir.join(".git/hooks/post-commit").exists(), "no hook was planted");
    }

    #[test]
    fn moving_the_whole_project_out_to_a_writable_folder_and_back_is_refused() {
        let (project, scratch) = (tempfile::tempdir().expect("project"), tempfile::tempdir().expect("scratch"));
        let dir = project.path().join("work");
        std::fs::create_dir(&dir).expect("work");
        repository(&dir);
        let mut confinement = guarded_repository(&dir);
        confinement.writable.push(scratch.path().to_path_buf());
        let away = scratch.path().join("away");

        let trick = format!(
            "mv '{0}' '{1}' && echo 'touch pwned' > '{1}/.git/hooks/post-commit' && mv '{1}' '{0}'",
            dir.display(),
            away.display()
        );
        assert!(!runs(&trick, &dir, &confinement));
        assert!(dir.join(".git/HEAD").is_file() && !away.exists());
    }

    #[test]
    fn a_linked_worktree_s_git_file_cannot_be_pointed_elsewhere() {
        let project = tempfile::tempdir().expect("project");
        let dir = project.path();
        std::fs::write(dir.join(".git"), "gitdir: /repo/.git/worktrees/fix\n").expect("git file");
        let confinement = Confinement { protected: vec![dir.join(".git")], ..within(dir) };

        assert!(!runs("echo 'gitdir: /tmp/evil' > .git", dir, &confinement));
        assert!(!runs("echo x > other && mv other .git", dir, &confinement));
        assert!(!runs("rm .git", dir, &confinement));
        assert_eq!(std::fs::read_to_string(dir.join(".git")).expect("git file"), "gitdir: /repo/.git/worktrees/fix\n");
    }

    #[test]
    fn credential_stores_cannot_be_read_unless_reopened_and_everything_else_still_reads() {
        let (project, home) = (tempfile::tempdir().expect("project"), tempfile::tempdir().expect("home"));
        let home = home.path();
        std::fs::create_dir_all(home.join(".ssh")).expect("ssh");
        std::fs::write(home.join(".ssh/id_ed25519"), "private").expect("key");
        std::fs::write(home.join(".ssh/known_hosts"), "github.com ssh-ed25519 AAAA").expect("hosts");
        std::fs::write(home.join(".npmrc"), "//registry/:_authToken=secret").expect("npmrc");
        std::fs::write(home.join("notes.txt"), "fine").expect("notes");
        let unreadable = vec![home.join(".ssh"), home.join(".npmrc")];
        let confinement = Confinement {
            unreadable,
            readable: vec![home.join(".ssh/known_hosts")],
            writable: vec![project.path().to_path_buf(), home.to_path_buf()],
            ..Confinement::default()
        };
        let dir = project.path();
        let at = |name: &str| home.join(name).display().to_string();

        for refused in [
            format!("cat '{}'", at(".ssh/id_ed25519")),
            format!("ls '{}'", at(".ssh")),
            format!("cat '{}'", at(".npmrc")),
            format!("ln -s '{}' link && cat link", at(".ssh/id_ed25519")),
            format!("cp '{}' copy", at(".ssh/id_ed25519")),
            format!("mv '{}' '{}' && cat '{}/id_ed25519'", at(".ssh"), at("moved"), at("moved")),
        ] {
            assert!(!runs(&refused, dir, &confinement), "{refused} was allowed");
        }
        assert!(home.join(".ssh/id_ed25519").is_file(), "nothing was moved");
        for allowed in [
            format!("cat '{}'", at(".ssh/known_hosts")),
            format!("cat '{}'", at("notes.txt")),
            format!("ls -la '{}' > /dev/null", home.display()),
            format!("test -d '{}'", at(".ssh")),
        ] {
            assert!(runs(&allowed, dir, &confinement), "{allowed} was refused");
        }
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

    struct Forgotten(String);

    impl Drop for Forgotten {
        fn drop(&mut self) {
            let _ = Command::new("/usr/bin/defaults")
                .args(["delete", &self.0])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if let Some(home) = std::env::var_os("HOME") {
                let _ = std::fs::remove_file(Path::new(&home).join(format!("Library/Preferences/{}.plist", self.0)));
            }
        }
    }

    #[test]
    fn a_command_cannot_change_a_preference_through_the_preferences_service() {
        let project = tempfile::tempdir().expect("project");
        let domain = format!("dev.sandbox-test.preferences.{}", std::process::id());
        let _forgotten = Forgotten(domain.clone());
        let write = format!("/usr/bin/defaults write {domain} reached yes");
        assert!(!runs(&write, project.path(), &within(project.path())), "the preference was written");
        let read = Command::new("/usr/bin/defaults").args(["read", &domain]).output().expect("defaults");
        assert!(!read.status.success(), "{}", String::from_utf8_lossy(&read.stdout));
        assert!(
            runs("/usr/bin/defaults read -g > /dev/null", project.path(), &within(project.path())),
            "reads still work"
        );
    }

    fn listening(path: &Path) -> std::os::unix::net::UnixListener {
        std::fs::create_dir_all(path.parent().expect("folder")).expect("folder");
        std::os::unix::net::UnixListener::bind(path).expect("socket")
    }

    fn connects(socket: &Path, dir: &Path, confinement: &Confinement) -> bool {
        let reach = format!(
            "/usr/bin/perl -MIO::Socket::UNIX -e 'IO::Socket::UNIX->new(Peer => $ARGV[0]) or exit 1' '{}'",
            socket.display()
        );
        runs(&reach, dir, confinement)
    }

    #[test]
    fn a_command_cannot_reach_a_container_engine_or_a_terminal_multiplexer_unless_allowed() {
        let project = tempfile::tempdir().expect("project");
        let sockets = tempfile::tempdir().expect("sockets");
        let docker = sockets.path().join("run/docker.sock");
        let tmux = sockets.path().join("tmux-501/default");
        let screen = sockets.path().join("screens/S-ana/123.tty.host");
        let other = sockets.path().join("app.sock");
        let _held = [&docker, &tmux, &screen, &other].map(|path| listening(path));
        let confined = within(project.path());

        assert!(connects(&other, project.path(), &confined), "another socket is reached");
        for refused in [&docker, &tmux, &screen] {
            assert!(!connects(refused, project.path(), &confined), "{} was reached", refused.display());
        }
        let allowed = Confinement { sockets: vec![docker.clone()], ..within(project.path()) };
        assert!(connects(&docker, project.path(), &allowed), "a socket the settings allow is reached");
        assert!(!connects(&tmux, project.path(), &allowed));
    }

    #[test]
    fn everyday_developer_tools_still_work_inside() {
        let project = tempfile::tempdir().expect("project");
        let confined = within(project.path());
        let identity = "git -c user.name=t -c user.email=t@t -c commit.gpgsign=false -c core.hooksPath=/dev/null";
        let everyday = format!(
            "{identity} init -q && echo a > a && {identity} add a && {identity} commit -q -m one && git status --short \
             && cargo --version && python3 -c 'import json, sqlite3; print(json.dumps(1))' \
             && (command -v npm > /dev/null || exit 0) && npm --version && node -e 'process.exit(0)'"
        );
        assert!(runs(&everyday, project.path(), &confined));
    }

    #[test]
    fn a_path_cannot_break_out_of_the_profile() {
        let confinement = within(Path::new("/tmp/a\"b\\c"));
        assert!(profile(&confinement).contains(r#"(subpath "/private/tmp/a\"b\\c")"#));
    }
}
