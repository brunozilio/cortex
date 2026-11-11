use serde_json::Value;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use subprocess::{Bounds, Ended};

use super::{CommandOutput, Handler};
use crate::tools::Workspace;

pub fn command(handler: &Handler, input: &Value, workspace: &Workspace) -> Result<CommandOutput, String> {
    #[cfg(windows)]
    let mut cmd = subprocess::WindowsShell::detect().command(&handler.command);
    #[cfg(not(windows))]
    let mut cmd = {
        let mut cmd = subprocess::command("/bin/sh");
        cmd.args(["-c", &handler.command]);
        cmd
    };
    cmd.env(concat!(brand::env_prefix!(), "_PROJECT_DIR"), &workspace.cwd);
    cmd.current_dir(&workspace.cwd);
    let input = Some(serde_json::to_vec(input).map_err(|e| e.to_string())?);
    let bounds =
        Bounds { timeout: Duration::from_secs(handler.timeout), max_output: 64 * 1024, stop: &workspace.stop, input };
    let done = subprocess::bounded::run(&mut cmd, bounds).map_err(|e| e.to_string())?;
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
        regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").expect("static env pattern");
    for (name, value) in &handler.headers {
        let value = variables
            .replace_all(value, |captures: &regex::Captures<'_>| {
                let key = captures.get(1).or_else(|| captures.get(2)).expect("variable").as_str();
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

pub fn script(handler: &Handler, input: &Value, workspace: &Workspace) -> Result<CommandOutput, String> {
    let path = std::path::PathBuf::from(&handler.command);
    let source = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let host: std::sync::Arc<dyn script::Host> = std::sync::Arc::new(crate::tools::script_io::StandaloneHost {
        cwd: workspace.cwd.clone(),
        access: workspace.scripts,
        stop: std::sync::Arc::clone(&workspace.stop),
    });
    let data =
        serde_json::json!({ "cwd": workspace.cwd, "session": workspace.session_id, "agent": workspace.agent_name });
    let limit = std::time::Duration::from_secs(handler.timeout);
    let stop = std::sync::Arc::clone(&workspace.stop);
    let shown = path.display().to_string();
    Ok(
        match script::run_within(
            &shown,
            &source,
            "default",
            std::slice::from_ref(input),
            &data,
            host,
            stop,
            Some(limit),
        ) {
            Ok(Value::Null) => CommandOutput { code: Some(0), stdout: String::new(), stderr: String::new() },
            Ok(Value::String(text)) => CommandOutput { code: Some(0), stdout: text, stderr: String::new() },
            Ok(value) => CommandOutput { code: Some(0), stdout: value.to_string(), stderr: String::new() },
            Err(error) => CommandOutput { code: Some(1), stdout: String::new(), stderr: error },
        },
    )
}
