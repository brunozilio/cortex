use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::{Value, json};

use super::script_tool::ScriptAccess;

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

pub fn render(path: &Path, args: &str, cwd: &Path, access: ScriptAccess) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let stop = Arc::new(AtomicBool::new(false));
    let host: Arc<dyn script::Host> =
        Arc::new(super::script_io::StandaloneHost { cwd: cwd.to_path_buf(), access, stop: Arc::clone(&stop) });
    let data = json!({ "cwd": cwd.display().to_string(), "args": args, "base": path.parent().map(|dir| dir.display().to_string()) });
    let shown = path.display().to_string();
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
