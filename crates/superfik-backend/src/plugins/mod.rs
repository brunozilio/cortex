//! Plugins on the backend: installing, connecting accounts, and what a
//! run gets from them ([`brief`]). Secrets stay here; the client only sees
//! [`PluginSummary`]s and [`Connection`]s.

mod brief;
mod channels;
mod cli_token;
pub mod model_providers;
pub mod oauth;
mod sealed;
mod secrets;
mod setup;
mod store;
mod vaults;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use superfik_core::{
    AuthKind, Connection, ConnectionId, CustomAgent, PluginId, PluginOrigin, PluginSummary, Principal, Skill,
};
use superfik_plugins::materialize::materialize;
use superfik_plugins::{Auth, Installed, PluginBook, PluginError, PluginPackage, catalog};
use superfik_protocol::ServerEvent;

pub(crate) use self::brief::discard;
pub use self::cli_token::{SystemTokenCommand, TokenCommand};
pub use self::oauth::{McpOAuth, OAuthFlow, OAuthRequest};
pub use self::sealed::SealedSecretStore;
pub use self::secrets::{InMemorySecretStore, KeychainSecretStore, SecretStore, Secrets};
pub use self::setup::{SetupRunner, SystemSetup};
pub use self::store::{FilePluginStore, InMemoryPluginStore, PluginStore};
use crate::outbox::Outbox;

#[derive(Clone)]
pub struct Plugins {
    store: Arc<dyn PluginStore>,
    secrets: Arc<dyn SecretStore>,
    oauth: Arc<dyn OAuthFlow>,
    cli: Arc<dyn TokenCommand>,
    setup: Arc<dyn SetupRunner>,
    catalog: Arc<Vec<PluginPackage>>,
    model_generations: Arc<Mutex<std::collections::HashMap<String, u64>>>,
    model_catalog: Arc<dyn model_providers::ModelCatalog>,
    lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for Plugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugins").field("catalog", &self.catalog.len()).finish_non_exhaustive()
    }
}

struct Present {
    package: PluginPackage,
    dir: PathBuf,
}

impl Plugins {
    pub fn new(
        store: Arc<dyn PluginStore>,
        secrets: Arc<dyn SecretStore>,
        oauth: Arc<dyn OAuthFlow>,
        cli: Arc<dyn TokenCommand>,
    ) -> Self {
        Self {
            store,
            secrets,
            oauth,
            cli,
            setup: Arc::new(SystemSetup),
            catalog: Arc::new(catalog::packages()),
            model_generations: Arc::default(),
            model_catalog: Arc::new(model_providers::ProviderCatalog),
            lock: Arc::default(),
        }
    }

    pub fn with_model_catalog(mut self, catalog: Arc<dyn model_providers::ModelCatalog>) -> Self {
        self.model_catalog = catalog;
        self
    }

    pub fn system() -> Self {
        Self::new(
            Arc::new(FilePluginStore::system()),
            Arc::new(KeychainSecretStore),
            Arc::new(McpOAuth::default()),
            Arc::new(SystemTokenCommand),
        )
    }

    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    pub fn secrets(&self) -> Arc<dyn SecretStore> {
        Arc::clone(&self.secrets)
    }

    pub fn with_store(mut self, store: Arc<dyn PluginStore>) -> Self {
        self.store = store;
        self
    }

    pub fn with_setup(mut self, setup: Arc<dyn SetupRunner>) -> Self {
        self.setup = setup;
        self
    }

    pub fn install_and_set_up(
        &self,
        owner: &Principal,
        plugin: Option<&PluginId>,
        folder: Option<&Path>,
        outbox: &Outbox,
    ) {
        let event = self.install(owner, plugin, folder);
        let installed = matches!(event, ServerEvent::Plugins { .. });
        outbox.send(event);
        if !installed {
            return;
        }
        let id = plugin.cloned().or_else(|| folder.and_then(|f| PluginPackage::load(f).ok()).map(|p| p.manifest.id));
        let book = self.store.load(owner);
        let Some(package) = id
            .and_then(|id| book.installed.iter().find(|i| i.id == id).cloned())
            .and_then(|installed| self.present_one(&self.store.packages(owner), &installed))
            .map(|present| present.package)
        else {
            return;
        };
        let Some(command) = package.manifest.setup.clone() else { return };
        if folder.is_some() && matches!(owner, Principal::Member(_)) {
            tracing::warn!(plugin = %package.manifest.id.0, "a member's folder plugin is installed without its setup");
            return;
        }
        let (runner, outbox, id) = (Arc::clone(&self.setup), outbox.clone(), package.manifest.id.clone());
        let (this, owner) = (self.clone(), owner.clone());
        std::thread::spawn(move || match runner.run(&command) {
            Ok(()) => outbox.send(this.list(&owner)),
            Err(message) => {
                let error = PluginError::Io(format!("{} setup failed: {message}", package.manifest.name));
                outbox.send(this.failure(Some(&id), error));
            }
        });
    }

