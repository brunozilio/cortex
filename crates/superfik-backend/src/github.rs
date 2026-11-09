//! Whose GitHub a command acts as. On this computer's own server it is
//! `gh`'s sign-in; on an organization's, each member's own GitHub user,
//! signed in through the organization's GitHub App (ADR 0010), so pull
//! requests and reviews carry the member's name.

use std::collections::HashMap;
use std::sync::Mutex;

use superfik_core::Principal;

pub trait GitHubTokens: Send + Sync {
    fn token(&self, owner: &Principal) -> Option<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoGitHubTokens;

impl GitHubTokens for NoGitHubTokens {
    fn token(&self, _owner: &Principal) -> Option<String> {
        None
    }
}

pub struct PushingAs {
    pub pusher: std::sync::Arc<dyn superfik_workspace::BranchPusher>,
    pub env: Option<Vec<(String, String)>>,
}

impl superfik_workspace::BranchPusher for PushingAs {
    fn push_current_with(
        &self,
        root: &std::path::Path,
        _env: &[(String, String)],
    ) -> Result<(), superfik_workspace::WorkspaceError> {
        match &self.env {
            Some(env) => self.pusher.push_current_with(root, env),
            None => Err(superfik_workspace::WorkspaceError::Git {
                command: "push".into(),
                message: "sign in to GitHub to push".into(),
            }),
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryGitHubTokens(Mutex<HashMap<Principal, String>>);

impl InMemoryGitHubTokens {
    pub fn set(&self, owner: Principal, token: &str) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(owner, token.to_owned());
    }
}

impl GitHubTokens for InMemoryGitHubTokens {
    fn token(&self, owner: &Principal) -> Option<String> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(owner).cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use superfik_core::{GitHubRepository, OrgId, UserId};
    use superfik_engine::EngineRegistry;
    use superfik_workspace::{GitHubClient, NoGitHub};

    use super::*;
    use crate::LocalServices;

    struct Gh(&'static str);

    impl GitHubClient for Gh {
        fn repository(&self, _: &Path) -> Option<String> {
            None
        }
        fn pull_request(&self, _: &Path) -> Option<superfik_core::PullRequest> {
            None
        }
        fn repositories(&self) -> Option<Vec<GitHubRepository>> {
            Some(vec![GitHubRepository { slug: format!("{}/r", self.0), private: false }])
        }
        fn pull_requests(&self, _: &Path) -> Option<Vec<superfik_core::PullRequest>> {
            None
        }
        fn checkout_pull_request(&self, r: &Path, n: u64) -> Result<(), superfik_workspace::WorkspaceError> {
            NoGitHub.checkout_pull_request(r, n)
        }
        fn create_pull_request(
            &self,
            r: &Path,
            d: &superfik_core::PullRequestDraft,
        ) -> Result<superfik_core::PullRequest, superfik_workspace::WorkspaceError> {
            NoGitHub.create_pull_request(r, d)
        }
        fn pull_request_detail(
            &self,
            r: &Path,
            n: u64,
        ) -> Result<superfik_core::PullRequestDetail, superfik_workspace::WorkspaceError> {
            NoGitHub.pull_request_detail(r, n)
        }
        fn submit_review(
            &self,
            r: &Path,
            n: u64,
            v: superfik_core::ReviewVerdict,
            b: &str,
            c: &[superfik_core::ReviewCommentDraft],
        ) -> Result<(), superfik_workspace::WorkspaceError> {
            NoGitHub.submit_review(r, n, v, b, c)
        }
        fn resolve_thread(&self, r: &Path, t: &str, x: bool) -> Result<(), superfik_workspace::WorkspaceError> {
            NoGitHub.resolve_thread(r, t, x)
        }
        fn clone_repository(&self, s: &str, d: &Path) -> Result<(), superfik_workspace::WorkspaceError> {
            NoGitHub.clone_repository(s, d)
        }
        fn as_user(&self, token: &str) -> Option<Arc<dyn GitHubClient>> {
            Some(Arc::new(Gh(if token == "ana-token" { "ana" } else { "someone" })))
        }
    }

    fn listed(services: &LocalServices) -> Option<String> {
        services.github.repositories().and_then(|repos| repos.first().map(|repo| repo.slug.clone()))
    }

    #[test]
    fn a_members_run_carries_their_github_token_and_the_owners_does_not() {
        use superfik_core::{EngineEvent, EngineKind, ModelOption, PromptRequest, RunId};
        use superfik_protocol::{ClientCommand, Envelope};

        let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
        let engine = superfik_engine::fake::FakeEngine::new(
            EngineKind::Native,
            vec![model.clone()],
            vec![EngineEvent::Cancelled],
        );
        let tokens = Arc::new(InMemoryGitHubTokens::default());
        let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine.clone())));
        services.memory = crate::Memories::plain(Arc::new(crate::InMemoryStore::default()));
        services.memory_tool = false;
        services.github_tokens = tokens.clone();
        services.authorizer = Arc::new(Everyone);
        let (tx, _events) = crossbeam_channel::unbounded();
        let handler = crate::CommandHandler::new(services, crate::Outbox::new(tx, Arc::new(|| {})));
        let ana = Principal::member(UserId::new(), OrgId::new());
        tokens.set(ana.clone(), "ghu_ana");
        let submit = || ClientCommand::SubmitPrompt {
            run: RunId::new(),
            request: PromptRequest::plain("/r".into(), "hi", model.clone()),
        };
        handler.handle(Envelope::new(ana, submit()));
        handler.handle(Envelope::new(Principal::LocalOwner, submit()));
        let github: Vec<Option<String>> = engine.requests().into_iter().map(|r| r.credentials.github).collect();
        assert_eq!(github, [Some("ghu_ana".to_owned()), None]);
    }

