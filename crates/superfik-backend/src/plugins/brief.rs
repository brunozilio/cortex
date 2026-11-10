//! What a run gets from plugins: every installed plugin's folder (its
//! skills and agents) and an MCP configuration for the accounts the
//! session picked, written to a private file removed when the run ends.

use std::path::{Path, PathBuf};

use superfik_core::{PluginBrief, Principal, PromptRequest, RunId};
use superfik_plugins::Auth;
use superfik_plugins::mcp_config::{Account, plan};

use super::{Plugins, Secrets, stores_secrets};

impl Plugins {
    pub fn brief(&self, owner: &Principal, run: RunId, request: &PromptRequest) -> Result<PromptRequest, String> {
        let book = self.store.load(owner);
        let present = self.present(owner, &book);
        let mut picked: Vec<(usize, &superfik_core::Connection, Secrets)> = Vec::new();
        for wanted in &request.connections {
            let Some(connection) = book.connection(wanted.id) else {
                tracing::debug!(account = %wanted.title(), "account no longer connected");
                continue;
            };
            let Some(index) = present.iter().position(|p| p.package.manifest.id == connection.plugin) else { continue };
            let secrets = self.secrets_for(owner, &present[index].package.manifest.auth, connection)?;
            picked.push((index, connection, secrets));
        }
        let accounts: Vec<Account> = picked
            .iter()
            .map(|(index, connection, secrets)| Account {
                manifest: &present[*index].package.manifest,
                connection,
                secrets,
            })
            .collect();
        let plan = plan(&accounts).map_err(|error| error.to_string())?;
        let mcp_config = if plan.is_empty() {
            None
        } else {
            let path = mcp_path(run);
            let mut config = plan.config;
            super::setup::resolve_system_programs(&mut config);
            write_private(&path, &config.to_string()).map_err(|error| error.to_string())?;
            Some(path)
        };
        let brief = PluginBrief {
            mcp_config,
            plugin_dirs: present.iter().map(|p| p.dir.clone()).collect(),
            local_dirs: Vec::new(),
            instructions: plan.instructions,
        };
        Ok(PromptRequest { plugins: (!brief.is_empty()).then_some(brief), ..request.clone() })
    }

    fn secrets_for(
        &self,
        owner: &Principal,
        auth: &Auth,
        connection: &superfik_core::Connection,
    ) -> Result<Secrets, String> {
        let signed_out = || format!("{} is signed out; connect it again in Plugins", connection.title());
        match auth {
            Auth::None => Ok(Secrets::new()),
            Auth::Cli { program, args } => {
                let token =
                    self.cli.token(program, args).map_err(|error| format!("{}: {error}", connection.title()))?;
                Ok(Secrets::from([("token".to_owned(), token)]))
            }
            auth if stores_secrets(auth.kind()) => {
                let secrets = self.secrets.get(owner, connection.id).ok_or_else(signed_out)?;
                match self.oauth.refresh(&secrets) {
                    Ok(Some(fresh)) => {
                        self.secrets.set(owner, connection.id, &fresh)?;
                        Ok(fresh)
                    }
                    Ok(None) => Ok(secrets),
                    Err(error) => Err(format!("{}: {error}", signed_out())),
                }
            }
            _ => Ok(Secrets::new()),
        }
    }
}

pub fn discard(brief: Option<&PluginBrief>) {
    if let Some(path) = brief.and_then(|brief| brief.mcp_config.as_ref()) {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn mcp_path(run: RunId) -> PathBuf {
    std::env::temp_dir().join(format!("superfik-mcp-{run}.json"))
}

pub(crate) fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    superfik_private_file::write(path, text.as_bytes())
}