    pub fn with_catalog(mut self, catalog: Vec<PluginPackage>) -> Self {
        self.catalog = Arc::new(catalog);
        self
    }

    fn catalog_package(&self, id: &PluginId) -> Option<&PluginPackage> {
        self.catalog.iter().find(|package| &package.manifest.id == id)
    }

    fn edit<T>(
        &self,
        owner: &Principal,
        edit: impl FnOnce(&mut PluginBook) -> Result<T, PluginError>,
    ) -> Result<T, PluginError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut book = self.store.load(owner);
        let value = edit(&mut book)?;
        self.store.save(owner, &book)?;
        Ok(value)
    }

    fn present(&self, owner: &Principal, book: &PluginBook) -> Vec<Present> {
        let root = self.store.packages(owner);
        book.installed.iter().filter_map(|installed| self.present_one(&root, installed)).collect()
    }

    fn present_one(&self, root: &Path, installed: &Installed) -> Option<Present> {
        let dir = root.join(installed.id.as_str());
        let on_disk = PluginPackage::load(&dir).ok();
        let package = match (&installed.origin, self.catalog_package(&installed.id)) {
            (PluginOrigin::Catalog, Some(shipped)) => {
                let stale = on_disk
                    .as_ref()
                    .is_none_or(|disk| disk.manifest != shipped.manifest || disk.files != shipped.files);
                if stale && let Err(error) = materialize(shipped, &dir) {
                    tracing::warn!(%error, plugin = shipped.id(), "could not lay out a plugin");
                }
                shipped.clone()
            }
            _ => PluginPackage { origin: installed.origin.clone(), ..on_disk? },
        };
        Some(Present { package, dir })
    }

    pub fn list(&self, owner: &Principal) -> ServerEvent {
        let book = self.store.load(owner);
        let mut plugins: Vec<PluginSummary> =
            self.catalog.iter().map(|package| package.summary(book.is_installed(&package.manifest.id))).collect();
        for present in self.present(owner, &book) {
            if !plugins.iter().any(|plugin| plugin.id == present.package.manifest.id) {
                plugins.push(present.package.summary(true));
            }
        }
        ServerEvent::Plugins { plugins, connections: book.connections }
    }

    pub fn install(&self, owner: &Principal, plugin: Option<&PluginId>, folder: Option<&Path>) -> ServerEvent {
        let result = self.try_install(owner, plugin, folder);
        self.answer(owner, plugin.cloned(), result)
    }

    fn try_install(
        &self,
        owner: &Principal,
        plugin: Option<&PluginId>,
        folder: Option<&Path>,
    ) -> Result<(), PluginError> {
        let package = match (plugin, folder) {
            (_, Some(folder)) => PluginPackage::load(folder)?,
            (Some(id), None) => self.catalog_package(id).cloned().ok_or_else(|| PluginError::NotFound(id.0.clone()))?,
            (None, None) => return Err(PluginError::Invalid("nothing to install".into())),
        };
        let id = package.manifest.id.clone();
        materialize(&package, &self.store.packages(owner).join(id.as_str()))?;
        self.edit(owner, |book| {
            book.install(id.clone(), package.origin.clone())?;
            if package.manifest.auth == Auth::None && !package.manifest.mcp.is_empty() {
                book.connect(connection(&package, String::new()), false)?;
            }
            Ok(())
        })
    }

    pub fn install_library(&self, owner: &Principal, items: &[superfik_core::library::LibraryItem]) -> ServerEvent {
        let id = PluginId(superfik_core::library::LIBRARY_PLUGIN.to_owned());
        let dir = self.store.packages(owner).join(id.as_str());
        let result = if items.is_empty() {
            self.edit(owner, |book| {
                if book.is_installed(&id) {
                    book.uninstall(&id)?;
                }
                Ok(())
            })
            .map(|()| {
                let _ = std::fs::remove_dir_all(&dir);
            })
        } else {
            superfik_plugins::library::package(items, &dir).and_then(|package| {
                materialize(&package, &dir)?;
                self.edit(owner, |book| match book.is_installed(&id) {
                    true => Ok(()),
                    false => book.install(id.clone(), package.origin.clone()),
                })
            })
        };
        self.answer(owner, Some(id), result)
    }

    pub fn uninstall(&self, owner: &Principal, plugin: &PluginId) -> ServerEvent {
        let result = self.edit(owner, |book| book.uninstall(plugin)).map(|dropped| {
            for connection in dropped {
                self.secrets.delete(owner, connection.id);
            }
            let _ = std::fs::remove_dir_all(self.store.packages(owner).join(plugin.as_str()));
        });
        self.answer(owner, Some(plugin.clone()), result)
    }

    pub fn disconnect(&self, owner: &Principal, id: ConnectionId) -> ServerEvent {
        let _ = self.edit(owner, |book| Ok(book.disconnect(id)));
        self.secrets.delete(owner, id);
        self.list(owner)
    }

    pub fn connect(
        &self,
        owner: &Principal,
        plugin: &PluginId,
        label: &str,
        values: &[(String, String)],
        outbox: &Outbox,
    ) {
        let book = self.store.load(owner);
        let Some(present) = book
            .installed
            .iter()
            .find(|i| &i.id == plugin)
            .and_then(|installed| self.present_one(&self.store.packages(owner), installed))
        else {
            return outbox.send(self.failure(Some(plugin), PluginError::NotFound(plugin.0.clone())));
        };
        let package = present.package;
        let label = label.trim().to_owned();
        let result = match &package.manifest.auth {
            Auth::None => Err(PluginError::Invalid(format!("{} needs no account", package.manifest.name))),
            Auth::ApiKey { fields } => {
                let secrets: Secrets = fields
                    .iter()
                    .filter_map(|field| {
                        let value = values.iter().find(|(key, _)| key == &field.key)?.1.trim();
                        (!value.is_empty()).then(|| (field.key.clone(), value.to_owned()))
                    })
                    .collect();
                match fields.iter().find(|field| !secrets.contains_key(&field.key)) {
                    Some(missing) => Err(PluginError::Invalid(format!("{} is empty", missing.label))),
                    None => {
                        let provider =
                            superfik_core::ModelProvider::ACCOUNTS.into_iter().find(|p| p.plugin_id() == package.id());
                        let valid = provider.map_or(Ok(()), |provider| {
                            let raw =
                                serde_json::to_string(&secrets).map_err(|e| PluginError::Invalid(e.to_string()))?;
                            superfik_plugins::providers::Config::parse(provider, &raw)
                                .map(|_| ())
                                .map_err(|e| PluginError::Invalid(e.to_string()))
                        });
                        valid.and_then(|()| self.keep(owner, &package, label, &secrets))
                    }
                }
            }
            Auth::Qr => self.keep(owner, &package, label, &Secrets::new()),
            Auth::Cli { program, args } => match self.cli.token(program, args) {
                Ok(_) => self.keep(owner, &package, label, &Secrets::new()),
                Err(message) => Err(PluginError::Io(message)),
            },
            Auth::OAuth(spec) => {
                let resource = spec.resource.clone().or_else(|| package.manifest.first_http_url().map(str::to_owned));
                let request = OAuthRequest { resource: resource.unwrap_or_default(), spec: spec.clone() };
                let (this, owner, outbox) = (self.clone(), owner.clone(), outbox.clone());
                std::thread::spawn(move || {
                    let id = package.manifest.id.clone();
                    let mut open = |url: String| outbox.send(ServerEvent::AuthorizeAt { plugin: id.clone(), url });
                    let result = this
                        .oauth
                        .sign_in(&request, &mut open)
                        .map_err(PluginError::Io)
                        .and_then(|secrets| this.keep(&owner, &package, label, &secrets));
                    outbox.send(this.answer(&owner, Some(package.manifest.id.clone()), result));
                });
                return;
            }
        };
        outbox.send(self.answer(owner, Some(plugin.clone()), result));
    }

    fn keep(
        &self,
        owner: &Principal,
        package: &PluginPackage,
        label: String,
        secrets: &Secrets,
    ) -> Result<(), PluginError> {
        let connection = connection(package, label);
        if !secrets.is_empty() {
            self.secrets.set(owner, connection.id, secrets).map_err(PluginError::Io)?;
        }
        let id = connection.id;
        let many = package.manifest.accounts == superfik_core::Accounts::Many;
        match self.edit(owner, |book| book.connect(connection, many)) {
            Ok(replaced) => {
                if let Some(old) = replaced {
                    self.secrets.delete(owner, old.id);
                }
                Ok(())
            }
            Err(error) => {
                self.secrets.delete(owner, id);
                Err(error)
            }
        }
    }

    fn answer(&self, owner: &Principal, plugin: Option<PluginId>, result: Result<(), PluginError>) -> ServerEvent {
        match result {
            Ok(()) => self.list(owner),
            Err(error) => self.failure(plugin.as_ref(), error),
        }
    }

    fn failure(&self, plugin: Option<&PluginId>, error: PluginError) -> ServerEvent {
        tracing::warn!(%error, ?plugin, "plugin command failed");
        ServerEvent::PluginFailed { plugin: plugin.cloned(), message: error.to_string() }
    }

    pub fn skills(&self, owner: &Principal) -> Vec<Skill> {
        let book = self.store.load(owner);
        self.present(owner, &book).iter().flat_map(|present| present.package.skills()).collect()
    }

    pub fn agents(&self, owner: &Principal) -> Vec<CustomAgent> {
        let book = self.store.load(owner);
        self.present(owner, &book).iter().flat_map(|present| present.package.agents()).collect()
    }
}

fn connection(package: &PluginPackage, label: String) -> Connection {
    Connection {
        id: ConnectionId::new(),
        plugin: package.manifest.id.clone(),
        plugin_name: package.manifest.name.clone(),
        label,
        icon: package.manifest.icon.clone(),
    }
}

fn stores_secrets(kind: AuthKind) -> bool {
    matches!(kind, AuthKind::ApiKey | AuthKind::OAuth)
}

#[cfg(test)]
mod tests;
