//! A hosted server: its organization's stores in one data folder, its
//! workers and its routes.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use superfik_backend::{LocalServices, RoleAuthorizer, SealedSecretStore};

use crate::audit::{AuditLog, FileAudit};
use crate::checkouts::GitCheckouts;
use crate::governance::FileGovernance;
use crate::library::FileLibrary;
use crate::project_sessions::FileProjectSessions;
use crate::shared_memory::FileSharedMemories;
use crate::{DirectoryRoles, Hosted, Organization, Server, admin};

fn git_hosts() -> Vec<String> {
    match std::env::var("SUPERFIK_GIT_HOSTS") {
        Ok(hosts) => hosts.split(',').map(|host| host.trim().to_owned()).filter(|host| !host.is_empty()).collect(),
        Err(_) => crate::checkouts::DEFAULT_HOSTS.map(str::to_owned).to_vec(),
    }
}

fn web_origins(organization: &Arc<Organization>) -> crate::session::Origins {
    let configured = std::env::var("SUPERFIK_WEB_ORIGINS").unwrap_or_default();
    let listed = configured.split(',').map(str::trim).filter(|o| !o.is_empty()).map(str::to_owned);
    let organization = Arc::clone(organization);
    crate::session::Origins::new(listed).with_live(Arc::new(move || organization.origin()))
}

fn github_app() -> Option<Arc<dyn crate::github_login::DeviceFlow>> {
    let id = std::env::var("SUPERFIK_GITHUB_CLIENT_ID").ok().filter(|id| !id.trim().is_empty())?;
    Some(Arc::new(crate::github_login::GitHubApp::new(id)))
}

fn checkouts_root(data: &Path) -> std::path::PathBuf {
    std::env::var_os("SUPERFIK_CHECKOUTS")
        .filter(|root| !root.is_empty())
        .map_or_else(|| data.join("checkouts"), std::path::PathBuf::from)
}

pub fn open(
    data: &Path,
    organization: Arc<Organization>,
    mut services: LocalServices,
    key: [u8; 32],
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    std::fs::create_dir_all(data)?;
    services = services.kept_in(&data.join("members"));
    let sealed: Arc<dyn superfik_backend::SecretStore> =
        Arc::new(SealedSecretStore::open(&data.join("secrets.json"), key)?);
    services.plugins = services.plugins.with_secrets(Arc::clone(&sealed));
    services.providers = services.providers.with_secrets(Arc::clone(&sealed));
    let app = github_app();
    let now =
        Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    let tokens = Arc::new(crate::github_login::KeptGitHubTokens::new(sealed, app.clone(), now));
    services.github_tokens = tokens.clone();
    let org = organization.directory.org();
    services.authorizer = Arc::new(RoleAuthorizer::new(org, DirectoryRoles(Arc::clone(&organization))));
    let audit = Arc::new(FileAudit::new(&data.join("audit.jsonl")));
    let exported: Arc<dyn AuditLog> = audit.clone();
    let hosted = Hosted {
        organization: Arc::clone(&organization),
        audit,
        sessions: Arc::new(FileProjectSessions::new(data.join("projects"))),
        memory: Arc::new(FileSharedMemories::new(data.join("memory"))),
        library: Arc::new(FileLibrary::new(data.join("library.json"))),
        governance: Arc::new(FileGovernance::new(data.to_path_buf())),
        checkouts: Arc::new(GitCheckouts::new(checkouts_root(data)).from_hosts(git_hosts())),
        comments: Arc::new(crate::comments::FileComments::new(data.join("comments"))),
        github: crate::GitHubSignIn { tokens, app },
    };
    Ok(routes(Server::start_for(services, Some(hosted))?, organization, exported))
}

pub fn routes(server: Server, organization: Arc<Organization>, audit: Arc<dyn AuditLog>) -> Router {
    let origins = web_origins(&organization);
    let authenticator: Arc<dyn crate::Authenticator> = organization.clone();
    let hooks = crate::hooks::routes(server.clone());
    let routes = crate::socket_for(server, authenticator, origins.clone())
        .merge(crate::sign_in::routes(Arc::clone(&organization), origins))
        .merge(hooks)
        .merge(admin::routes(organization, audit));
    crate::headers::hardened(routes)
}
