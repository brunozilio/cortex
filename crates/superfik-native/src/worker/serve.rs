//! The worker process's side: answering one tool, setup or hook request
//! read from stdin, or handing stdin over to an MCP server process.

use serde::Deserialize;
use std::{
    io::{BufRead, Read, Write},
    sync::atomic::Ordering,
};

use super::{Request, clean_env};
use crate::tools::{Outcome, Workspace};

pub fn main() -> Result<(), String> {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let mut line = String::new();
    (&mut stdin).take(2 * 1024 * 1024).read_line(&mut line).map_err(|e| e.to_string())?;
    let request: Request = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    let workspace = Workspace::new(request.cwd);
    let workspace = Workspace {
        credentials: superfik_core::credentials::RunCredentials { github: request.github, ..Default::default() },
        ..workspace
    };
    if request.read
        && let Some(path) = request.input["file_path"].as_str()
    {
        workspace.mark_read(&workspace.resolve(path));
    }
    let stop = workspace.stop.clone();
    std::thread::spawn(move || {
        let mut byte = [0];
        let _ = stdin.read(&mut byte);
        stop.store(true, Ordering::Relaxed);
    });
    if matches!(request.name.as_str(), "AgentPrepare" | "AgentResume" | "AgentCleanup" | "HookCommand") {
        let result = match request.name.as_str() {
            "AgentPrepare" => {
                let config = serde_json::from_value(request.input["config"].clone()).map_err(|e| e.to_string())?;
                crate::agent_runtime::local_prepare(
                    &config,
                    request.input["name"].as_str().ok_or("missing agent name")?,
                    request.input["owner"].as_str().ok_or("missing owner scope")?,
                    &workspace,
                )
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
            }
            "AgentResume" => {
                let config = serde_json::from_value(request.input["config"].clone()).map_err(|e| e.to_string())?;
                let state = serde_json::from_value(request.input["state"].clone()).map_err(|e| e.to_string())?;
                crate::agent_runtime::local_resume(
                    &config,
                    request.input["name"].as_str().ok_or("missing name")?,
                    request.input["owner"].as_str().ok_or("missing owner")?,
                    &state,
                    &workspace,
                )
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
            }
            "AgentCleanup" => serde_json::from_value(request.input.clone())
                .map_err(|e| e.to_string())
                .and_then(|setup| crate::agent_runtime::local_cleanup(&setup))
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
            _ => serde_json::from_value::<crate::hooks::Handler>(request.input["handler"].clone())
                .map_err(|e| e.to_string())
                .and_then(|handler| {
                    if handler.kind == "http" {
                        crate::hooks::http(&handler, &request.input["input"], &workspace)
                    } else {
                        crate::hooks::command(&handler, &request.input["input"], &workspace)
                    }
                })
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        };
        let outcome = match result {
            Ok(value) => Outcome::ok(value.to_string()),
            Err(error) => Outcome::error(error),
        };
        return serde_json::to_writer(std::io::stdout(), &outcome).map_err(|e| e.to_string());
    }
    if request.name == "ReadSkill" {
        let result = request.input["file_path"]
            .as_str()
            .ok_or("missing skill path")
            .and_then(|p| std::fs::read_to_string(workspace.resolve(p)).map_err(|_| "skill could not be read"));
        let outcome = match result {
            Ok(text) => Outcome::ok(text),
            Err(error) => Outcome::error(error),
        };
        return serde_json::to_writer(std::io::stdout(), &outcome).map_err(|e| e.to_string());
    }
    let tool = crate::tools::standard()
        .into_iter()
        .find(|t| t.spec().name == request.name && request.name != "AskUserQuestion")
        .ok_or("unknown worker tool")?;
    let outcome = tool.run(&request.input, &workspace);
    serde_json::to_writer(std::io::stdout(), &outcome).map_err(|e| e.to_string())
}

pub fn mcp_main() -> Result<(), String> {
    #[derive(Deserialize)]
    struct Config {
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    }
    let mut header = String::new();
    std::io::stdin().read_line(&mut header).map_err(|e| e.to_string())?;
    let config: Config = serde_json::from_str(&header).map_err(|e| e.to_string())?;
    let mut command = superfik_process::command(config.command);
    clean_env(&mut command);
    command.args(config.args).envs(config.env);
    writeln!(std::io::stdout(), "superfik-mcp-ready")
        .and_then(|_| std::io::stdout().flush())
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().to_string())
    }
    #[cfg(not(unix))]
    {
        let status = command.status().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err("MCP process failed".into()) }
    }
}
