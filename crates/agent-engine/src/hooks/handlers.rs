use platform::{Bounds, Ended};
use serde_json::Value;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use super::{CommandOutput, Handler};
use crate::tools::Workspace;

pub fn command(handler: &Handler, input: &Value, workspace: &Workspace) -> Result<CommandOutput, String> {
    let mut cmd = workspace.confined(platform::script_shell(&handler.command));
    cmd.env(concat!(brand::env_prefix!(), "_PROJECT_DIR"), &workspace.cwd);
    cmd.current_dir(&workspace.cwd);
    let input = Some(serde_json::to_vec(input).map_err(|e| e.to_string())?);
    let bounds =
        Bounds { timeout: Duration::from_secs(handler.timeout), max_output: 64 * 1024, stop: &workspace.stop, input };
    let done = platform::bounded::run(&mut cmd, bounds).map_err(|e| e.to_string())?;
    match done.ended {
        Ended::Exited(status) => Ok(CommandOutput { code: status.code(), stdout: done.stdout, stderr: done.stderr }),
        Ended::Stopped | Ended::TimedOut => Err("hook cancelled or timed out".into()),
    }
}

pub(super) fn bounded<T: Send>(
    timeout: u64,
    workspace: &Workspace,
    work: impl FnOnce(std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<T, String> + Send,
) -> Result<T, String> {
    {
        let signal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (send, receive) = std::sync::mpsc::channel();
        let signal_ref = signal.clone();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                let _ = send.send(work(signal_ref));
            });
            let start = Instant::now();
            loop {
                match receive.recv_timeout(Duration::from_millis(25)) {
                    Ok(result) => return result,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err("hook worker stopped".into()),
                    Err(_) => {}
                }
                if workspace.stop.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(timeout) {
                    signal.store(true, Ordering::Relaxed);
                    return Err("hook cancelled or timed out".into());
                }
            }
        })
    }
}

pub fn http(handler: &Handler, input: &Value, workspace: &Workspace) -> Result<CommandOutput, String> {
    use crate::model::http::{Http, NetHttp};
    let url = handler.url.as_deref().ok_or("HTTP hook requires url")?;
    let mut headers = Vec::new();
    let variables =
        regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").map_err(|e| e.to_string())?;
    for (name, value) in &handler.headers {
        let value = variables
            .replace_all(value, |captures: &regex::Captures<'_>| {
                let key = captures.get(1).or_else(|| captures.get(2)).map_or("", |found| found.as_str());
                if handler.allowed_env_vars.iter().any(|allowed| allowed == key) {
                    std::env::var(key).unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .into_owned();
        headers.push((name.clone(), value));
    }
    bounded(handler.timeout, workspace, |stop| {
        let mut response = NetHttp.post_stopped(url, &headers, &input.to_string(), &stop).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err("HTTP hook cancelled".into());
            }
            match response.body.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    bytes.extend_from_slice(&buffer[..count]);
                    if bytes.len() > 64 * 1024 {
                        return Err("HTTP hook output exceeds 64 KiB".into());
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        let ok = (200..300).contains(&response.status);
        Ok(CommandOutput {
            code: Some(if ok { 0 } else { 1 }),
            stdout: if ok {
                String::from_utf8(bytes).map_err(|_| "HTTP hook returned non UTF-8 output")?
            } else {
                String::new()
            },
            stderr: if ok { String::new() } else { format!("HTTP hook status {}", response.status) },
        })
    })
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;

    #[test]
    fn a_command_hook_is_confined_like_bash() {
        let dir = tempfile::tempdir().expect("dir");
        let settings = dir.path().join(brand::DIR).join("settings.json");
        std::fs::create_dir_all(dir.path().join(brand::DIR)).expect("folder");
        std::fs::write(&settings, "{}").expect("settings");
        let handler: Handler = serde_json::from_value(serde_json::json!({
            "type": "command",
            "command": format!("echo granted > {}/settings.json", brand::DIR)
        }))
        .expect("handler");
        let mut workspace = Workspace::new(dir.path());

        workspace.policy.sandbox = Some(domain::sandbox::Sandbox::default());
        let output = command(&handler, &serde_json::json!({}), &workspace).expect("ran");

        assert_ne!(output.code, Some(0));
        assert_eq!(std::fs::read_to_string(&settings).expect("read"), "{}");
    }

    #[cfg(unix)]
    #[test]
    fn what_a_hook_printed_reaches_the_model_without_its_trailing_newlines() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        let hooks = |event: &str, command: &str| {
            let groups = serde_json::json!({ event: [{"hooks": [{"type": "command", "command": command}]}] });
            super::super::Hooks::parse(&groups).expect("hooks")
        };

        let blocking = hooks("Stop", "printf 'not yet\\n\\n' >&2; exit 2");
        let blocked = blocking.run("Stop", "", serde_json::json!({}), &workspace).expect("ran");
        assert_eq!(blocked.blocked.as_deref(), Some("not yet"));

        let adding = hooks("UserPromptSubmit", "printf 'Team convention:\\n  tabs.\\n'");
        let added = adding.run("UserPromptSubmit", "", serde_json::json!({}), &workspace).expect("ran");
        assert_eq!(added.context, "Team convention:\n  tabs.");
    }
}
