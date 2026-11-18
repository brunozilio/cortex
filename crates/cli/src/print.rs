use std::path::PathBuf;
use std::process::ExitCode;

use app::{Client, Decision, EngineEvent, PermissionMode};

use crate::print_output::{Format, Output};

#[derive(Default)]
pub struct Wanted<'a> {
    pub model: Option<&'a str>,
    pub mode: Option<PermissionMode>,
    pub verification: Option<String>,
    pub add_dirs: Vec<PathBuf>,
    pub headless: app::Headless,
    pub format: Format,
    pub resume: Option<app::SessionId>,
    pub fork: bool,
    pub trust: bool,
    pub asked: Option<Answer<'a>>,
}

pub type Answer<'a> = &'a (dyn Fn(&app::AgentAsk) -> Option<Decision> + 'a);

pub fn resumed(
    picked: Option<&str>,
    continuing: bool,
    last: impl FnOnce() -> Result<Option<app::SessionId>, String>,
) -> Result<Option<app::SessionId>, String> {
    match picked {
        Some("") => Err("--resume needs a session id with --print".to_owned()),
        Some(id) => Ok(Some(app::SessionId(id.to_owned()))),
        None if continuing => last(),
        None => Ok(None),
    }
}

pub fn print_all(
    client: &Client,
    cwd: PathBuf,
    wanted: Wanted<'_>,
    prompts: impl IntoIterator<Item = Result<String, String>>,
) -> Result<ExitCode, String> {
    let mut output = Output::new(wanted.format, std::io::stdout().lock());
    match answer_all(client, cwd, wanted, prompts, &mut output) {
        Ok(failed) => Ok(if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }),
        Err(message) => {
            output.abandon(&message);
            Err(message)
        }
    }
}

fn answer_all(
    client: &Client,
    cwd: PathBuf,
    wanted: Wanted<'_>,
    prompts: impl IntoIterator<Item = Result<String, String>>,
    output: &mut Output<impl std::io::Write>,
) -> Result<bool, String> {
    let Wanted { model: wanted, mode, verification, add_dirs, headless, format: _, resume, fork, trust, asked } =
        wanted;
    let model = chosen_model(client, wanted)?;
    settle_trust(client, &cwd, trust, &mut std::io::stderr().lock())?;
    warn_of_what_is_not_kept(client, &mut std::io::stderr().lock());
    let mut session = client.session(cwd, model);
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
    let mut failed_verification = false;
    for prompt in prompts {
        let mut run = session.send(&prompt?).map_err(|e| e.to_string())?;
        while let Some(event) = run.next_event() {
            output.event(&event);
            match event {
                EngineEvent::Asked(ask @ app::AgentAsk { kind: app::AskKind::Plan { .. }, .. })
                    if let Some(decision) = asked.and_then(|answer| answer(&ask)) =>
                {
                    run.answer(ask.id, decision).map_err(|e| e.to_string())?;
                }
                EngineEvent::Asked(app::AgentAsk { id, kind: app::AskKind::Plan { plan } }) => {
                    output.push_text(&plan);
                    output.note("plan not carried out: run again with --mode edits to implement it");
                    run.answer(id, Decision::Deny).map_err(|e| e.to_string())?;
                }
                EngineEvent::Asked(ask) => {
                    let decision = asked.and_then(|answer| answer(&ask)).unwrap_or_else(|| {
                        output.note("denied: the agent asked for permission (use --mode or --allowedTools)");
                        Decision::Deny
                    });
                    run.answer(ask.id, decision).map_err(|e| e.to_string())?;
                }
                EngineEvent::Verified(verdict) => {
                    output.note(&domain::conversation::verdict::line(&verdict));
                    failed_verification |= !verdict.passed;
                }
                _ => {}
            }
        }
    }
    Ok(output.finish(failed_verification))
}

pub fn chosen_model(client: &Client, wanted: Option<&str>) -> Result<app::Model, String> {
    let models = client.models().map_err(|e| e.to_string())?;
    let remembered = client.settings().ok().and_then(|s| s["model"].as_str().map(str::to_owned));
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
    Ok(model)
}

fn settle_trust(
    client: &Client,
    cwd: &std::path::Path,
    trust: bool,
    errors: &mut dyn std::io::Write,
) -> Result<(), String> {
    let trusted_once = trust.then(|| client.trust_for_this_process(cwd));
    let report = match trusted_once {
        Some(Ok(report)) => report,
        Some(Err(refused)) => {
            let _ = writeln!(errors, "{}: --trust ignored: {refused}", brand::COMMAND);
            client.trust_state(cwd).map_err(|error| error.to_string())?
        }
        None => client.trust_state(cwd).map_err(|error| error.to_string())?,
    };
    if let Some(skipped) = report.skipped_line() {
        let _ = writeln!(errors, "{}: {skipped}", brand::COMMAND);
    }
    let replaced = client.permissions(cwd).map(|info| info.notices).unwrap_or_default();
    for notice in replaced.iter().filter(|notice| notice.contains(domain::trust::LEGACY_OPT_IN)) {
        let _ = writeln!(errors, "{}: {notice}", brand::COMMAND);
    }
    Ok(())
}

