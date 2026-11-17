use std::path::{Component, Path, PathBuf};

use domain::permission::{Call, Ruling, ruling_in};
use domain::permission_rules::PermissionRules;
use serde_json::Value;

use super::{Tool, Workspace};

impl Workspace {
    pub fn ruling(&self, tool: &dyn Tool, input: &Value) -> Ruling {
        self.ruling_with(&self.policy.permissions, tool, input)
    }

    pub fn ruling_with(&self, rules: &PermissionRules, tool: &dyn Tool, input: &Value) -> Ruling {
        if rules.is_empty() {
            return None;
        }
        let (name, subject, effects) = (tool.spec().name, tool.summary(input), tool.effects(input, self));
        let cwd = self.cwd.to_string_lossy();
        let home = self.home.as_ref().map(|home| home.to_string_lossy().into_owned());
        ruling_in(rules, &Call { tool: &name, subject: &subject, effects: &effects }, &cwd, home.as_deref())
    }
}

pub(super) fn folded(path: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use domain::permission_rules::{PermissionRules, RuleKind};
    use serde_json::json;

    use super::*;

    fn kind(ruling: Ruling) -> Option<RuleKind> {
        ruling.map(|(kind, _)| kind)
    }

    fn ruled(workspace: &Workspace, tool: &str, input: &Value) -> Option<RuleKind> {
        let tool = crate::tools::named(tool).expect("a standard tool");
        kind(workspace.ruling(&*tool, input))
    }

    #[test]
    fn a_path_cannot_climb_around_a_rule() {
        let mut workspace = Workspace::new("/work/rocket");
        workspace.policy.permissions = PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)");
        let read = |path: &str| ruled(&workspace, "Read", &json!({ "file_path": path }));
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
        workspace.policy.permissions =
            PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)").with(RuleKind::Allow, "Edit(src/**)");
        let call = |tool: &str| ruled(&workspace, tool, &json!({ "file_path": "src/link/key" }));
        assert_eq!(call("Read"), Some(RuleKind::Deny), "the link leads into the denied folder");
        assert_eq!(call("Edit"), None, "allowed as written, not where it leads");
    }

    #[test]
    fn a_search_is_judged_by_the_folder_it_searches() {
        let mut workspace = Workspace::new("/work/rocket");
        workspace.policy.permissions = PermissionRules::default().with(RuleKind::Deny, "Read(secrets/**)");
        let grep = |input| ruled(&workspace, "Grep", &input);
        assert_eq!(grep(json!({ "pattern": "key" })), Some(RuleKind::Deny), "the project holds the denied folder");
        assert_eq!(grep(json!({ "pattern": "key", "path": "src" })), None);
    }
}
