//! The Integrations and Vaults panes of Settings: shelves of plugins
//! (installed first, then by category), each a card that installs or
//! removes it and connects accounts (typed values in a sheet, or a browser
//! sign-in the backend starts). Vault plugins list under Vaults, the rest
//! under Integrations.

use std::path::PathBuf;

use superfik_core::{Connection, ConnectionId, PluginId, PluginSummary};
use superfik_protocol::{ClientCommand, ServerEvent};
use superfik_ui::TextEdit;

use super::{Effect, Notice, Session, SettingsPane};

mod connect;

pub use connect::ConnectSheet;

#[derive(Debug, Clone, Default)]
pub struct PluginsState {
    pub list: Vec<PluginSummary>,
    pub connections: Vec<Connection>,
    pub loaded: bool,
    pub local_model: TextEdit,
    pub downloads: std::collections::BTreeMap<String, superfik_protocol::DownloadState>,
    pub panel: PluginsPanel,
}

#[derive(Debug, Clone, Default)]
pub struct PluginsPanel {
    pub selected: Option<PluginId>,
    pub scroll: f32,
    pub sheet: Option<ConnectSheet>,
    pub pending: Option<PluginId>,
    pub connect_after_install: Option<PluginId>,
    pub ai_adding: bool,
    pub local_models: bool,
    pub local_focus: usize,
    pub local_picker: bool,
}

impl PluginsState {
    pub fn get(&self, id: &PluginId) -> Option<&PluginSummary> {
        self.list.iter().find(|plugin| &plugin.id == id)
    }

    pub fn connections_of<'a>(&'a self, id: &'a PluginId) -> impl Iterator<Item = &'a Connection> {
        self.connections.iter().filter(move |connection| &connection.plugin == id)
    }

    pub fn selected(&self) -> Option<&PluginSummary> {
        self.panel.selected.as_ref().and_then(|id| self.get(id))
    }

    pub fn ai_groups(&self) -> Vec<AiProviderGroup<'_>> {
        [
            ("ChatGPT", "openai", ["openai-subscription", "openai"]),
            ("Claude", "claude", ["claude-subscription", "anthropic"]),
        ]
        .into_iter()
        .filter_map(|(name, icon, ids)| {
            let plugins: Vec<_> =
                ids.into_iter().filter_map(|id| self.list.iter().find(|p| p.id.as_str() == id)).collect();
            (!plugins.is_empty()).then_some(AiProviderGroup { name, icon, plugins })
        })
        .collect()
    }

    pub fn shelves(&self, vaults: bool) -> Vec<(Shelf, Vec<&PluginSummary>)> {
        let matches = |plugin: &&PluginSummary| plugin.vault.is_some() == vaults && !plugin.is_ai_provider();
        let mut shelves = vec![(Shelf::Installed, self.list.iter().filter(|p| p.installed).filter(matches).collect())];
        for category in CATEGORIES {
            let plugins = self.list.iter().filter(|p| !p.installed && p.category == category).filter(matches).collect();
            shelves.push((Shelf::Category(category), plugins));
        }
        let other = self
            .list
            .iter()
            .filter(|p| !p.installed && !CATEGORIES.contains(&p.category.as_str()))
            .filter(matches)
            .collect();
        shelves.push((Shelf::Category("other"), other));
        shelves.retain(|(_, plugins): &(Shelf, Vec<&PluginSummary>)| !plugins.is_empty());
        shelves
    }
}

#[derive(Debug)]
pub struct AiProviderGroup<'a> {
    pub name: &'static str,
    pub icon: &'static str,
    pub plugins: Vec<&'a PluginSummary>,
}