fn warn_of_what_is_not_kept(client: &Client, errors: &mut dyn std::io::Write) {
    let notices = client.notices(false).map(|notices| notices.for_user).unwrap_or_default();
    for notice in notices {
        let _ = writeln!(errors, "{}: {notice}", brand::COMMAND);
    }
}

#[cfg(test)]
#[path = "print_trust_tests.rs"]
mod trust_tests;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_engine::fake::FakeEngine;
    use app::{Criteria, Model, Verdict};
    use app::{InMemorySecretStore, LocalServices};

    use super::*;

    fn printed(verdict: Verdict) -> (ExitCode, FakeEngine) {
        let data = tempfile::tempdir().expect("data");
        let engine = FakeEngine::new(
            vec![Model::new("scripted", "Scripted")],
            vec![EngineEvent::Verified(verdict), EngineEvent::Completed { duration_ms: 1, cost_usd: None }],
        );
        let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
        services.off.insert(domain::features::MEMORY_SEARCH.into());
        services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
        let client = Client::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
        let wanted = Wanted { verification: Some("tests pass".into()), ..Wanted::default() };
        (print_all(&client, data.path().into(), wanted, [Ok("fix it".into())]).expect("printed"), engine)
    }

    fn scripted(data: &std::path::Path) -> (Client, FakeEngine) {
        let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
        let engine = FakeEngine::new(vec![Model::new("scripted", "Scripted")], done);
        let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
        services.off.insert(domain::features::MEMORY_SEARCH.into());
        services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
        (Client::builder().data_dir(data).services(services).build().expect(brand::COMMAND), engine)
    }

    #[test]
    fn a_database_that_cannot_be_opened_is_said_once_on_the_error_stream() {
        let data = tempfile::tempdir().expect("data");
        std::fs::create_dir(data.path().join("app.db")).expect("a folder where the database should be");
        let (client, _) = scripted(data.path());

        let mut errors = Vec::new();
        warn_of_what_is_not_kept(&client, &mut errors);
        let said = String::from_utf8(errors).expect("text");
        assert!(said.starts_with(&format!("{}: The session list and memory could not be opened (", brand::COMMAND)));
        assert!(said.trim_end().ends_with("they will not be kept after this session."), "{said}");
        assert_eq!(said.lines().count(), 1, "{said}");

        let mut again = Vec::new();
        warn_of_what_is_not_kept(&client, &mut again);
        assert!(again.is_empty(), "said once");
        let (kept, _) = scripted(tempfile::tempdir().expect("data").path());
        warn_of_what_is_not_kept(&kept, &mut again);
        assert!(again.is_empty(), "nothing to say when the database opens");
    }

    #[test]
    fn each_prompt_is_answered_before_the_next_one_is_read() {
        let data = tempfile::tempdir().expect("data");
        let (client, engine) = scripted(data.path());
        let answered_when_read = std::cell::RefCell::new(Vec::new());
        let prompts = ["first", "second", "third"].into_iter().map(|prompt| {
            answered_when_read.borrow_mut().push(engine.requests().len());
            Ok(prompt.to_owned())
        });
        let code = print_all(&client, data.path().into(), Wanted::default(), prompts).expect("printed");
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(*answered_when_read.borrow(), [0, 1, 2]);
    }

    #[test]
    fn an_input_error_ends_the_command_after_the_prompts_before_it() {
        let data = tempfile::tempdir().expect("data");
        let (client, engine) = scripted(data.path());
        let prompts = [Ok("first".to_owned()), Err("input line 2: could not be read".to_owned()), Ok("x".to_owned())];
        let ended = print_all(&client, data.path().into(), Wanted::default(), prompts);
        assert_eq!(ended, Err("input line 2: could not be read".to_owned()));
        assert_eq!(engine.requests().len(), 1, "the first was answered, the one after the error never sent");
    }

    #[test]
    fn resume_without_an_id_is_an_error_when_printing() {
        let none = || Ok(None);
        assert_eq!(resumed(Some(""), false, none), Err("--resume needs a session id with --print".to_owned()));
        assert_eq!(resumed(Some("s-1"), false, none), Ok(Some(app::SessionId("s-1".into()))));
        assert_eq!(
            resumed(None, true, || Ok(Some(app::SessionId("last".into())))),
            Ok(Some(app::SessionId("last".into())))
        );
        assert_eq!(resumed(None, false, || Err("never read".to_owned())), Ok(None));
    }

    #[test]
    fn a_failed_verification_fails_the_command() {
        let (code, engine) = printed(Verdict::parse(r#"{"passed":false}"#, 3).expect("verdict"));
        assert_eq!(code, ExitCode::FAILURE);
        assert_eq!(engine.requests()[0].policy.verification, Some(Criteria::user("tests pass")));
    }

    #[test]
    fn a_passed_verification_succeeds() {
        let (code, _) = printed(Verdict::parse(r#"{"passed":true}"#, 1).expect("verdict"));
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
