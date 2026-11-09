//! Password vaults for agents. A run whose session picked a vault account
//! gets the vault tools over MCP on a loopback port, with a bearer token
//! bound to that run and revoked when it ends. The agent only ever names
//! secrets: the backend asks the user before each use, runs the command
//! with the values in its environment and hides them in what it prints.
//!
//! [`Vault::handle`] is the protocol over a token; [`Vault::start`] adds the
//! socket. Each kind of vault is a [`VaultProvider`]; the shell sits
//! behind [`CommandRunner`].

mod approvals;
pub mod bitwarden;
mod bitwarden_passkeys;
mod credentials;
mod granting;
mod logins;
mod lookup;
mod mcp;
mod provider;
pub mod runner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::Value;
use superfik_core::vault::{
    LIST_GRANTED_TOOL, LIST_SECRETS_TOOL, RELEASE_CREDENTIALS_TOOL, REQUEST_CREDENTIALS_TOOL, RUN_WITH_SECRETS_TOOL,
    VAULT_SERVER, ask_subject, redact,
};
use superfik_core::{AskId, Decision, Principal, RunId, VaultTool};

use self::approvals::Approvals;
pub use self::approvals::Notify;
pub use self::bitwarden::{Bitwarden, BitwardenCli, SystemBitwarden};
pub use self::logins::Logins;
use self::lookup::Folder;
pub use self::provider::{Item, KeptPasskey, VaultAccount, VaultPasskey, VaultProvider, VaultProviders};
pub use self::runner::{CommandRunner, Ran, ShellRunner};
use crate::loopback::{self, RpcError};

pub(crate) const APPROVAL_LIMIT: Duration = Duration::from_secs(600);
const MAX_OUTPUT: usize = 30_000;

struct Grant {
    owner: Principal,
    run: RunId,
    cwd: PathBuf,
    accounts: Vec<VaultAccount>,
    sessions: Mutex<Vec<Option<String>>>,
    granted: Mutex<Vec<credentials::Granted>>,
}

#[derive(Clone)]
pub struct Vault {
    url: String,
    grants: Arc<Mutex<HashMap<String, Arc<Grant>>>>,
    approvals: Approvals,
    providers: VaultProviders,
    runner: Arc<dyn CommandRunner>,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault").field("url", &self.url).finish_non_exhaustive()
    }
}

impl Vault {
    pub fn new(providers: VaultProviders, runner: Arc<dyn CommandRunner>, notify: Notify) -> Self {
        Self { url: String::new(), grants: Arc::default(), approvals: Approvals::new(notify), providers, runner }
    }

