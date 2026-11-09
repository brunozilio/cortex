//! What an organization's server checks before a member's command reaches
//! its workers. The server runs everything (ADR 0010), but a member points
//! it only at their own checkouts: every path a command names must lie in
//! them. Every prompt follows the organization's policy and budget, and a
//! repository is located in the checkout the server keeps for the member.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use superfik_backend::{AuthError, BackendError, forbidden};
use superfik_core::access::{Ability, allows};
use superfik_core::policy::{Violation, month_of};
use superfik_core::project::ProjectRef;
use superfik_core::{EngineEvent, Member, Principal, PromptRequest, RunId};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use super::{Connection, Hosted, Inner};

const PREPARING_AT_MOST: usize = 4;

impl Connection {
    pub(super) fn admitted(&self, hosted: &Hosted, command: &ClientCommand) -> Result<(), AuthError> {
        match (command, &self.principal) {
            (ClientCommand::SignInToGitHub | ClientCommand::SignOutOfGitHub, Principal::Member(member)) => hosted
                .organization
                .directory
                .role(member)
                .is_some_and(|role| allows(role, None, Ability::Run))
                .then_some(())
                .ok_or_else(|| forbidden(&self.principal, command.name())),
            (
                ClientCommand::Viewing { .. } | ClientCommand::Upload { .. } | ClientCommand::LocateRepository { .. },
                _,
            ) => self.server.authorizer.authorize(&self.principal, command),
            _ => Ok(()),
        }
    }

    pub(super) fn member_command(&self, hosted: &Hosted, command: &ClientCommand) -> Option<Result<(), BackendError>> {
        let Principal::Member(member) = self.principal else { return None };
        match command {
            ClientCommand::LocateRepository { slug, .. } => return Some(self.locate(hosted, member, slug)),
            ClientCommand::SignInToGitHub => return Some(self.sign_in_to_github(hosted)),
            ClientCommand::SignOutOfGitHub => {
                hosted.github.tokens.forget(&self.principal);
                let event = ServerEvent::GitHubSignedIn { login: None };
                self.server.links.deliver(Addressed { to: Recipient::Principal(self.principal.clone()), event });
                return Some(Ok(()));
            }
            _ => {}
        }
        let home = hosted.checkouts.home(member.user);
        if let ClientCommand::Upload { from, name, data } = command {
            let event = match crate::uploads::keep(&home, name, data) {
                Ok(path) => ServerEvent::Uploaded { from: from.clone(), path },
                Err(why) => ServerEvent::error(format!("could not send {name}: {why}")),
            };
            self.server.links.deliver(Addressed { to: Recipient::Principal(self.principal.clone()), event });
            return Some(Ok(()));
        }
        if let Some(outside) = command.paths().into_iter().find(|path| !within(path, &home)) {
            let why = format!("{} is outside your checkouts on this server", outside.display());
            return Some(self.refuse(command, why));
        }
        if let ClientCommand::EditRoutines { edits, .. } = command {
            let policy = hosted.governance.policy();
            let violation = edits.iter().find_map(|edit| edit.violation(&policy))?;
            return Some(
                self.refuse(command, format!("the organization's policy does not allow this routine: {violation:?}")),
            );
        }
        if let ClientCommand::SetPermissionMode { mode, .. } = command {
            let violation = Violation::Mode(*mode);
            return (!hosted.governance.policy().allows_mode(*mode)).then(|| {
                self.refuse(command, format!("the organization's policy does not allow this run: {violation:?}"))
            });
        }
        let request = match command {
            ClientCommand::SubmitPrompt { request, .. } => request.clone(),
            ClientCommand::OptimizeMemory { path, model, .. } => {
                PromptRequest::plain(path.clone(), String::new(), model.clone())
            }
            _ => return None,
        };
        let why = allowed(hosted, member, &request).err()?;
        Some(self.refuse(command, why))
    }

    fn refuse(&self, command: &ClientCommand, why: String) -> Result<(), BackendError> {
        let event = match run_of(command) {
            Some(run) => ServerEvent::Run { run, event: EngineEvent::Failed(why) },
            None => ServerEvent::error(why),
        };
        self.server.links.deliver(Addressed { to: Recipient::Principal(self.principal.clone()), event });
        Ok(())
    }

    fn sign_in_to_github(&self, hosted: &Hosted) -> Result<(), BackendError> {
        let to = Recipient::Principal(self.principal.clone());
        let deliver = |event| self.server.links.deliver(Addressed { to: to.clone(), event });
        let Some(app) = hosted.github.app.clone() else {
            deliver(ServerEvent::error("this server has no GitHub App to sign in with".to_owned()));
            return Ok(());
        };
        let code = match app.begin() {
            Ok(code) => code,
            Err(why) => {
                deliver(ServerEvent::error(format!("could not start signing in to GitHub: {why}")));
                return Ok(());
            }
        };
        deliver(ServerEvent::GitHubCode { code: code.user_code.clone(), url: code.url.clone() });
        let (server, tokens, owner) =
            (Arc::clone(&self.server), Arc::clone(&hosted.github.tokens), self.principal.clone());
        std::thread::Builder::new()
            .name("superfik-github-sign-in".to_owned())
            .spawn(move || {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                let signed_in = app
                    .finish(&code, now)
                    .and_then(|granted| tokens.keep(&owner, &granted).map(|()| granted))
                    .and_then(|granted| app.login(&granted.token));
                let event = match signed_in {
                    Ok(login) => ServerEvent::GitHubSignedIn { login: Some(login) },
                    Err(why) => ServerEvent::error(format!("signing in to GitHub did not finish: {why}")),
                };
                server.links.deliver(Addressed { to: Recipient::Principal(owner), event });
            })
            .map(|_| ())
            .map_err(BackendError::Worker)
    }

