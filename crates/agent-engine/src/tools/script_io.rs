use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::bounded::{Bounds, Ended};
use serde_json::{Value, json};

use super::Confiner;
use super::script_tool::{Origin, ScriptAccess};
use crate::model::http::{Http, NetHttp, Response};
use crate::tools::builtin::web::reach;

const OUTPUT: usize = 1 << 20;
pub const FETCHED: u64 = 5 << 20;

pub fn exec(cwd: &Path, arguments: &Value, stop: &AtomicBool, confine: &Confiner) -> Result<Value, String> {
    let command =
        arguments["command"].as_str().filter(|command| !command.trim().is_empty()).ok_or("ctx.exec needs a command")?;
    let seconds = arguments["timeout"].as_u64().unwrap_or(120);
    let mut process = confine.confined(platform::script_shell(command));
    process.current_dir(cwd);
    let input = arguments["input"].as_str().map(|text| text.as_bytes().to_vec());
    let bounds = Bounds { timeout: Duration::from_secs(seconds), max_output: OUTPUT, stop, input };
    let done = platform::bounded::run(&mut process, bounds).map_err(|error| error.to_string())?;
    let code = match done.ended {
        Ended::Exited(status) => status.code().map_or(Value::Null, |code| json!(code)),
        Ended::Stopped => return Err("cancelled".into()),
        Ended::TimedOut => return Err(format!("the command ran past {seconds} s")),
    };
    Ok(json!({ "code": code, "stdout": done.stdout, "stderr": done.stderr }))
}

pub fn fetch(arguments: &Value, origin: Origin) -> Result<Value, String> {
    fetch_over(&NetHttp, arguments, origin)
}

pub fn fetch_over(web: &dyn Http, arguments: &Value, origin: Origin) -> Result<Value, String> {
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
    let body = arguments["body"].as_str();
    let mut response = match origin {
        Origin::Owner => web.streamed(url, &headers, body)?,
        Origin::Project => public(web, url, &headers, body)?,
    };
    let status = response.status;
    let bytes = crate::model::web_client::read_capped(response.body.as_mut(), FETCHED)
        .map_err(|error| format!("Could not read {url}: {error}"))?;
    Ok(json!({ "status": status, "body": String::from_utf8_lossy(&bytes) }))
}

fn public(web: &dyn Http, url: &str, headers: &[(String, String)], body: Option<&str>) -> Result<Response, String> {
    let Some(body) = body else { return reach::fetch(web, url, headers).map(|reached| reached.response) };
    let url = reach::secure(url);
    match reach::refusal(&url, web) {
        Some(why) => Err(why),
        None => web.one_hop(&url, headers, Some(body)),
    }
}

pub struct StandaloneHost {
    pub cwd: std::path::PathBuf,
    pub access: ScriptAccess,
    pub origin: Origin,
    pub confine: Confiner,
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
            "exec" if self.access.processes => exec(&self.cwd, &arguments, &self.stop, &self.confine),
            "fetch" if self.access.network => fetch(&arguments, self.origin),
            "readFile" | "writeFile" => Err("scripts may not touch files here (settings: scripts.files)".into()),
            "exec" => Err("scripts may not start processes here (settings: scripts.processes)".into()),
            "fetch" => Err("scripts may not reach the network here (settings: scripts.network)".into()),
            other => Err(format!("ctx.{other} is not available here")),
        }
    }
}

#[cfg(test)]
#[path = "script_io_tests.rs"]
mod tests;
