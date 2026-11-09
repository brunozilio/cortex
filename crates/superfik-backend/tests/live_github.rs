//! The real `gh` through `LocalBackend`: lists the signed-in account's
//! repositories and locates this checkout without cloning. Needs `gh auth
//! login`; run with
//! `cargo test -p superfik-backend --test live_github -- --ignored --nocapture`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_backend::{Backend, LocalBackend, LocalServices, Wake};
use superfik_protocol::{ClientCommand, Envelope, ServerEvent};

fn backend() -> (LocalBackend, Receiver<()>) {
    let (woke_tx, woke) = crossbeam_channel::unbounded();
    let wake: Wake = Arc::new(move || {
        let _ = woke_tx.send(());
    });
    (LocalBackend::new(LocalServices::detect(), wake).expect("backend"), woke)
}

fn next(backend: &LocalBackend, woke: &Receiver<()>, matches: impl Fn(&ServerEvent) -> bool) -> ServerEvent {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let _ = woke.recv_timeout(Duration::from_millis(50));
        while let Some(event) = backend.try_recv() {
            if matches(&event) {
                return event;
            }
        }
    }
    panic!("no matching event within 30s");
}

#[test]
#[ignore = "needs a signed-in gh"]
fn lists_the_account_and_locates_this_checkout() {
    let (backend, woke) = backend();
    backend.send(Envelope::local(ClientCommand::ListGitHubRepositories)).expect("send");
    let listing = next(&backend, &woke, |e| matches!(e, ServerEvent::GitHubRepositories { .. }));
    let ServerEvent::GitHubRepositories { signed_in, repositories } = listing else { unreachable!() };
    assert!(signed_in, "gh is signed out");
    assert!(!repositories.is_empty());
    for repo in &repositories {
        eprintln!("{}{}", repo.slug, if repo.private { " (private)" } else { "" });
    }

    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace");
    let git = superfik_workspace::GitCli::system();
    let slug = superfik_workspace::GitInspector::inspect(&git, &here).expect("this checkout").slug;
    let clone_root = tempfile::tempdir().expect("tempdir");
    backend
        .send(Envelope::local(ClientCommand::LocateRepository {
            slug: slug.clone(),
            known: vec![PathBuf::from("/nowhere"), here.clone()],
            clone_root: clone_root.path().to_path_buf(),
        }))
        .expect("send");
    let located =
        next(&backend, &woke, |e| matches!(e, ServerEvent::RepositoryLocated { .. } | ServerEvent::Error { .. }));
    eprintln!("{located:?}");
    assert!(matches!(&located, ServerEvent::RepositoryLocated { slug: s, path } if *s == slug && *path == here));
    assert!(std::fs::read_dir(clone_root.path()).expect("read").next().is_none(), "nothing was cloned");
}

#[test]
#[ignore = "needs a signed-in gh and SUPERFIK_LIVE_PR_REPO"]
fn lists_open_pull_requests_of_a_checkout() {
    let Some(path) = std::env::var_os("SUPERFIK_LIVE_PR_REPO").map(PathBuf::from) else { return };
    let (backend, woke) = backend();
    backend.send(Envelope::local(ClientCommand::ListPullRequests { path })).expect("send");
    let ServerEvent::PullRequests(prs) = next(&backend, &woke, |e| matches!(e, ServerEvent::PullRequests(_))) else {
        unreachable!()
    };
    assert!(!prs.is_empty(), "no open pull requests");
    for pr in &prs {
        eprintln!(
            "#{} {} by {} ({} ← {}) {:?} {}",
            pr.number,
            pr.title,
            pr.author,
            pr.base_branch,
            pr.head_branch,
            pr.review_decision,
            pr.size_caption()
        );
    }
    assert!(prs.iter().all(|pr| pr.number > 0 && !pr.head_branch.is_empty()));
}
