use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::{Value, json};

use super::Confiner;
use super::host_judge::{Grounds, HostJudge};
use super::script_tool::{Capabilities, Declaration, Origin, ScriptAccess};

pub struct Described {
    pub name: Option<String>,
    pub description: String,
    pub model_invocation: bool,
}

pub fn is_script(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "js")
}

pub fn describe(path: &Path) -> Option<Described> {
    let source = std::fs::read_to_string(path).ok()?;
    let shown = path.display().to_string();
    let read = |name: &str| script::exported(&shown, &source, name).ok().filter(|value| !value.is_null());
    let text = |name: &str| read(name).and_then(|value| value.as_str().map(str::to_owned));
    Some(Described {
        name: text("name"),
        description: text("description").unwrap_or_default(),
        model_invocation: read("modelInvocation").and_then(|value| value.as_bool()).unwrap_or(true),
    })
}

#[derive(Debug, Clone)]
pub struct Granted {
    pub access: ScriptAccess,
    pub origin: Origin,
    pub confine: Confiner,
    pub judge: Arc<HostJudge>,
    pub grounds: Grounds,
}

pub fn render(path: &Path, args: &str, cwd: &Path, granted: &Granted) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let shown = path.display().to_string();
    let listed = script::exported(&shown, &source, "capabilities").ok();
    let capabilities =
        Capabilities::listed(listed.as_ref(), Declaration::Export).map_err(|error| format!("{shown}: {error}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let host: Arc<dyn script::Host> = Arc::new(super::script_io::StandaloneHost {
        cwd: cwd.to_path_buf(),
        access: granted.access,
        origin: granted.origin,
        confine: granted.confine.clone(),
        stop: Arc::clone(&stop),
        cancel: crate::model::CancellationToken::new(),
        script: format!("the skill script {shown}"),
        capabilities,
        judge: Arc::clone(&granted.judge),
        grounds: granted.grounds.clone(),
    });
    let data = json!({ "cwd": cwd.display().to_string(), "args": args, "base": path.parent().map(|dir| dir.display().to_string()) });
    let value = script::run_within(
        &shown,
        &source,
        "default",
        &[json!(args)],
        &data,
        host,
        stop,
        Some(Duration::from_secs(60)),
    )?;
    Ok(match value {
        Value::String(text) => text,
        Value::Null => String::new(),
        other => other.to_string(),
    })
}
