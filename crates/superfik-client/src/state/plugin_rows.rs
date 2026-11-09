//! What the Plugins node lists: one row per installed plugin, with a select
//! of the account the session uses (or none), and the menu that select
//! opens. A plugin with no account yet connects one instead.

use superfik_core::{Connection, ConnectionId, PluginId, PluginSummary};

use super::connections::SKILL;
use super::{Action, Effect, PopoverKind, Session, SettingsPane};

#[derive(Debug, Clone, PartialEq)]
pub struct PluginRow<'a> {
    pub plugin: &'a PluginId,
    pub name: &'a str,
    pub icon: &'a str,
    pub used: Option<&'a Connection>,
    pub accounts: Vec<&'a Connection>,
}

impl PluginRow<'_> {
    pub fn action(&self) -> Action {
        if self.accounts.is_empty() {
            Action::ConnectPlugin(self.plugin.clone())
        } else {
            Action::OpenAccountMenu(self.plugin.clone())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccountChoice<'a> {
    Account { connection: &'a Connection, selected: bool },
    NotUsed { selected: bool },
    Add,
}

impl AccountChoice<'_> {
    pub fn action(&self, plugin: &PluginId, used: Option<ConnectionId>) -> Option<Action> {
        match self {
            Self::Account { connection, .. } => Some(Action::UseConnection(connection.id)),
            Self::NotUsed { .. } => used.map(Action::RemoveConnection),
            Self::Add => Some(Action::ConnectPlugin(plugin.clone())),
        }
    }
}

pub fn account_label(connection: &Connection) -> &str {
    if connection.label.is_empty() { &connection.plugin_name } else { &connection.label }
}

impl Session {
    pub fn plugin_rows(&self) -> Vec<PluginRow<'_>> {
        let used = self.session_connections();
        let mut rows: Vec<PluginRow> = self
            .plugins
            .list
            .iter()
            .filter(|p| p.installed)
            .map(|p| PluginRow {
                plugin: &p.id,
                name: &p.name,
                icon: &p.icon,
                used: used.iter().copied().find(|c| c.plugin == p.id),
                accounts: self.plugins.connections_of(&p.id).collect(),
            })
            .collect();
        for &c in &used {
            if !rows.iter().any(|row| *row.plugin == c.plugin) {
                rows.push(PluginRow {
                    plugin: &c.plugin,
                    name: &c.plugin_name,
                    icon: &c.icon,
                    used: Some(c),
                    accounts: vec![c],
                });
            }
        }
        rows
    }

    pub fn account_menu_row(&self) -> Option<PluginRow<'_>> {
        let open = self.account_menu.as_ref().filter(|_| self.popover == Some(PopoverKind::PluginAccounts))?;
        self.plugin_rows().into_iter().find(|row| row.plugin == open)
    }

    pub fn account_choices(&self) -> Vec<AccountChoice<'_>> {
        let Some(row) = self.account_menu_row() else { return Vec::new() };
        let used = row.used.map(|c| c.id);
        let accounts = row
            .accounts
            .iter()
            .map(|&connection| AccountChoice::Account { connection, selected: used == Some(connection.id) });
        accounts.chain([AccountChoice::NotUsed { selected: used.is_none() }, AccountChoice::Add]).collect()
    }

    pub fn account_choice_action(&self, index: usize) -> Option<Action> {
        let row = self.account_menu_row()?;
        self.account_choices().get(index)?.action(row.plugin, row.used.map(|c| c.id))
    }

    pub(super) fn unconnected_plugins(&self, query: &str) -> impl Iterator<Item = &PluginSummary> {
        self.plugins.list.iter().filter(move |p| {
            p.installed
                && self.plugins.connections_of(&p.id).next().is_none()
                && (p.id.as_str().contains(query) || p.name.to_ascii_lowercase().contains(query))
        })
    }

    pub(super) fn open_account_menu(&mut self, plugin: PluginId) -> Vec<Effect> {
        let same = self.account_menu.as_ref() == Some(&plugin);
        let open = self.popover == Some(PopoverKind::PluginAccounts) && same;
        self.account_menu = Some(plugin);
        self.popover = if open { None } else { Some(PopoverKind::PluginAccounts) };
        Vec::new()
    }

    pub(super) fn use_connection(&mut self, id: ConnectionId) -> Vec<Effect> {
        let Some(connection) = self.plugins.connections.iter().find(|c| c.id == id).cloned() else {
            return Vec::new();
        };
        let others: Vec<ConnectionId> = self
            .session_connections()
            .iter()
            .filter(|c| c.plugin == connection.plugin && c.id != id)
            .map(|c| c.id)
            .collect();
        for other in others {
            self.remove_connection(other);
        }
        let c = self.conversation_mut();
        if !c.connections.iter().any(|known| known.id == id) {
            c.connections.push(connection);
        }
        self.popover = None;
        Vec::new()
    }

    pub(super) fn connect_plugin(&mut self, id: PluginId) -> Vec<Effect> {
        let composer = &mut self.conversation_mut().composer;
        if composer.trigger_query(&[SKILL]).is_some() {
            composer.clear_word();
        }
        let ai = self.plugins.get(&id).is_some_and(|plugin| plugin.is_ai_provider());
        let vault = self.plugins.get(&id).is_some_and(|plugin| plugin.vault.is_some());
        let mut effects = self.open_plugin_pane(if ai {
            SettingsPane::Ai
        } else if vault {
            SettingsPane::Vaults
        } else {
            SettingsPane::Integrations
        });
        if ai && self.plugins.get(&id).is_some_and(|plugin| !plugin.installed) {
            self.plugins.panel.selected = Some(id.clone());
            effects.extend(self.install_plugin(id));
        } else {
            effects.extend(self.begin_connect(id));
        }
        effects
    }
}
