mod git;

use std::path::{Path, PathBuf};

use domain::Principal;
use domain::changes::DiffScope;
use serde_json::{Value, json};

const MENTIONED: usize = 5;
const KEPT: usize = 50_000;

pub fn handle(_client: u64, _principal: &Principal, input: Value) -> Result<Value, String> {
    let path: PathBuf = input["path"].as_str().map(PathBuf::from).ok_or("the workspace feature needs a path")?;
    match input["op"].as_str().unwrap_or_default() {
        "changes" => {
            let scope: DiffScope = serde_json::from_value(input["scope"].clone()).map_err(|e| e.to_string())?;
            serde_json::to_value(git::changes_in(&path, scope)?).map_err(|e| e.to_string())
        }
        "files" => Ok(json!(git::files(&path))),
        "mentions" => Ok(json!(with_files(&path, input["prompt"].as_str().unwrap_or_default()))),
        other => Err(format!("the workspace feature has no {other:?}")),
    }
}

pub fn forget(_client: u64) {}

fn with_files(cwd: &Path, prompt: &str) -> String {
    let mut out = prompt.to_owned();
    let mut seen = Vec::new();
    for word in prompt.split_whitespace() {
        let Some(path) = word.strip_prefix('@').map(|p| p.trim_end_matches([',', '.', ':', ';', ')', '?', '!'])) else {
            continue;
        };
        if path.is_empty() || seen.contains(&path) || seen.len() >= MENTIONED {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(cwd.join(path)) else { continue };
        seen.push(path);
        let kept: String = text.chars().take(KEPT).collect();
        let cut = if kept.len() < text.len() { "\n[… cut]" } else { "" };
        out.push_str(&format!("\n\n<file path=\"{path}\">\n{kept}{cut}\n</file>"));
    }
    out
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
