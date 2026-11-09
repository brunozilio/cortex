//! What one owner has installed and connected: plugin ids (with the folder
//! of local plugins) and connections. No secrets: those live in the
//! backend's secret store.

use serde::{Deserialize, Serialize};
use superfik_core::{Connection, ConnectionId, PluginId, PluginOrigin};

use crate::error::PluginError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub id: PluginId,
    pub origin: PluginOrigin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginBook {
    #[serde(default)]
    pub installed: Vec<Installed>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

impl PluginBook {
    pub fn is_installed(&self, id: &PluginId) -> bool {
        self.installed.iter().any(|plugin| &plugin.id == id)
    }

    pub fn install(&mut self, id: PluginId, origin: PluginOrigin) -> Result<(), PluginError> {
        if self.is_installed(&id) {
            return Err(PluginError::AlreadyInstalled(id.0));
        }
        self.installed.push(Installed { id, origin });
        Ok(())
    }

    pub fn uninstall(&mut self, id: &PluginId) -> Result<Vec<Connection>, PluginError> {
        let index = self
            .installed
            .iter()
            .position(|plugin| &plugin.id == id)
            .ok_or_else(|| PluginError::NotFound(id.0.clone()))?;
        self.installed.remove(index);
        let (dropped, kept) = std::mem::take(&mut self.connections).into_iter().partition(|c| &c.plugin == id);
        self.connections = kept;
        Ok(dropped)
    }

    pub fn connections_of(&self, id: &PluginId) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |connection| &connection.plugin == id)
    }

    pub fn connection(&self, id: ConnectionId) -> Option<&Connection> {
        self.connections.iter().find(|connection| connection.id == id)
    }

    pub fn connect(&mut self, connection: Connection, many: bool) -> Result<Option<Connection>, PluginError> {
        if !self.is_installed(&connection.plugin) {
            return Err(PluginError::NotFound(connection.plugin.0));
        }
        let replaced = if many {
            None
        } else {
            let index = self.connections.iter().position(|c| c.plugin == connection.plugin);
            index.map(|index| self.connections.remove(index))
        };
        self.connections.push(connection);
        Ok(replaced)
    }

    pub fn disconnect(&mut self, id: ConnectionId) -> Option<Connection> {
        let index = self.connections.iter().position(|connection| connection.id == id)?;
        Some(self.connections.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(plugin: &str, label: &str) -> Connection {
        Connection {
            id: ConnectionId::new(),
            plugin: PluginId(plugin.into()),
            plugin_name: plugin.into(),
            label: label.into(),
            icon: String::new(),
        }
    }

    #[test]
    fn install_connect_and_uninstall() {
        let mut book = PluginBook::default();
        let gmail = PluginId("gmail".into());
        assert!(book.connect(connection("gmail", "work"), true).is_err(), "not installed");
        book.install(gmail.clone(), PluginOrigin::Catalog).expect("install");
        assert_eq!(
            book.install(gmail.clone(), PluginOrigin::Catalog),
            Err(PluginError::AlreadyInstalled("gmail".into()))
        );
        book.connect(connection("gmail", "work"), true).expect("connect");
        book.connect(connection("gmail", "home"), true).expect("connect");
        assert_eq!(book.connections_of(&gmail).count(), 2);
        let dropped = book.uninstall(&gmail).expect("uninstall");
        assert_eq!(dropped.len(), 2);
        assert!(book.connections.is_empty());
        assert!(book.uninstall(&gmail).is_err());
    }

    #[test]
    fn one_account_plugins_replace_their_connection() {
        let mut book = PluginBook::default();
        book.install(PluginId("github".into()), PluginOrigin::Catalog).expect("install");
        let first = connection("github", "a");
        let first_id = first.id;
        assert_eq!(book.connect(first, false).expect("connect"), None);
        let replaced = book.connect(connection("github", "b"), false).expect("connect");
        assert_eq!(replaced.map(|c| c.id), Some(first_id));
        assert_eq!(book.connections.len(), 1);
        let id = book.connections[0].id;
        assert!(book.connection(id).is_some());
        assert_eq!(book.disconnect(id).map(|c| c.label), Some("b".into()));
        assert!(book.disconnect(id).is_none());
    }
}
