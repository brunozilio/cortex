use std::path::{Path, PathBuf};

use serde_json::Value;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];

const CACHES: [&str; 10] = [
    "Library/Caches",
    ".cache",
    ".npm",
    ".cargo/registry",
    ".cargo/git",
    ".cargo/.package-cache",
    ".cargo/.package-cache-mutate",
    ".cargo/.global-cache",
    "go/pkg",
    ".gradle/caches",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sandbox {
    pub allow_write: Vec<String>,
    pub network: bool,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self { allow_write: Vec::new(), network: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places<'a> {
    pub project: &'a Path,
    pub added: &'a [PathBuf],
    pub repository: Option<&'a Path>,
    pub temp: &'a [PathBuf],
    pub home: Option<&'a Path>,
    pub settings_dir: &'a str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confinement {
    pub writable: Vec<PathBuf>,
    pub protected: Vec<PathBuf>,
    pub network: bool,
}

impl Sandbox {
    pub fn layered(mut self, value: &Value, owners: bool) -> Result<Self, String> {
        if let Some(flag) = value.get("network") {
            let flag = flag.as_bool().ok_or("sandbox.network must be true or false")?;
            self.network = if owners { flag } else { self.network && flag };
        }
        if let Some(paths) = value.get("allowWrite") {
            let paths = paths.as_array().ok_or("sandbox.allowWrite must be a list of folders")?;
            let named = paths.iter().map(|path| path.as_str().map(str::to_owned));
            let named: Option<Vec<String>> = named.collect();
            let named = named.ok_or("sandbox.allowWrite must be a list of folders")?;
            if owners {
                self.allow_write.extend(named);
            }
        }
        Ok(self)
    }

    pub fn confinement(&self, places: &Places<'_>) -> Confinement {
        let mut writable = vec![places.project.to_path_buf()];
        writable.extend(places.added.iter().cloned());
        writable.extend(places.repository.map(Path::to_path_buf));
        writable.extend(places.temp.iter().cloned());
        if let Some(home) = places.home {
            writable.extend(CACHES.iter().map(|cache| home.join(cache)));
        }
        writable.extend(self.allow_write.iter().filter_map(|path| owned(path, places.home)));
        let mut kept = Vec::new();
        for path in writable {
            if !kept.contains(&path) {
                kept.push(path);
            }
        }
        let roots = std::iter::once(places.project).chain(places.added.iter().map(PathBuf::as_path));
        let protected =
            roots.flat_map(|root| SETTINGS.iter().map(move |name| root.join(places.settings_dir).join(name))).collect();
        Confinement { writable: kept, protected, network: self.network }
    }
}

fn owned(path: &str, home: Option<&Path>) -> Option<PathBuf> {
    match path.strip_prefix("~/") {
        Some(rest) => home.map(|home| home.join(rest)),
        None => Path::new(path).is_absolute().then(|| PathBuf::from(path)),
    }
}

pub fn blocked(output: &str) -> bool {
    output.to_ascii_lowercase().contains("operation not permitted")
}

pub fn hint(settings_dir: &str) -> String {
    format!(
        "(the sandbox may have refused this: commands write only inside the project, added folders, the \
         temporary folder and package caches. Do not work around it; tell the user, who can add a folder to \
         sandbox.allowWrite in ~/{settings_dir}/settings.json)"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn places<'a>(added: &'a [PathBuf], temp: &'a [PathBuf]) -> Places<'a> {
        Places {
            project: Path::new("/repo"),
            added,
            repository: None,
            temp,
            home: Some(Path::new("/Users/ana")),
            settings_dir: ".app",
        }
    }

    #[test]
    fn commands_write_in_the_project_added_folders_temp_and_caches_only() {
        let added = [PathBuf::from("/other")];
        let temp = [PathBuf::from("/tmp")];
        let confinement = Sandbox::default().confinement(&places(&added, &temp));

        assert_eq!(confinement.writable[..3], [PathBuf::from("/repo"), "/other".into(), "/tmp".into()]);
        assert!(confinement.writable.contains(&PathBuf::from("/Users/ana/.cargo/registry")));
        assert!(!confinement.writable.iter().any(|path| path == Path::new("/Users/ana")));
        assert!(!confinement.writable.iter().any(|path| path == Path::new("/Users/ana/.cargo")));
        assert!(confinement.network);
    }

    #[test]
    fn the_settings_that_grant_permissions_stay_out_of_reach() {
        let added = [PathBuf::from("/other")];
        let confinement = Sandbox::default().confinement(&places(&added, &[]));

        assert_eq!(
            confinement.protected,
            [
                PathBuf::from("/repo/.app/settings.json"),
                "/repo/.app/settings.local.json".into(),
                "/other/.app/settings.json".into(),
                "/other/.app/settings.local.json".into(),
            ]
        );
    }

    #[test]
    fn a_session_worktree_reaches_its_repository() {
        let places = Places { repository: Some(Path::new("/repo")), ..places(&[], &[]) };
        let places = Places { project: Path::new("/repo/.app/worktrees/fix"), ..places };

        let confinement = Sandbox::default().confinement(&places);

        assert_eq!(confinement.writable[..2], [PathBuf::from("/repo/.app/worktrees/fix"), "/repo".into()]);
    }

    #[test]
    fn only_the_owner_widens_the_sandbox() {
        let project = json!({ "allowWrite": ["/etc"], "network": false });
        let narrowed = Sandbox::default().layered(&project, false).expect("settings");
        assert!(narrowed.allow_write.is_empty() && !narrowed.network);

        let owners = json!({ "allowWrite": ["~/.cargo", "/data", "relative"], "network": true });
        let widened = Sandbox::default().layered(&owners, true).expect("settings");
        let confinement = widened.confinement(&places(&[], &[]));
        assert!(confinement.writable.contains(&PathBuf::from("/Users/ana/.cargo")));
        assert!(confinement.writable.contains(&PathBuf::from("/data")));
        assert!(!confinement.writable.iter().any(|path| path.ends_with("relative")));

        let off = Sandbox { network: false, ..Sandbox::default() };
        let back_on = off.layered(&json!({ "network": true }), false).expect("settings");
        assert!(!back_on.network, "a project cannot turn the network back on");
    }

    #[test]
    fn settings_of_the_wrong_shape_are_refused() {
        assert!(Sandbox::default().layered(&json!({ "network": "yes" }), true).is_err());
        assert!(Sandbox::default().layered(&json!({ "allowWrite": "/data" }), true).is_err());
        assert!(Sandbox::default().layered(&json!({ "allowWrite": [1] }), true).is_err());
    }

    #[test]
    fn a_refused_write_is_recognised_and_explained() {
        assert!(blocked("touch: /etc/x: Operation not permitted"));
        assert!(blocked("zsh:1: operation not permitted: .app/settings.json"));
        assert!(!blocked("No such file or directory"));
        assert!(hint(".app").contains("~/.app/settings.json"));
    }
}
