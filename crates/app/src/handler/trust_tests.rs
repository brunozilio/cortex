use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agent_engine::fake::FakeEngine;
use domain::trust::{Trust, TrustReport};
use domain::{EngineEvent, ModelOption, PromptRequest, RunId};

use super::*;
use crate::api::{Envelope, RequestId};

type Sent = crossbeam_channel::Receiver<ServerEvent>;

struct Stage {
    handler: CommandHandler,
    sent: Sent,
    engine: FakeEngine,
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
}

fn stage() -> Stage {
    let done = vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }];
    let engine = FakeEngine::new(vec![ModelOption::new("m", "M")], done);
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    let (home, repo) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("repo"));
    services.homes = Arc::new(crate::OwnerHome::at(home.path()));
    let hooks = repo.path().join(brand::DIR).join("hooks");
    std::fs::create_dir_all(&hooks).expect("hooks");
    std::fs::write(hooks.join("guard.js"), "export const event = \"Stop\"; export default () => ({});").expect("hook");
    let (events, sent) = crossbeam_channel::unbounded();
    let handler = CommandHandler::new(services, Outbox::new(events, Arc::new(|| {})));
    Stage { handler, sent, engine, home, repo }
}

impl Stage {
    fn answer(&self, command: ClientCommand) -> Reply {
        self.handler.handle(Envelope::local(RequestId(1), command));
        loop {
            match self.sent.recv_timeout(Duration::from_secs(5)) {
                Ok(ServerEvent::Reply { reply, .. }) => return reply,
                Ok(_) => {}
                Err(error) => panic!("no answer: {error}"),
            }
        }
    }

    fn report(&self, command: ClientCommand) -> TrustReport {
        match self.answer(command) {
            Reply::Trust(report) => report,
            other => panic!("expected the trust state, got {other:?}"),
        }
    }

    fn state(&self, path: &Path) -> TrustReport {
        self.report(ClientCommand::TrustState { path: path.to_path_buf() })
    }

    fn trust_a_run_gets(&self, claimed: Trust) -> Trust {
        let mut request = PromptRequest::plain(self.repo.path().into(), "hi", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { trust: claimed, ..Default::default() });
        let before = self.engine.requests().len();
        assert_eq!(self.answer(ClientCommand::SubmitPrompt { run: RunId::new(), request }), Reply::Done);
        let started = self.engine.requests().get(before).cloned().expect("the run started");
        started.host.extensions.map(|extensions| extensions.trust).unwrap_or_default()
    }
}

#[test]
fn a_client_cannot_mark_its_own_request_trusted() {
    let stage = stage();

    assert_eq!(stage.trust_a_run_gets(Trust::Trusted), Trust::Untrusted);
}

#[test]
fn trusting_a_folder_is_kept_reaches_the_next_run_and_can_be_taken_back() {
    let stage = stage();
    let repo = stage.repo.path().to_path_buf();

    let before = stage.state(&repo);
    assert_eq!(before.trust, Trust::Untrusted);
    assert!(before.asks(), "it brings a hook script: {:?}", before.brings);
    assert_eq!(before.root, agent_engine::trust::root_of(&repo, Some(stage.home.path())));

    let trusted = stage.report(ClientCommand::Trust { path: repo.clone(), trusted: true });
    assert_eq!(trusted.trust, Trust::Trusted);
    assert_eq!(stage.state(&repo).trust, Trust::Trusted);
    assert_eq!(stage.trust_a_run_gets(Trust::Untrusted), Trust::Trusted, "the store decides, not the request");

    let untrusted = stage.report(ClientCommand::Trust { path: repo.clone(), trusted: false });
    assert_eq!(untrusted.trust, Trust::Untrusted);
    assert_eq!(stage.trust_a_run_gets(Trust::Trusted), Trust::Untrusted);
}

#[test]
fn trust_for_this_process_reaches_runs_and_is_never_stored() {
    let stage = stage();
    let repo = stage.repo.path().to_path_buf();

    let once = stage.report(ClientCommand::TrustForThisProcess { path: repo.clone() });

    assert_eq!(once.trust, Trust::Trusted);
    assert_eq!(stage.trust_a_run_gets(Trust::Untrusted), Trust::Trusted);
    assert!(stage.handler.services.extensions.trusts.stored().listed().is_empty(), "nothing was written");
}

#[test]
fn the_home_folder_is_refused_with_the_reason() {
    let stage = stage();
    let home = stage.home.path().to_path_buf();

    let refused = stage.answer(ClientCommand::Trust { path: home.clone(), trusted: true });
    let Reply::Error { message } = refused else { panic!("expected a refusal, got {refused:?}") };
    assert!(message.contains("home folder cannot be trusted"), "{message}");
    let once = stage.answer(ClientCommand::TrustForThisProcess { path: home.clone() });
    assert!(matches!(once, Reply::Error { .. }), "{once:?}");
    let state = stage.state(&home);
    assert_eq!(state.trust, Trust::Untrusted);
    assert!(!state.asks(), "nothing to ask about at home");
}