    fn locate(&self, hosted: &Hosted, member: Member, slug: &str) -> Result<(), BackendError> {
        let to = Recipient::Principal(self.principal.clone());
        let preparing: &AtomicUsize = &self.server.preparing;
        if preparing.fetch_add(1, Ordering::SeqCst) >= PREPARING_AT_MOST {
            preparing.fetch_sub(1, Ordering::SeqCst);
            let event = ServerEvent::error("the server is busy; try again".to_owned());
            self.server.links.deliver(Addressed { to, event });
            return Ok(());
        }
        let (server, hosted, slug) = (Arc::clone(&self.server), hosted.clone(), slug.to_owned());
        let started = std::thread::Builder::new().name("superfik-server-locate".to_owned()).spawn(move || {
            let _done = Preparing(Arc::clone(&server));
            located(&server, &hosted, member, slug, to);
        });
        if started.is_err() {
            preparing.fetch_sub(1, Ordering::SeqCst);
        }
        started.map(|_| ()).map_err(BackendError::Worker)
    }
}

struct Preparing(Arc<Inner>);

impl Drop for Preparing {
    fn drop(&mut self) {
        self.0.preparing.fetch_sub(1, Ordering::SeqCst);
    }
}

fn located(server: &Inner, hosted: &Hosted, member: Member, slug: String, to: Recipient) {
    let project = ProjectRef::Remote(format!("github.com/{slug}"));
    let token = superfik_backend::GitHubTokens::token(hosted.github.tokens.as_ref(), &Principal::Member(member));
    let event = match hosted.checkouts.prepare(&project, member.user, token.as_deref()) {
        Ok(path) => ServerEvent::RepositoryLocated { slug, path },
        Err(why) => ServerEvent::error(format!("could not prepare {slug} on the server: {why}")),
    };
    server.links.deliver(Addressed { to, event });
}

fn allowed(hosted: &Hosted, member: Member, request: &PromptRequest) -> Result<(), String> {
    let policy = hosted.governance.policy();
    let month = month_of(i64::try_from(hosted.organization.now()).unwrap_or(i64::MAX));
    let spent = hosted.governance.usage(&month).get(&member.user).map_or(0.0, |(usd, _)| *usd);
    policy
        .check(request, spent)
        .map_err(|violation| format!("the organization's policy does not allow this run: {violation:?}"))
}

fn within(path: &Path, home: &Path) -> bool {
    path.is_absolute()
        && !path.components().any(|part| part == Component::ParentDir)
        && path.starts_with(home)
        && resolved(path).zip(resolved(home)).is_some_and(|(path, home)| path.starts_with(home))
}

fn resolved(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return Some(rest.iter().rev().fold(real, |path, part| path.join(part)));
        }
        if existing.symlink_metadata().is_ok() {
            return None;
        }
        rest.push(existing.file_name()?.to_owned());
        existing = existing.parent()?;
    }
}

fn run_of(command: &ClientCommand) -> Option<RunId> {
    match command {
        ClientCommand::SubmitPrompt { run, .. } | ClientCommand::OptimizeMemory { run, .. } => Some(*run),
        _ => None,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_within_home_only_below_it_and_without_climbing() {
        let home = Path::new("/srv/checkouts/u1");
        assert!(within(Path::new("/srv/checkouts/u1/github.com/a/b"), home));
        assert!(within(home, home));
        assert!(!within(Path::new("/srv/checkouts/u10"), home), "a sibling sharing the prefix");
        assert!(!within(Path::new("/srv/checkouts/u1/../u2"), home));
        assert!(!within(Path::new("u1/x"), home), "relative");
        assert!(!within(Path::new("/etc"), home));
    }

    #[test]
    fn a_link_in_home_pointing_out_of_it_is_not_within() {
        let (home, outside) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("outside"));
        std::fs::create_dir(home.path().join("rocket")).expect("repo");
        std::os::unix::fs::symlink(outside.path(), home.path().join("escape")).expect("link");
        std::os::unix::fs::symlink("/nowhere/at/all", home.path().join("dangling")).expect("link");
        assert!(!within(&home.path().join("dangling/new.txt"), home.path()), "a link to nowhere");
        assert!(within(&home.path().join("rocket"), home.path()));
        assert!(within(&home.path().join("rocket/not-yet/new.txt"), home.path()), "not there yet, still inside");
        assert!(!within(&home.path().join("escape"), home.path()));
        assert!(!within(&home.path().join("escape/secrets.txt"), home.path()));
    }
}
