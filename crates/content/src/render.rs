use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

pub const LIMIT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct ContentHost {
    calls: AtomicUsize,
}

impl script::Host for ContentHost {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        match function {
            "text" => Ok(Value::String(super::text(arguments.as_str().unwrap_or_default()))),
            "include" => {
                let path = arguments["path"].as_str().unwrap_or_default();
                Ok(Value::String(render(path, &arguments["data"])))
            }
            other => Err(format!("a prompt cannot call {other}")),
        }
    }
}

fn rendered() -> &'static Mutex<HashMap<u64, Value>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Value>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn values() -> &'static Mutex<HashMap<u64, Option<Value>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Option<Value>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn key(parts: &[&str]) -> u64 {
    let mut hasher = DefaultHasher::new();
    parts.hash(&mut hasher);
    hasher.finish()
}

fn shown(value: Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())
}

fn run(path: &str, source: &str, export: &str, data: &Value) -> Result<Value, String> {
    let data_text = data.to_string();
    let cached = key(&[path, source, export, &data_text]);
    if let Some(value) = rendered().lock().ok().and_then(|cache| cache.get(&cached).cloned()) {
        return Ok(value);
    }
    let host = Arc::new(ContentHost::default());
    let stop = Arc::new(AtomicBool::new(false));
    let shared: Arc<dyn script::Host> = host.clone();
    let value = script::run_within(path, source, export, &[], data, shared, stop, Some(LIMIT))?;
    if host.calls.load(Ordering::Relaxed) == 0
        && let Ok(mut cache) = rendered().lock()
    {
        cache.insert(cached, value.clone());
    }
    Ok(value)
}

pub fn render(path: &str, data: &Value) -> String {
    render_export_in(super::folder().as_deref(), path, "default", data).unwrap_or_default()
}

pub fn render_export(path: &str, export: &str, data: &Value) -> Option<String> {
    render_export_in(super::folder().as_deref(), path, export, data)
}

pub fn render_in(folder: Option<&Path>, path: &str, data: &Value) -> String {
    render_export_in(folder, path, "default", data).unwrap_or_default()
}

pub fn render_export_in(folder: Option<&Path>, path: &str, export: &str, data: &Value) -> Option<String> {
    call_in(folder, path, export, data).map(shown)
}

pub fn call(path: &str, export: &str, data: &Value) -> Option<Value> {
    call_in(super::folder().as_deref(), path, export, data)
}

pub fn call_in(folder: Option<&Path>, path: &str, export: &str, data: &Value) -> Option<Value> {
    if let Some(mine) = super::user_text(folder, path) {
        match run(path, &mine, export, data) {
            Ok(value) => return Some(value),
            Err(error) => tracing::warn!(%error, path, "the user's file failed; the built-in one is used"),
        }
    }
    let builtin = super::builtin_text(path)?;
    run(path, &builtin, export, data)
        .inspect_err(|error| tracing::error!(%error, path, export, "the built-in file failed"))
        .ok()
}

pub fn value(path: &str, export: &str) -> Option<Value> {
    value_in(super::folder().as_deref(), path, export)
}

pub fn value_in(folder: Option<&Path>, path: &str, export: &str) -> Option<Value> {
    let read = |source: String| -> Option<Value> {
        let cached = key(&[path, &source, export]);
        if let Some(found) = values().lock().ok().and_then(|cache| cache.get(&cached).cloned()) {
            return found;
        }
        let found = script::exported(path, &source, export).ok().filter(|value| !value.is_null());
        if let Ok(mut cache) = values().lock() {
            cache.insert(cached, found.clone());
        }
        found
    };
    super::user_text(folder, path).and_then(read).or_else(|| super::builtin_text(path).and_then(read))
}

pub fn exports(path: &str, export: &str) -> bool {
    let source = super::user_text(super::folder().as_deref(), path).or_else(|| super::builtin_text(path));
    source.is_some_and(|source| {
        let cached = key(&[path, &source, export, "exports"]);
        if let Some(found) = values().lock().ok().and_then(|cache| cache.get(&cached).cloned()) {
            return found.is_some();
        }
        let found = script::exports(path, &source, export).then_some(Value::Bool(true));
        if let Ok(mut cache) = values().lock() {
            cache.insert(cached, found.clone());
        }
        found.is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(folder: &Path, path: &str, source: &str) {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
        std::fs::write(file, source).expect("write");
    }

    #[test]
    fn the_user_s_file_replaces_the_built_in_one_and_a_broken_one_falls_back() {
        let folder = tempfile::tempdir().expect("folder");
        let data = serde_json::json!({ "mode": "ask", "depth": 0, "style": null });
        let built_in = render_in(None, "prompts/reminder.js", &data);
        assert!(built_in.contains("# Runtime reminder"), "{built_in}");
        write(folder.path(), "prompts/reminder.js", "export default (ctx) => `mode is ${ctx.mode}`;");
        assert_eq!(render_in(Some(folder.path()), "prompts/reminder.js", &data), "mode is ask");
        write(folder.path(), "prompts/reminder.js", "export default () => { throw new Error('mine broke') };");
        assert_eq!(render_in(Some(folder.path()), "prompts/reminder.js", &data), built_in);
    }

    #[test]
    fn a_file_that_never_ends_is_stopped() {
        let folder = tempfile::tempdir().expect("folder");
        write(folder.path(), "prompts/reminder.js", "export default () => { for (;;) {} };");
        let started = std::time::Instant::now();
        let data = serde_json::json!({ "mode": "ask", "depth": 0, "style": null });
        assert!(render_in(Some(folder.path()), "prompts/reminder.js", &data).contains("# Runtime reminder"));
        assert!(started.elapsed() < LIMIT + Duration::from_secs(3));
    }

    #[test]
    fn a_static_export_is_read_as_a_value_and_a_text_export_renders_as_is() {
        let folder = tempfile::tempdir().expect("folder");
        write(
            folder.path(),
            "tools/Echo.js",
            "export const spec = { name: 'Echo', input: { type: 'object' }, description: (ctx) => `says ${ctx.word}` };\nexport const note = 'plain';",
        );
        let spec = value_in(Some(folder.path()), "tools/Echo.js", "spec").expect("spec");
        assert_eq!(spec["name"], "Echo");
        assert_eq!(spec["input"]["type"], "object");
        assert!(spec.get("description").is_none(), "a function is not a value");
        let data = serde_json::json!({ "word": "hi" });
        assert_eq!(
            render_export_in(Some(folder.path()), "tools/Echo.js", "spec.description", &data).as_deref(),
            Some("says hi")
        );
        assert_eq!(
            render_export_in(Some(folder.path()), "tools/Echo.js", "note", &Value::Null).as_deref(),
            Some("plain")
        );
    }
}
