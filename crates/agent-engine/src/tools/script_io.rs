use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::{Value, json};
use subprocess::bounded::{Bounds, Ended};

use crate::model::http::{Http, NetHttp};

const OUTPUT: usize = 1 << 20;

pub fn exec(cwd: &Path, arguments: &Value, stop: &AtomicBool) -> Result<Value, String> {
    let command =
        arguments["command"].as_str().filter(|command| !command.trim().is_empty()).ok_or("ctx.exec needs a command")?;
    let seconds = arguments["timeout"].as_u64().unwrap_or(120);
    let mut process = if cfg!(windows) {
        let mut process = subprocess::command("cmd");
        process.args(["/C", command]);
        process
    } else {
        let mut process = subprocess::command("sh");
        process.args(["-c", command]);
        process
    };
    process.current_dir(cwd);
    let input = arguments["input"].as_str().map(|text| text.as_bytes().to_vec());
    let bounds = Bounds { timeout: Duration::from_secs(seconds), max_output: OUTPUT, stop, input };
    let done = subprocess::bounded::run(&mut process, bounds).map_err(|error| error.to_string())?;
    let code = match done.ended {
        Ended::Exited(status) => status.code().map_or(Value::Null, |code| json!(code)),
        Ended::Stopped => return Err("cancelled".into()),
        Ended::TimedOut => return Err(format!("the command ran past {seconds} s")),
    };
    Ok(json!({ "code": code, "stdout": done.stdout, "stderr": done.stderr }))
}

pub fn fetch(arguments: &Value) -> Result<Value, String> {
    let url = arguments["url"].as_str().ok_or("ctx.fetch needs a url")?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("{url} is not an http(s) address"));
    }
    let headers: Vec<(String, String)> = arguments["headers"]
        .as_object()
        .map(|headers| {
            headers.iter().map(|(key, value)| (key.clone(), value.as_str().unwrap_or_default().to_owned())).collect()
        })
        .unwrap_or_default();
    let response = match arguments["body"].as_str() {
        Some(body) => NetHttp.post(url, &headers, body),
        None => NetHttp.get(url, &headers),
    }?;
    let status = response.status;
    let body = response.text_stopped(&AtomicBool::new(false)).map_err(|error| error.to_string())?;
    Ok(json!({ "status": status, "body": body }))
}

pub struct StandaloneHost {
    pub cwd: std::path::PathBuf,
    pub access: super::script_tool::ScriptAccess,
    pub stop: std::sync::Arc<AtomicBool>,
}

impl script::Host for StandaloneHost {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        let text = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let resolve = |path: &str| {
            let path = std::path::Path::new(path);
            if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) }
        };
        match function {
            "text" => Ok(Value::String(content::text(arguments.as_str().unwrap_or_default()))),
            "log" => {
                tracing::info!(message = arguments.as_str().unwrap_or_default(), "script");
                Ok(Value::Null)
            }
            "readFile" if self.access.files => {
                let path = resolve(arguments.as_str().unwrap_or_default());
                std::fs::read_to_string(&path)
                    .map(Value::String)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            "writeFile" if self.access.files => {
                let path = resolve(&text("path"));
                std::fs::write(&path, text("text"))
                    .map(|()| Value::Null)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            "exec" if self.access.processes => exec(&self.cwd, &arguments, &self.stop),
            "fetch" if self.access.network => fetch(&arguments),
            "readFile" | "writeFile" => Err("scripts may not touch files here (settings: scripts.files)".into()),
            "exec" => Err("scripts may not start processes here (settings: scripts.processes)".into()),
            "fetch" => Err("scripts may not reach the network here (settings: scripts.network)".into()),
            other => Err(format!("ctx.{other} is not available here")),
        }
    }
}
