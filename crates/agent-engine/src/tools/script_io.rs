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
