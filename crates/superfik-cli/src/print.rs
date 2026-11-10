//! `superfik -p`: one prompt, the reply on stdout, the agent's steps on
//! stderr. Nobody is there to answer, so whatever the agent asks is
//! denied (pick `--mode bypass` or `edits` to let it act).

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use superfik_sdk::{Decision, EngineEvent, PermissionMode, Superfik};

pub fn print(
    superfik: &Superfik,
    cwd: PathBuf,
    wanted: Option<&str>,
    mode: Option<PermissionMode>,
    prompt: &str,
) -> Result<ExitCode, String> {
    let models = superfik.models().map_err(|e| e.to_string())?;
    let remembered = superfik.settings().ok().and_then(|s| s["model"].as_str().map(str::to_owned));
    let model = superfik_core::model::choose_model(&models, wanted.or(remembered.as_deref()))
        .ok_or("no model is available: run `superfik login` first")?;
    if let Some(wanted) = wanted
        && !model.is_named(wanted)
    {
        return Err(format!("no model named {wanted}: `superfik models` lists them"));
    }
    let mut session = superfik.session(cwd, model);
    if let Some(mode) = mode {
        session.set_permission(mode);
    }
    let mut run = session.send(prompt).map_err(|e| e.to_string())?;
    let mut out = std::io::stdout().lock();
    let mut code = ExitCode::SUCCESS;
    while let Some(event) = run.next_event() {
        match event {
            EngineEvent::TextDelta(text) => {
                let _ = out.write_all(text.as_bytes());
                let _ = out.flush();
            }
            EngineEvent::ToolStarted(call) => eprintln!("\x1b[2m● {}({})\x1b[0m", call.name, call.summary),
            EngineEvent::Asked(ask) => {
                eprintln!("\x1b[33mdenied: the agent asked for permission (use --mode)\x1b[0m");
                run.answer(ask.id, Decision::Deny).map_err(|e| e.to_string())?;
            }
            EngineEvent::Failed(message) => {
                eprintln!("superfik: {message}");
                code = ExitCode::FAILURE;
            }
            EngineEvent::Cancelled => code = ExitCode::FAILURE,
            _ => {}
        }
    }
    let _ = writeln!(out);
    Ok(code)
}
