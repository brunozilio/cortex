//! The "add account" sheet: typed values for a new account (its name, then
//! the plugin's fields), how focus moves through it and what Connect sends.

use superfik_core::{AuthKind, PluginId};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::command;
use crate::state::{EditKey, Effect, Session, SettingsPane};

#[derive(Debug, Clone)]
pub struct ConnectSheet {
    pub plugin: PluginId,
    pub label: TextEdit,
    pub fields: Vec<TextEdit>,
    pub focus: usize,
}

impl ConnectSheet {
    pub fn connect_slot(&self) -> usize {
        self.fields.len() + 1
    }

    pub fn cancel_slot(&self) -> usize {
        self.fields.len() + 2
    }
}

impl Session {
    pub(in crate::state) fn begin_connect(&mut self, id: PluginId) -> Vec<Effect> {
        let Some(plugin) = self.plugins.get(&id) else { return Vec::new() };
        let fields =
            plugin.fields.iter().map(|field| TextEdit::new(field.default_value.clone().unwrap_or_default())).collect();
        let panel = &mut self.plugins.panel;
        panel.local_models = false;
        panel.selected = Some(id.clone());
        panel.sheet = Some(ConnectSheet { plugin: id, label: TextEdit::default(), fields, focus: 0 });
        Vec::new()
    }

    pub(in crate::state) fn focus_connect_field(&mut self, index: usize) -> Vec<Effect> {
        if let Some(sheet) = self.connect_sheet_mut() {
            sheet.focus = index.min(sheet.fields.len());
        }
        Vec::new()
    }

    pub(in crate::state) fn step_connect_focus(&mut self, back: bool) -> Vec<Effect> {
        if self.plugins.panel.local_models {
            self.plugins.panel.local_focus = 1 - self.plugins.panel.local_focus;
            return Vec::new();
        }
        if let Some(sheet) = self.connect_sheet_mut() {
            let slots = sheet.cancel_slot() + 1;
            sheet.focus = if back { (sheet.focus + slots - 1) % slots } else { (sheet.focus + 1) % slots };
        }
        Vec::new()
    }

    fn connect_sheet_mut(&mut self) -> Option<&mut ConnectSheet> {
        self.plugins.panel.sheet.as_mut()
    }

    pub(in crate::state) fn edit_connect_sheet(&mut self, key: &EditKey) -> bool {
        if self.settings.open && self.settings.pane == SettingsPane::Ai && self.plugins.panel.local_models {
            if self.plugins.panel.local_focus == 0 {
                crate::state::composer::apply_key(&mut self.plugins.local_model, key.clone());
            }
            return true;
        }
        let Some(sheet) = self.connect_sheet_mut() else { return false };
        let field = match sheet.focus {
            0 => &mut sheet.label,
            n => match sheet.fields.get_mut(n - 1) {
                Some(field) => field,
                None => return true,
            },
        };
        crate::state::composer::apply_key(field, key.clone());
        true
    }

    pub(in crate::state) fn enter_connect_sheet(&mut self) -> Option<Vec<Effect>> {
        let sheet = self.connect_sheet_mut()?;
        if sheet.focus < sheet.fields.len() {
            sheet.focus += 1;
            return Some(Vec::new());
        }
        if sheet.focus == sheet.cancel_slot() {
            self.plugins.panel.sheet = None;
            return Some(Vec::new());
        }
        Some(self.confirm_connect())
    }

    pub(in crate::state) fn confirm_connect(&mut self) -> Vec<Effect> {
        let Some(sheet) = self.plugins.panel.sheet.take() else { return Vec::new() };
        self.plugins.panel.ai_adding = false;
        let Some(plugin) = self.plugins.list.iter().find(|p| p.id == sheet.plugin) else { return Vec::new() };
        let values =
            plugin.fields.iter().zip(&sheet.fields).map(|(f, edit)| (f.key.clone(), edit.text().to_owned())).collect();
        if plugin.auth == AuthKind::OAuth {
            self.plugins.panel.pending = Some(sheet.plugin.clone());
        }
        command(ClientCommand::Connect { plugin: sheet.plugin, label: sheet.label.text().trim().to_owned(), values })
    }
}
