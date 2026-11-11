use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::Value;

const LIMIT: Duration = Duration::from_secs(5);

struct PromptHost;

impl script::Host for PromptHost {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        match function {
            "text" => Ok(Value::String(content::text(arguments.as_str().unwrap_or_default()))),
            "include" => {
                let path = arguments["path"].as_str().unwrap_or_default();
                Ok(Value::String(render(path, &arguments["data"])))
            }
            other => Err(format!("a prompt cannot call {other}")),
        }
    }
}

fn run(path: &str, source: &str, data: &Value) -> Result<String, String> {
    let host: Arc<dyn script::Host> = Arc::new(PromptHost);
    let stop = Arc::new(AtomicBool::new(false));
    let value = script::run_within(path, source, "default", &[], data, host, stop, Some(LIMIT))?;
    Ok(value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()))
}

pub fn render(path: &str, data: &Value) -> String {
    render_in(content::user_folder().as_deref(), path, data)
}

pub fn render_in(folder: Option<&std::path::Path>, path: &str, data: &Value) -> String {
    if let Some(mine) = content::user_text(folder, path) {
        match run(path, &mine, data) {
            Ok(text) => return text,
            Err(error) => tracing::warn!(%error, path, "the user's prompt failed; the built-in one is used"),
        }
    }
    let Some(builtin) = content::builtin_text(path) else {
        tracing::error!(path, "no such prompt");
        return String::new();
    };
    run(path, &builtin, data).unwrap_or_else(|error| {
        tracing::error!(%error, path, "the built-in prompt failed");
        String::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(folder: &std::path::Path, path: &str, source: &str) {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
        std::fs::write(file, source).expect("write");
    }

    #[test]
    fn the_user_s_prompt_replaces_the_built_in_one_and_a_broken_one_falls_back() {
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
    fn a_prompt_that_never_ends_is_stopped() {
        let folder = tempfile::tempdir().expect("folder");
        write(folder.path(), "prompts/reminder.js", "export default () => { for (;;) {} };");
        let started = std::time::Instant::now();
        let data = serde_json::json!({ "mode": "ask", "depth": 0, "style": null });
        assert!(render_in(Some(folder.path()), "prompts/reminder.js", &data).contains("# Runtime reminder"));
        assert!(started.elapsed() < LIMIT + Duration::from_secs(3));
    }
}
