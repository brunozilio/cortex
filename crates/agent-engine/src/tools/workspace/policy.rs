use std::path::Path;
use std::sync::Arc;

use domain::permission_rules::PermissionRules;
use domain::redact::Known;
use domain::sandbox::Sandbox;

use crate::guard::Safety;
use crate::tools::script_tool::ScriptAccess;

#[derive(Debug, Default)]
pub struct Policy {
    pub permissions: PermissionRules,
    pub pre_approved: PermissionRules,
    pub scripts: ScriptAccess,
    pub safety: Option<Arc<Safety>>,
    pub sandbox: Option<Sandbox>,
    pub redact: Option<Arc<Known>>,
    pub snapshots: bool,
    pub dependency_guard: bool,
}

impl Policy {
    pub fn fork(&self) -> Self {
        Self {
            permissions: self.permissions.clone(),
            pre_approved: PermissionRules::default(),
            scripts: self.scripts,
            safety: self.safety.as_ref().map(|safety| Arc::new(safety.fork())),
            sandbox: self.sandbox.clone(),
            redact: self.redact.clone(),
            snapshots: self.snapshots,
            dependency_guard: self.dependency_guard,
        }
    }

    pub fn redacted(&self, text: &str) -> String {
        match &self.redact {
            Some(known) => domain::redact::redact(text, known),
            None => text.to_owned(),
        }
    }
}

pub fn env_secrets(project: &Path) -> Known {
    let files = std::fs::read_dir(project).into_iter().flatten().flatten().filter(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        (name == ".env" || name.starts_with(".env.")) && !name.ends_with(".example") && !name.ends_with(".sample")
    });
    let texts: Vec<String> = files.filter_map(|entry| std::fs::read_to_string(entry.path()).ok()).collect();
    Known::from_env_files(texts.iter().map(String::as_str))
}
