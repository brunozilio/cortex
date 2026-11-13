use std::sync::Arc;

use domain::permission_rules::PermissionRules;
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
}

impl Policy {
    pub fn fork(&self) -> Self {
        Self {
            permissions: self.permissions.clone(),
            pre_approved: PermissionRules::default(),
            scripts: self.scripts,
            safety: self.safety.as_ref().map(|safety| Arc::new(safety.fork())),
            sandbox: self.sandbox.clone(),
        }
    }
}
