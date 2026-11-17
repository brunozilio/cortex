use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::bounded::{Bounds, Ended};
use serde_json::{Value, json};

use super::Confiner;
use super::host_judge::{Grounds, HostAsk, HostJudge};
use super::script_tool::{Capabilities, Capability, Origin, ScriptAccess};
use crate::model::CancellationToken;
use crate::model::http::{Http, NetHttp, Response, reason};
use crate::tools::builtin::web::{reach, under};

const OUTPUT: usize = 1 << 20;
pub const FETCHED: usize = 5 << 20;

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

pub fn fetch(arguments: &Value, origin: Origin, cancel: &CancellationToken) -> Result<Value, String> {
    fetch_over(&NetHttp, arguments, origin, cancel)
}

pub fn fetch_over(
    web: &dyn Http,
    arguments: &Value,
    origin: Origin,
    cancel: &CancellationToken,
) -> Result<Value, String> {
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
    under(cancel, async {
        let response = match origin {
            Origin::Owner => web.request_cancellable(url, &headers, body, cancel).await.map_err(reason)?,
            Origin::Project => public(web, url, &headers, body, cancel).await?,
        };
        let status = response.status;
        let bytes = response
            .body
            .capped(cancel, FETCHED)
            .await
            .map_err(|error| format!("Could not read {url}: {}", reason(error)))?;
        Ok(json!({ "status": status, "body": String::from_utf8_lossy(&bytes) }))
    })
}

async fn public(
    web: &dyn Http,
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
    cancel: &CancellationToken,
) -> Result<Response, String> {
    let Some(body) = body else { return reach::fetch(web, url, headers, cancel).await.map(|reached| reached.response) };
    let url = reach::secure(url);
    match reach::refusal(&url, web) {
        Some(why) => Err(why),
        None => web.one_hop(&url, headers, Some(body), cancel).await.map_err(reason),
    }
}

pub struct StandaloneHost {
    pub cwd: std::path::PathBuf,
    pub access: ScriptAccess,
    pub origin: Origin,
    pub confine: Confiner,
    pub stop: std::sync::Arc<AtomicBool>,
    pub cancel: CancellationToken,
    pub script: String,
    pub capabilities: Capabilities,
    pub judge: std::sync::Arc<HostJudge>,
    pub grounds: Grounds,
}

impl StandaloneHost {
    fn performed(&self, capability: Capability, arguments: &Value) -> Result<Value, String> {
        let text = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let resolve = |path: &str| {
            let path = std::path::Path::new(path);
            if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) }
        };
        match capability {
            Capability::ReadFile => {
                let path = resolve(arguments.as_str().unwrap_or_default());
                std::fs::read_to_string(&path)
                    .map(Value::String)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            Capability::WriteFile => {
                let path = resolve(&text("path"));
                platform::atomic::write(&path, text("text").as_bytes())
                    .map(|_| Value::Null)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            Capability::Exec => exec(&self.cwd, arguments, &self.stop, &self.confine),
            Capability::Fetch => fetch(arguments, self.origin, &self.cancel),
        }
    }
}

impl script::Host for StandaloneHost {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        if let Some(capability) = Capability::of(function) {
            self.capabilities.allows(capability, &self.script)?;
            capability.granted(self.access)?;
            let ask = HostAsk { capability, script: &self.script, arguments: &arguments };
            self.judge.judge(&ask, &self.grounds)?;
            return self.performed(capability, &arguments);
        }
        match function {
            "text" => Ok(Value::String(content::text(arguments.as_str().unwrap_or_default()))),
            "log" => {
                tracing::info!(message = arguments.as_str().unwrap_or_default(), "script");
                Ok(Value::Null)
            }
            other => Err(format!("ctx.{other} is not available here")),
        }
    }
}

#[cfg(test)]
#[path = "script_io_tests.rs"]
mod tests;
