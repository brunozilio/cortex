use platform::{Bounds, Ended, Stop};
use serde_json::Value;
use std::{sync::Arc, time::Duration};

use super::{CommandOutput, Handler};
use crate::model::ModelError;
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

const ENDED_EARLY: &str = "hook cancelled or timed out";

async fn within<T>(timeout: u64, run: &Stop, work: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    tokio::select! {
        biased;
        () = run.wait() => Err(ENDED_EARLY.into()),
        () = tokio::time::sleep(Duration::from_secs(timeout)) => Err(ENDED_EARLY.into()),
        done = work => done,
    }
}

fn waited<T: Send>(
    timeout: u64,
    run: &Stop,
    work: impl Future<Output = Result<T, String>> + Send,
) -> Result<T, String> {
    platform::runtime::block_on(within(timeout, run, work)).unwrap_or_else(|error| Err(error.to_string()))
}

pub(super) fn bounded<T: Send>(
    timeout: u64,
    workspace: &Workspace,
    work: impl FnOnce(Arc<Stop>) -> Result<T, String> + Send,
) -> Result<T, String> {
    let own = Arc::new(Stop::new());
    let (send, receive) = tokio::sync::oneshot::channel();
    let handed = Arc::clone(&own);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let _ = send.send(work(handed));
        });
        let done = async { receive.await.unwrap_or_else(|_| Err("hook worker stopped".into())) };
        let outcome = waited(timeout, &workspace.stop, done);
        if outcome.is_err() {
            own.cancel();
        }
        outcome
    })
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
    let body = input.to_string();
    let hook = workspace.stop.token().child_token();
    let asked = async {
        let response = NetHttp.post_streaming(url, &headers, &body, &hook).await.map_err(|e| e.to_string())?;
        let status = response.status;
        let text = response.body.text(&hook, HTTP_OUTPUT_AT_MOST).await.map_err(|error| match error {
            ModelError::Provider(_) => "HTTP hook output exceeds 64 KiB".to_owned(),
            ModelError::Network(why) if why.contains("UTF-8") => "HTTP hook returned non UTF-8 output".to_owned(),
            error => error.to_string(),
        })?;
        let ok = (200..300).contains(&status);
        Ok(CommandOutput {
            code: Some(if ok { 0 } else { 1 }),
            stdout: if ok { text } else { String::new() },
            stderr: if ok { String::new() } else { format!("HTTP hook status {status}") },
        })
    };
    let outcome = waited(handler.timeout, &workspace.stop, asked);
    hook.cancel();
    outcome
}

const HTTP_OUTPUT_AT_MOST: usize = 64 * 1024;

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

#[cfg(test)]
mod scoped {
    use std::time::Instant;

    use super::*;

    fn silent_hook(timeout: u64) -> (Handler, std::net::TcpListener) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let url = format!("http://{}/hook", listener.local_addr().expect("address"));
        let handler = serde_json::from_value(serde_json::json!({ "type": "http", "url": url, "timeout": timeout }))
            .expect("handler");
        (handler, listener)
    }

    #[test]
    fn an_http_hook_that_runs_out_of_time_ends_alone_and_leaves_the_run_going() {
        let (handler, _listener) = silent_hook(1);
        let workspace = Workspace::new("/tmp");
        let started = Instant::now();
        assert_eq!(http(&handler, &serde_json::json!({}), &workspace).map(|_| ()), Err(ENDED_EARLY.into()));
        assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
        assert!(!workspace.stop.is_stopped(), "only the hook was ended");
    }

    #[test]
    fn cancelling_the_run_ends_a_waiting_http_hook_at_once() {
        let (handler, _listener) = silent_hook(600);
        let workspace = Workspace::new("/tmp");
        let stop = Arc::clone(&workspace.stop);
        let cancelled_at = Arc::new(std::sync::Mutex::new(None));
        let marked = Arc::clone(&cancelled_at);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            *marked.lock().expect("mark") = Some(Instant::now());
            stop.cancel();
        });
        assert!(http(&handler, &serde_json::json!({}), &workspace).is_err());
        let ended = Instant::now();
        let cancelled = cancelled_at.lock().expect("mark").expect("cancelled");
        assert!(ended.saturating_duration_since(cancelled) < Duration::from_secs(2), "{:?}", ended - cancelled);
    }

    #[test]
    fn a_hook_worker_s_timeout_stops_its_own_work_only() {
        let workspace = Workspace::new("/tmp");
        let ended = bounded(1, &workspace, |own: Arc<Stop>| {
            platform::runtime::block_on(async { own.wait().await }).map_err(|error| error.to_string())?;
            Ok(own.is_stopped())
        });
        assert_eq!(ended, Err(ENDED_EARLY.into()));
        assert!(!workspace.stop.is_stopped());
    }
}