    pub fn start(providers: VaultProviders, runner: Arc<dyn CommandRunner>, notify: Notify) -> Option<Self> {
        let server = match tiny_http::Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(error) => {
                tracing::warn!(%error, "vault tools unavailable");
                return None;
            }
        };
        let port = server.server_addr().to_ip()?.port();
        let vault = Self { url: format!("http://127.0.0.1:{port}/mcp"), ..Self::new(providers, runner, notify) };
        let served = vault.clone();
        std::thread::Builder::new()
            .name("superfik-vault-mcp".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    let vault = served.clone();
                    std::thread::spawn(move || vault.serve(request));
                }
            })
            .ok()?;
        Some(vault)
    }

    pub fn grant(&self, owner: &Principal, run: RunId, cwd: PathBuf, accounts: Vec<VaultAccount>) -> VaultTool {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let sessions = Mutex::new(vec![None; accounts.len()]);
        let grant = Grant { owner: owner.clone(), run, cwd, accounts, sessions, granted: Mutex::default() };
        self.lock().insert(token.clone(), Arc::new(grant));
        VaultTool { url: self.url.clone(), token }
    }

    pub fn revoke(&self, token: &str) {
        if let Some(grant) = self.lock().remove(token) {
            self.approvals.end(grant.run);
        }
    }

    pub fn approve(
        &self,
        owner: &Principal,
        run: RunId,
        item: &str,
        subject: String,
    ) -> Result<Vec<VaultAccount>, String> {
        let grant = self.run_grant(owner, run)?;
        if Self::login_approved(&grant, item) {
            return Ok(grant.accounts.clone());
        }
        if !self.approvals.ask(&grant.owner, grant.run, &asker(&grant.accounts), subject, APPROVAL_LIMIT) {
            return Err("The user did not allow this. Do not retry it unless they ask you to.".into());
        }
        Ok(grant.accounts.clone())
    }

    fn run_grant(&self, owner: &Principal, run: RunId) -> Result<Arc<Grant>, String> {
        let grant = self.lock().values().find(|g| g.run == run && &g.owner == owner).cloned();
        grant.ok_or_else(|| "This session picked no vault account; the user can add one with `/`.".to_owned())
    }

    pub fn answer(&self, owner: &Principal, ask: &AskId, decision: &Decision) -> bool {
        self.approvals.answer(owner, ask, decision)
    }

    pub fn handle(&self, token: &str, message: &Value) -> Result<Option<Value>, u16> {
        let grant = self.lock().get(token).cloned().ok_or(401_u16)?;
        Ok(loopback::respond(message, VAULT_SERVER, mcp::tools, |name, arguments| self.call(&grant, name, arguments)))
    }

    fn call(&self, grant: &Grant, name: &str, arguments: &Value) -> Result<Value, RpcError> {
        let result = match name {
            LIST_SECRETS_TOOL => self.folders(grant).map(|folders| lookup::listing(&folders)),
            RUN_WITH_SECRETS_TOOL => self.run_with_secrets(grant, arguments),
            REQUEST_CREDENTIALS_TOOL => self.request_credentials(grant, arguments),
            LIST_GRANTED_TOOL => Ok(Self::list_granted(grant)),
            RELEASE_CREDENTIALS_TOOL => Ok(Self::release(grant)),
            _ => return Err(loopback::unknown_tool(name)),
        };
        Ok(match result {
            Ok(text) => loopback::text(text, false),
            Err(text) => loopback::text(text, true),
        })
    }

    fn run_with_secrets(&self, grant: &Grant, arguments: &Value) -> Result<String, String> {
        let call = mcp::run_call(arguments)?;
        let values = lookup::resolve(&self.folders(grant)?, &call.uses)?;
        let subject = ask_subject(&call.command, &call.uses);
        if !self.approvals.ask(&grant.owner, grant.run, &asker(&grant.accounts), subject, APPROVAL_LIMIT) {
            return Err("The user did not allow this. Do not retry it unless they ask you to.".into());
        }
        let ran = self.runner.run(&call.command, &grant.cwd, &values, call.limit)?;
        let status = match (ran.timed_out, ran.code) {
            (true, _) => format!("timed out after {}s", call.limit.as_secs()),
            (false, Some(code)) => format!("exit {code}"),
            (false, None) => "killed".to_owned(),
        };
        Ok(format!("{status}\n{}", clip(&redact(&ran.output, &values))))
    }

    fn folders(&self, grant: &Grant) -> Result<Vec<Folder>, String> {
        let mut sessions = grant.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let mut folders = Vec::new();
        for (account, session) in grant.accounts.iter().zip(sessions.iter_mut()) {
            let provider = self
                .providers
                .get(account.kind)
                .ok_or_else(|| format!("{}: this vault is not supported", account.title))?;
            let items = provider.items(account, session).map_err(|error| format!("{}: {error}", account.title))?;
            folders.push(Folder { title: account.title.clone(), name: account.scope.clone(), items });
        }
        Ok(folders)
    }

    fn serve(&self, mut request: tiny_http::Request) {
        let Some(token) = loopback::bearer(&request) else { return loopback::reply(request, 401, None) };
        if !self.lock().contains_key(&token) {
            return loopback::reply(request, 401, None);
        }
        let message = match loopback::read_message(&mut request) {
            Ok(message) => message,
            Err(status) => return loopback::reply(request, status, None),
        };
        match self.handle(&token, &message) {
            Ok(Some(answer)) => loopback::reply(request, 200, Some(answer)),
            Ok(None) => loopback::reply(request, 202, None),
            Err(status) => loopback::reply(request, status, None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<Grant>>> {
        self.grants.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn asker(accounts: &[VaultAccount]) -> String {
    let mut names: Vec<&str> = accounts.iter().map(|account| account.kind.name()).collect();
    names.dedup();
    names.join(" · ")
}

fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_OUTPUT) {
        Some((at, _)) => format!("{}\n… ({} more characters)", &text[..at], text[at..].chars().count()),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests;
