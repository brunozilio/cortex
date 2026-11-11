use std::path::PathBuf;
use std::process::ExitCode;

use sdk::{Decision, EngineEvent, PermissionMode, Sdk};

use crate::print_output::{Format, Output};

#[derive(Default)]
pub struct Wanted<'a> {
    pub model: Option<&'a str>,
    pub mode: Option<PermissionMode>,
    pub verification: Option<String>,
    pub add_dirs: Vec<PathBuf>,
    pub headless: sdk::Headless,
    pub format: Format,
    pub resume: Option<sdk::SessionId>,
    pub fork: bool,
}

pub fn print_all(sdk: &Sdk, cwd: PathBuf, wanted: Wanted<'_>, prompts: &[String]) -> Result<ExitCode, String> {
    let Wanted { model: wanted, mode, verification, add_dirs, headless, format, resume, fork } = wanted;
    let models = sdk.models().map_err(|e| e.to_string())?;
    let remembered = sdk.settings().ok().and_then(|s| s["model"].as_str().map(str::to_owned));
    let model = domain::model::choose_model(&models, wanted.or(remembered.as_deref())).ok_or(concat!(
        "no model is available: run `",
        brand::command!(),
        " login` first"
    ))?;
    if let Some(wanted) = wanted
        && !model.is_named(wanted)
    {
        return Err(format!(
            "no model named {wanted}: `{brand_command} models` lists them",
            brand_command = brand::COMMAND
        ));
    }
    let mut session = sdk.session(cwd, model);
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
                EngineEvent::Asked(sdk::AgentAsk { id, kind: sdk::AskKind::Plan { plan } }) => {
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

    use agent_engine::fake::FakeEngine;
    use backend::{InMemorySecretStore, LocalServices};
    use sdk::{Criteria, Model, Verdict};

    use super::*;

    fn printed(verdict: Verdict) -> (ExitCode, FakeEngine) {
        let data = tempfile::tempdir().expect("data");
        let engine = FakeEngine::new(
            vec![Model::new("scripted", "Scripted")],
            vec![EngineEvent::Verified(verdict), EngineEvent::Completed { duration_ms: 1, cost_usd: None }],
        );
        let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
        services.data = None;
        services.off.insert("memory_search".into());
        services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
        let sdk = Sdk::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
        let wanted = Wanted { verification: Some("tests pass".into()), ..Wanted::default() };
        (print_all(&sdk, data.path().into(), wanted, &["fix it".into()]).expect("printed"), engine)
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