    #[derive(Default)]
    struct Pushes(Mutex<Vec<Vec<(String, String)>>>);

    impl superfik_workspace::BranchPusher for Pushes {
        fn push_current_with(
            &self,
            _: &Path,
            env: &[(String, String)],
        ) -> Result<(), superfik_workspace::WorkspaceError> {
            self.0.lock().expect("lock").push(env.to_vec());
            Ok(())
        }
    }

    #[test]
    fn a_member_pushes_as_themselves_or_not_at_all() {
        let pushes = Arc::new(Pushes::default());
        let tokens = Arc::new(InMemoryGitHubTokens::default());
        let mut services = LocalServices::with_engines(EngineRegistry::new());
        services.pusher = pushes.clone();
        services.github_tokens = tokens.clone();
        let (ana, rui) =
            (Principal::member(UserId::new(), OrgId::new()), Principal::member(UserId::new(), OrgId::new()));
        tokens.set(ana.clone(), "ghu_ana");
        assert!(services.acting_as(&ana).pusher.push_current(Path::new("/r")).is_ok());
        assert!(services.acting_as(&rui).pusher.push_current(Path::new("/r")).is_err(), "not as the server");
        let pushed = pushes.0.lock().expect("lock").clone();
        assert_eq!(pushed.len(), 1);
        assert!(pushed[0].contains(&("GH_TOKEN".to_owned(), "ghu_ana".to_owned())));
    }

    struct Everyone;

    impl crate::Authorizer for Everyone {
        fn authorize(&self, _: &Principal, _: &superfik_protocol::ClientCommand) -> Result<(), crate::AuthError> {
            Ok(())
        }
    }

    #[test]
    fn a_member_acts_as_their_own_github_user_and_never_as_the_servers() {
        let tokens = Arc::new(InMemoryGitHubTokens::default());
        let mut services = LocalServices::with_engines(EngineRegistry::new());
        services.github = Arc::new(Gh("server"));
        services.github_tokens = tokens.clone();
        let (ana, rui) =
            (Principal::member(UserId::new(), OrgId::new()), Principal::member(UserId::new(), OrgId::new()));
        tokens.set(ana.clone(), "ana-token");
        assert_eq!(listed(&services.acting_as(&ana)).as_deref(), Some("ana/r"));
        assert_eq!(listed(&services.acting_as(&rui)), None, "not signed in: no GitHub, not the server's");
        assert_eq!(listed(&services.acting_as(&Principal::LocalOwner)).as_deref(), Some("server/r"));
    }
}
