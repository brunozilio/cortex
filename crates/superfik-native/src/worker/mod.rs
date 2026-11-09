//! Process boundary for server file and shell tools. Only a tool request and
//! the member's GitHub identity cross it; provider credentials stay in backend.
//!
//! This module is the engine's side; `serve` is the worker process's.
mod serve;

pub use self::serve::{main, mcp_main};
use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::Stdio,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
#[derive(Serialize, Deserialize)]
struct Request {
    name: String,
    input: Value,
    cwd: PathBuf,
    github: Option<String>,
    read: bool,
}
struct Remote {
    tool: Box<dyn Tool>,
    program: PathBuf,
}
impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteTool").field("name", &self.tool.spec().name).finish()
    }
}
impl Tool for Remote {
    fn spec(&self) -> ToolSpec {
        self.tool.spec()
    }
    fn access(&self) -> Access {
        self.tool.access()
    }
    fn reaches_outside(&self, input: &Value, workspace: &Workspace) -> Option<PathBuf> {
        self.tool.reaches_outside(input, workspace)
    }
    fn summary(&self, input: &Value) -> String {
        self.tool.summary(input)
    }
    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        match run(&self.program, &self.tool.spec().name, input, workspace) {
            Ok(out) => out,
            Err(e) => Outcome::error(format!("Isolated tool failed: {e}")),
        }
    }
}
pub fn wrap(tools: Vec<Box<dyn Tool>>, program: Option<&std::path::Path>) -> Vec<Box<dyn Tool>> {
    tools
        .into_iter()
        .map(|tool| {
            if let Some(program) = program
                .filter(|_| matches!(tool.spec().name.as_str(), "Read" | "Write" | "Edit" | "Glob" | "Grep" | "Bash"))
            {
                Box::new(Remote { tool, program: program.into() }) as Box<dyn Tool>
            } else {
                tool
            }
        })
        .collect()
}
pub fn clean_env(command: &mut std::process::Command) {
    command.env_clear();
    for key in [
        "PATH",
        "HOME",
        "USER",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "SystemRoot",
        "TEMP",
        "TMP",
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "SUPERFIK_DISABLE_AUTO_MEMORY",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}
fn run(program: &std::path::Path, name: &str, input: &Value, workspace: &Workspace) -> Result<Outcome, String> {
    let path = input["file_path"].as_str().map(|p| workspace.resolve(p));
    let request = Request {
        name: name.into(),
        input: input.clone(),
        cwd: workspace.cwd.clone(),
        github: workspace.credentials.github.clone(),
        read: path.as_ref().is_some_and(|p| workspace.has_read(p)),
    };
    let mut command = superfik_process::command(program);
    clean_env(&mut command);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut tree = superfik_process::Tree::spawn(&mut command).map_err(|e| e.to_string())?;
    let mut stdin = tree.child().stdin.take().ok_or("missing worker stdin")?;
    serde_json::to_writer(&mut stdin, &request).map_err(|e| e.to_string())?;
    stdin.write_all(b"\n").and_then(|_| stdin.flush()).map_err(|e| e.to_string())?;
    let stdout = tree.child().stdout.take().ok_or("missing worker stdout")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(64 * 1024 * 1024).read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut input = Some(stdin);
    let mut cancelled_at = None;
    let deadline = Instant::now() + Duration::from_secs(610);
    let status = loop {
        if let Some(status) = tree.child().try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if workspace.stop.load(Ordering::Relaxed) || Instant::now() > deadline {
            if cancelled_at.is_none() {
                input.take();
                cancelled_at = Some(Instant::now());
            }
            if cancelled_at.is_some_and(|t: Instant| t.elapsed() > Duration::from_secs(2)) {
                tree.kill();
                break tree.child().wait().map_err(|e| e.to_string())?;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let bytes = reader.join().map_err(|_| "worker output reader stopped")?.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("worker exited unsuccessfully".into());
    }
    let outcome: Outcome = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if name == "Read"
        && !outcome.is_error
        && let Some(path) = path
    {
        workspace.mark_read(&path);
    }
    Ok(outcome)
}

mod requests;
pub use requests::{brief, operation, read_text};