pub const CATEGORIES: [&str; 4] = ["development", "productivity", "data", "web"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    Installed,
    Category(&'static str),
}

fn command(command: ClientCommand) -> Vec<Effect> {
    vec![Effect::Command(Box::new(command))]
}

impl Session {
    pub fn plugins_pane_open(&self) -> bool {
        self.settings.open && self.settings.pane.lists_plugins()
    }

    pub(super) fn open_plugins(&mut self) -> Vec<Effect> {
        self.open_plugin_pane(SettingsPane::Integrations)
    }

    pub(super) fn open_plugin_pane(&mut self, pane: SettingsPane) -> Vec<Effect> {
        self.settings.pane = pane;
        self.plugins.panel = PluginsPanel::default();
        self.open_settings()
    }

    pub(super) fn plugins_back(&mut self) -> Vec<Effect> {
        let panel = &mut self.plugins.panel;
        if panel.local_models {
            panel.local_models = false;
            return Vec::new();
        }
        if self.settings.open && panel.sheet.take().is_none() && panel.selected.take().is_none() {
            return self.close_settings();
        }
        Vec::new()
    }

    pub(super) fn scroll_settings(&mut self, by: f32, max: f32) -> Vec<Effect> {
        let panel = &mut self.plugins.panel;
        panel.scroll = (panel.scroll + by).clamp(0.0, max.max(0.0));
        Vec::new()
    }

    pub(super) fn select_plugin(&mut self, id: PluginId) -> Vec<Effect> {
        let panel = &mut self.plugins.panel;
        panel.local_models = false;
        panel.selected = Some(id);
        panel.sheet = None;
        Vec::new()
    }

    fn set_pending(&mut self, id: Option<PluginId>) {
        self.plugins.panel.pending = id;
    }

    pub(super) fn install_plugin(&mut self, id: PluginId) -> Vec<Effect> {
        if self.settings.pane == SettingsPane::Ai && self.plugins.get(&id).is_some_and(|plugin| plugin.is_ai_provider())
        {
            self.plugins.panel.connect_after_install = Some(id.clone());
        }
        self.set_pending(Some(id.clone()));
        command(ClientCommand::InstallPlugin { plugin: Some(id), folder: None })
    }

    pub(super) fn install_plugin_folder(&mut self, folder: PathBuf) -> Vec<Effect> {
        command(ClientCommand::InstallPlugin { plugin: None, folder: Some(folder) })
    }

    pub(super) fn uninstall_plugin(&mut self, id: PluginId) -> Vec<Effect> {
        if self.plugins.get(&id).is_some_and(|p| !p.removable()) {
            return Vec::new();
        }
        for conversation in &mut self.conversations {
            conversation.connections.retain(|c| c.plugin != id);
        }
        command(ClientCommand::UninstallPlugin { plugin: id })
    }

    pub(super) fn disconnect(&mut self, id: ConnectionId) -> Vec<Effect> {
        for conversation in &mut self.conversations {
            conversation.connections.retain(|c| c.id != id);
        }
        command(ClientCommand::Disconnect { connection: id })
    }

    pub(super) fn plugins_event(&mut self, event: ServerEvent) -> Vec<Effect> {
        match event {
            ServerEvent::Plugins { plugins, connections } => {
                let installed = |list: &[PluginSummary]| -> Vec<PluginId> {
                    list.iter().filter(|p| p.installed).map(|p| p.id.clone()).collect()
                };
                let changed = self.plugins.loaded && installed(&self.plugins.list) != installed(&plugins);
                let waiting = self.plugins.panel.pending.clone();
                let connect_after_install = self.plugins.panel.connect_after_install.clone();
                let opens = waiting.filter(|id| {
                    plugins.iter().any(|p| &p.id == id && p.installed && p.needs_account())
                        && !connections.iter().any(|c| &c.plugin == id)
                });
                let provider_connections = |list: &[Connection]| {
                    list.iter()
                        .filter(|c| {
                            superfik_core::ModelProvider::ACCOUNTS.iter().any(|p| p.plugin_id() == c.plugin.as_str())
                        })
                        .map(|c| c.id)
                        .collect::<Vec<_>>()
                };
                let models_changed =
                    provider_connections(&self.plugins.connections) != provider_connections(&connections);
                self.plugins.list = plugins;
                self.plugins.connections = connections;
                self.plugins.loaded = true;
                self.set_pending(None);
                if let Some(id) = opens {
                    self.plugins.panel.selected = Some(id);
                }
                let mut effects = match (&self.root, changed) {
                    (Some(path), true) => vec![
                        Effect::Command(Box::new(ClientCommand::ListSkills { path: path.clone() })),
                        Effect::Command(Box::new(ClientCommand::ListAgents { path: path.clone() })),
                    ],
                    _ => Vec::new(),
                };
                if let Some(id) = connect_after_install
                    && self.plugins.get(&id).is_some_and(|plugin| plugin.installed)
                {
                    self.plugins.panel.connect_after_install = None;
                    effects.extend(self.begin_connect(id));
                }
                if models_changed {
                    effects.push(Effect::Command(Box::new(ClientCommand::ListModels)));
                }
                effects
            }
            ServerEvent::AuthorizeAt { url, .. } => vec![Effect::OpenUrl(url)],
            ServerEvent::PluginFailed { message, .. } => {
                self.plugins.panel.connect_after_install = None;
                self.set_pending(None);
                self.notice = Some(Notice::Raw(message));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}
