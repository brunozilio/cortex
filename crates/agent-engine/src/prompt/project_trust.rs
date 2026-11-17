use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde_json::Value;

pub const OPT_IN: &str = "trustProjectHooks";

pub fn tracked(file: &Path) -> bool {
    let (Some(folder), Some(name)) = (file.parent(), file.file_name()) else { return false };
    let args = [OsStr::new("ls-files"), OsStr::new("--error-unmatch"), OsStr::new("--"), name];
    crate::agent_runtime::git::output(folder, &args, &AtomicBool::new(false)).is_ok()
}

#[derive(Debug)]
pub struct ProjectTrust {
    user_root: Option<PathBuf>,
    opted_in: bool,
    skipped: RefCell<Vec<PathBuf>>,
}

impl ProjectTrust {
    pub fn of(home: Option<&Path>) -> Self {
        let user_root = home.map(|home| home.join(brand::DIR));
        let opted_in = user_root.as_deref().is_some_and(opted_in);
        Self { user_root, opted_in, skipped: RefCell::default() }
    }

    fn users(&self, file: &Path) -> bool {
        self.user_root.as_deref().is_some_and(|root| file.starts_with(root))
    }

    pub fn local_is_the_owner_s(&self, file: &Path) -> bool {
        self.users(file) || !tracked(file)
    }

    pub fn runs_hooks_of(&self, file: &Path) -> bool {
        if self.users(file) || self.opted_in || !tracked(file) {
            return true;
        }
        tracing::warn!(file = %file.display(), "a versioned project file's hooks are skipped");
        self.skipped.borrow_mut().push(file.to_path_buf());
        false
    }

    pub fn notices(&self) -> Vec<String> {
        let skipped = self.skipped.borrow();
        if skipped.is_empty() {
            return Vec::new();
        }
        let files: Vec<String> = skipped.iter().map(|file| file.display().to_string()).collect();
        let settings = self.user_root.as_deref().map_or_else(
            || format!("~/{}/settings.json", brand::DIR),
            |root| root.join("settings.json").display().to_string(),
        );
        vec![format!(
            "This project's hooks were skipped: {} {} versioned in git, and workspace trust is not implemented yet, \
             so code a repository brings is not run for you. To run them as before, set \"{OPT_IN}\": true in {settings}.",
            files.join(", "),
            if files.len() == 1 { "is" } else { "are" },
        )]
    }
}

fn opted_in(user_root: &Path) -> bool {
    ["settings.json", "settings.local.json", super::settings::MANAGED]
        .iter()
        .filter_map(|name| std::fs::read_to_string(user_root.join(name)).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .filter_map(|settings| settings.get(OPT_IN).and_then(Value::as_bool))
        .next_back()
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "project_trust_tests.rs"]
mod tests;
