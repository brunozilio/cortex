//! Conditions and native tabs exposed to the run-scoped MCP connection.
use serde_json::{Value, json};
use superfik_core::browser::*;

pub(super) fn tools() -> Vec<Value> {
    vec![
        json!({"name":DIALOG_TOOL,"description":"Inspect an open JavaScript dialog. Supply accept explicitly to accept or dismiss it; prompt supplies text for a prompt dialog. Never infer consent for a consequential action.","inputSchema":{"type":"object","properties":{"accept":{"type":"boolean"},"prompt":{"type":"string","maxLength":4096}}}}),
        json!({"name":WAIT_TOOL,"description":"Wait for accessibility text, a URL substring, or a snapshot ref to be exposed/hidden. With no condition, wait until the page finishes loading. An unmet condition returns a timeout error.","inputSchema":{"type":"object","properties":{"text":{"type":"string"},"url":{"type":"string"},"reference":{"type":"integer","minimum":1},"hidden":{"type":"boolean"},"timeout_ms":{"type":"integer","minimum":100,"maximum":30000}}}}),
        json!({"name":TABS_TOOL,"description":"List the primary page and native popup tabs in this conversation, with their ids and the active tab. OAuth popups retain their native opener and cookies.","inputSchema":{"type":"object","properties":{}}}),
        json!({"name":SWITCH_TAB_TOOL,"description":"Show and operate a tab from browser_tabs in this conversation's browser card.","inputSchema":{"type":"object","properties":{"tab":{"type":"integer","minimum":1}},"required":["tab"]}}),
        json!({"name":CLOSE_TAB_TOOL,"description":"Close a popup tab. Omit tab to close the selected popup. The conversation's primary page stays available.","inputSchema":{"type":"object","properties":{"tab":{"type":"integer","minimum":1}}}}),
        json!({"name":FILES_TOOL,"description":"List recent downloads, saved file paths and this conversation's upload staging directory. Copy user-requested files into that directory before browser_upload.","inputSchema":{"type":"object","properties":{}}}),
        json!({"name":UPLOAD_TOOL,"description":"Attach staged files using a file input or custom attach-button ref from browser_snapshot. Paths may be relative to the upload directory or absolute within it. An empty list clears a file input selection.","inputSchema":{"type":"object","properties":{"reference":{"type":"integer","minimum":1},"paths":{"type":"array","maxItems":16,"items":{"type":"string"}}},"required":["reference","paths"]}}),
    ]
}
fn tab(arguments: &Value) -> Result<u32, String> {
    arguments["tab"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or("Give a positive tab id from browser_tabs.".into())
}

pub(super) fn op(name: &str, arguments: &Value) -> Option<Result<BrowserOp, String>> {
    Some(match name {
        WAIT_TOOL => {
            serde_json::from_value::<BrowserWait>(arguments.clone()).map_err(|e| e.to_string()).and_then(|wait| {
                wait.validate()?;
                Ok(BrowserOp::Wait(wait))
            })
        }
        TABS_TOOL => Ok(BrowserOp::Tabs),
        FILES_TOOL => Ok(BrowserOp::Files),
        DIALOG_TOOL => {
            if arguments.get("accept").is_some_and(|v| !v.is_boolean())
                || arguments.get("prompt").is_some_and(|v| !v.is_string())
            {
                Err("accept must be a boolean and prompt must be text.".into())
            } else {
                let accept = arguments["accept"].as_bool();
                let prompt = arguments["prompt"].as_str().map(str::to_owned);
                if prompt.as_ref().is_some_and(|p| p.len() > 4096) || (prompt.is_some() && accept.is_none()) {
                    Err("A prompt needs an explicit accept decision and at most 4096 bytes.".into())
                } else {
                    Ok(BrowserOp::Dialog { accept, prompt })
                }
            }
        }
        UPLOAD_TOOL => {
            let reference = arguments["reference"].as_u64().and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0);
            let paths = serde_json::from_value::<Vec<String>>(arguments["paths"].clone());
            match (reference, paths) {
                (Some(reference), Ok(paths)) if paths.len() <= 16 => Ok(BrowserOp::Upload { reference, paths }),
                _ => Err("Give a file input reference and up to 16 staged paths.".into()),
            }
        }
        SWITCH_TAB_TOOL => tab(arguments).map(|tab| BrowserOp::SwitchTab { tab }),
        CLOSE_TAB_TOOL => {
            if arguments.get("tab").is_some() {
                tab(arguments).map(|tab| BrowserOp::CloseTab { tab: Some(tab) })
            } else {
                Ok(BrowserOp::CloseTab { tab: None })
            }
        }
        _ => return None,
    })
}
