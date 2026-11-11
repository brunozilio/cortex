//! What a tool call acts on, for the settings' permission rules: the
//! command a `Bash` runs, the folder a `Glob` or `Grep` searches, or the
//! path a file tool reads or writes. A path is judged both as written (made
//! absolute, `..` folded) and as the file system resolves it (links
//! followed): either one denied or asked about is enough, and both must be
//! allowed.

use std::path::{Component, Path, PathBuf};

use domain::permission_rules::{PermissionRules, RuleKind, Target};
use serde_json::Value;

use super::Workspace;

type Ruling = Option<(RuleKind, String)>;

impl Workspace {
    /// What the permission rules say about tool `name` called with `input`.
    pub fn ruling(&self, name: &str, subject: &str, input: &Value) -> Ruling {
        self.ruling_with(&self.permissions, name, subject, input)
    }

    /// [`ruling`](Self::ruling) under other rules (those allowed during the
    /// run).
    pub fn ruling_with(&self, rules: &PermissionRules, name: &str, subject: &str, input: &Value) -> Ruling {
        if rules.is_empty() {
            return None;
        }
        let cwd = self.cwd.to_string_lossy();
        let home = self.home.as_ref().map(|home| home.to_string_lossy().into_owned());
        let rule = |target: &Target<'_>| rules.ruling(name, target, &cwd, home.as_deref());
        let path = ["file_path", "notebook_path", "path"].iter().find_map(|key| input.get(key).and_then(Value::as_str));
        let searches = matches!(name, "Glob" | "Grep");
        if let ("Bash", Some(command)) = (name, input.get("command").and_then(Value::as_str)) {
            return rule(&Target::Command(command));
        }
        if matches!(name, "ListMcpResources" | "ReadMcpResource") {
            // A server's resources are its own: its rules (`mcp__docs`) apply.
            let server = input.get("server").and_then(Value::as_str).unwrap_or_default();
            let uri = input.get("uri").and_then(Value::as_str).unwrap_or_default();
            return rules.ruling(&format!("mcp__{server}__resources"), &Target::Other(uri), &cwd, home.as_deref());
        }
        if name == "WebFetch" {
            let url = input.get("url").and_then(Value::as_str).unwrap_or(subject);
            let site = super::web::host(url).map(|host| format!("domain:{host}")).unwrap_or_default();
            // `domain:Docs.RS.` is the same site as `domain:docs.rs`.
            return rule(&Target::Other(&site));
        }
        if !searches && path.is_none() {
            return rule(&Target::Other(subject));
        }
        let written = folded(&path.map_or_else(|| self.cwd.clone(), |path| self.resolve(path)));
        let real = resolved(&written);
        let judge = |path: &Path| {
            let path = path.to_string_lossy();
            rule(&if searches { Target::Tree(&path) } else { Target::Path(&path) })
        };
        let as_written = judge(&written);
        if real == written { as_written } else { strictest(as_written, judge(&real)) }
    }
}

/// Of two rulings of one call: a deny, else an ask, and an allow only when
/// both allow.
fn strictest(a: Ruling, b: Ruling) -> Ruling {
    let kind = |r: &Ruling| r.as_ref().map(|(kind, _)| *kind);
    for wanted in [RuleKind::Deny, RuleKind::Ask] {
        if kind(&a) == Some(wanted) {
            return a;
        }
        if kind(&b) == Some(wanted) {
            return b;
        }
    }
    if kind(&a) == Some(RuleKind::Allow) && kind(&b) == Some(RuleKind::Allow) { a } else { None }
}

/// `path` without `.` and with each `..` taking its folder away, as written.
fn folded(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// `path` with its deepest existing folder resolved through links (what is
/// below it does not exist yet).
fn resolved(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return rest.iter().rev().fold(real, |path, name| path.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::permission_rules::{PermissionRules, RuleKind};
    use serde_json::json;

    use super::*;

    fn kind(ruling: Ruling) -> Option<RuleKind> {
        ruling.map(|(kind, _)| kind)
    }

    #[test]
    fn a_path_cannot_climb_around_a_rule() {
        let mut workspace = Workspace::new("/work/rocket");
        workspace.permissions = PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)");
        let read = |path: &str| kind(workspace.ruling("Read", path, &json!({ "file_path": path })));
        assert_eq!(read("src/../secrets/key"), Some(RuleKind::Deny));
        assert_eq!(read("/work/rocket/./secrets/key"), Some(RuleKind::Deny));
        assert_eq!(read("src/main.rs"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_judged_by_where_it_leads_too() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().canonicalize().expect("real");
        std::fs::create_dir_all(project.join("secrets")).expect("secrets");
        std::fs::create_dir_all(project.join("src")).expect("src");
        std::os::unix::fs::symlink(project.join("secrets"), project.join("src/link")).expect("link");
        let mut workspace = Workspace::new(&project);
        workspace.permissions =
            PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)").with(RuleKind::Allow, "Edit(src/**)");
        let call = |tool: &str| kind(workspace.ruling(tool, "", &json!({ "file_path": "src/link/key" })));
        assert_eq!(call("Read"), Some(RuleKind::Deny), "the link leads into the denied folder");
        assert_eq!(call("Edit"), None, "allowed as written, not where it leads");
    }

    #[test]
    fn a_search_is_judged_by_the_folder_it_searches() {
        let mut workspace = Workspace::new("/work/rocket");
        workspace.permissions = PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)");
        let grep = |input| kind(workspace.ruling("Grep", "key", &input));
        assert_eq!(grep(json!({ "pattern": "key" })), Some(RuleKind::Deny), "the project holds the denied folder");
        assert_eq!(grep(json!({ "pattern": "key", "path": "src" })), None);
    }
}
