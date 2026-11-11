//! `superfik -p`: one prompt, the reply on stdout, the agent's steps on
//! stderr. Nobody is there to answer, so whatever the agent asks is
//! denied (pick `--mode bypass` or `edits` to let it act).

use std::path::PathBuf;
use std::process::ExitCode;

use superfik_sdk::{Decision, EngineEvent, PermissionMode, Superfik};

use crate::print_output::{Format, Output};

/// What the command line asked for besides the prompt.
#[derive(Default)]
pub struct Wanted<'a> {
    pub model: Option<&'a str>,
    pub mode: Option<PermissionMode>,
    pub verification: Option<String>,
    /// Folders read like the project (`--add-dir`).
    pub add_dirs: Vec<PathBuf>,
    /// `--max-turns`, `--allowedTools`, `--disallowedTools`,
    /// `--append-system-prompt`.
    pub headless: superfik_sdk::Headless,
    pub format: Format,
    /// The conversation to go on with (`--resume <id>`, `-c`).
    pub resume: Option<superfik_sdk::SessionId>,
    /// Go on in a copy of it (`--fork-session`).
    pub fork: bool,
}

/// Each prompt in turn, in one conversation (`--input-format stream-json`
/// gives several).
pub fn print_all(
    superfik: &Superfik,
    cwd: PathBuf,
    wanted: Wanted<'_>,
    prompts: &[String],
) -> Result<ExitCode, String> {
    let Wanted { model: wanted, mode, verification, add_dirs, headless, format, resume, fork } = wanted;
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
    if let Some(id) = resume {
        session = session.resume(id);
    }
    if let Some(mode) = mode {
        session.set_permission(mode);
    }
    session.set_verification(verification);
    session.set_headless(headless);
    if fork {
        session.fork_next();
    }
    for dir in add_dirs {
        session.add_dir(dir);
    }
    let mut output = Output::new(format, std::io::stdout().lock());
    let mut failed_verification = false;
    for prompt in prompts {
        let mut run = session.send(prompt).map_err(|e| e.to_string())?;
        while let Some(event) = run.next_event() {
            output.event(&event);
            match event {
                EngineEvent::Asked(superfik_sdk::AgentAsk { id, kind: superfik_sdk::AskKind::Plan { plan } }) => {
                    output.push_text(&plan);
                    output.note("plan not carried out: run again with --mode edits to implement it");
                    run.answer(id, Decision::Deny).map_err(|e| e.to_string())?;
                }
                EngineEvent::Asked(ask) => {
                    output.note("denied: the agent asked for permission (use --mode or --allowedTools)");
                    run.answer(ask.id, Decision::Deny).map_err(|e| e.to_string())?;
                }
                EngineEvent::Verified(verdict) => {
                    output.note(&crate::verdict::line(&verdict));
                    failed_verification |= !verdict.passed;
                }
                _ => {}
            }
        }
    }
    let failed = output.finish(failed_verification);
    Ok(if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS })
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
        let wanted = Wanted { verification: Some("tests pass".into()), ..Wanted::default() };
        (print_all(&superfik, data.path().into(), wanted, &["fix it".into()]).expect("printed"), engine)
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
