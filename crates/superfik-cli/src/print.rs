//! `superfik -p`: one prompt, the reply on stdout, the agent's steps on
//! stderr. Nobody is there to answer, so whatever the agent asks is
//! denied (pick `--mode bypass` or `edits` to let it act).

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use superfik_sdk::{Decision, EngineEvent, PermissionMode, Superfik};

/// What the command line asked for besides the prompt.
pub struct Wanted<'a> {
    pub model: Option<&'a str>,
    pub mode: Option<PermissionMode>,
    pub verification: Option<String>,
}

pub fn print(superfik: &Superfik, cwd: PathBuf, wanted: Wanted<'_>, prompt: &str) -> Result<ExitCode, String> {
    let Wanted { model: wanted, mode, verification } = wanted;
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
    session.set_verification(verification);
    let mut run = session.send(prompt).map_err(|e| e.to_string())?;
    let mut out = std::io::stdout().lock();
    let mut code = ExitCode::SUCCESS;
    let mut failed_verification = false;
    while let Some(event) = run.next_event() {
        match event {
            EngineEvent::TextDelta(text) => {
                let _ = out.write_all(text.as_bytes());
                let _ = out.flush();
            }
            EngineEvent::ToolStarted(call) => eprintln!("\x1b[2m● {}({})\x1b[0m", call.name, call.summary),
            EngineEvent::Asked(superfik_sdk::AgentAsk { id, kind: superfik_sdk::AskKind::Plan { plan } }) => {
                let _ = writeln!(out, "{plan}");
                eprintln!("\x1b[2mplan not carried out: run again with --mode edits to implement it\x1b[0m");
                run.answer(id, Decision::Deny).map_err(|e| e.to_string())?;
            }
            EngineEvent::Asked(ask) => {
                eprintln!("\x1b[33mdenied: the agent asked for permission (use --mode)\x1b[0m");
                run.answer(ask.id, Decision::Deny).map_err(|e| e.to_string())?;
            }
            EngineEvent::Failed(message) => {
                eprintln!("superfik: {message}");
                code = ExitCode::FAILURE;
            }
            EngineEvent::Cancelled => code = ExitCode::FAILURE,
            EngineEvent::Verified(verdict) => {
                eprintln!("{}", crate::verdict::line(&verdict));
                failed_verification = !verdict.passed;
            }
            _ => {}
        }
    }
    if failed_verification {
        code = ExitCode::FAILURE;
    }
    let _ = writeln!(out);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use superfik_backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
    use superfik_native::fake::FakeEngine;
    use superfik_sdk::{Criteria, Model, Verdict};

    use super::*;

    fn printed(verdict: Verdict) -> (ExitCode, FakeEngine) {
        let data = tempfile::tempdir().expect("data");
        let engine = FakeEngine::new(
            vec![Model::new("scripted", "Scripted")],
            vec![EngineEvent::Verified(verdict), EngineEvent::Completed { duration_ms: 1, cost_usd: None }],
        );
        let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
        services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
        services.memory_tool = false;
        services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
        let superfik = Superfik::builder().data_dir(data.path()).services(services).build().expect("superfik");
        let wanted = Wanted { model: None, mode: None, verification: Some("tests pass".into()) };
        (print(&superfik, data.path().into(), wanted, "fix it").expect("printed"), engine)
    }

    #[test]
    fn a_failed_verification_fails_the_command() {
        let (code, engine) = printed(Verdict::parse(r#"{"passed":false}"#, 3).expect("verdict"));
        assert_eq!(code, ExitCode::FAILURE);
        assert_eq!(engine.requests()[0].verification, Some(Criteria::user("tests pass")));
    }

    #[test]
    fn a_passed_verification_succeeds() {
        let (code, _) = printed(Verdict::parse(r#"{"passed":true}"#, 1).expect("verdict"));
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
