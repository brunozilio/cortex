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
    if let Some(dir) = inner.get_current_dir() {
        outer.current_dir(dir);
    }
    outer
}

pub fn profile(confinement: &Confinement) -> String {
    let mut lines = vec!["(version 1)".to_owned(), "(allow default)".to_owned(), "(deny file-write*)".to_owned()];
    lines.push(DEVICES.to_owned());
    lines.extend(confinement.writable.iter().map(|path| format!("(allow file-write* {})", subpath(path))));
    lines.extend(confinement.protected.iter().map(|path| format!("(deny file-write* {})", subpath(path))));
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
        Confinement { writable: vec![project.to_path_buf()], protected: Vec::new(), network: true }
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

    #[test]
    fn a_path_cannot_break_out_of_the_profile() {
        let confinement = within(Path::new("/tmp/a\"b\\c"));
        assert!(profile(&confinement).contains(r#"(subpath "/private/tmp/a\"b\\c")"#));
    }
}
