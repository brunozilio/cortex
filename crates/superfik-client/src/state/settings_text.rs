//! The text field a Settings sheet is typing into, for copy, cut and paste:
//! the connect sheet's, or the local models' id. Secret fields are never
//! copied out.

use superfik_ui::TextEdit;

use super::Session;

#[derive(Debug, Clone, Copy)]
pub struct SettingsText<'a> {
    pub edit: &'a TextEdit,
    pub secret: bool,
}

impl Session {
    pub fn settings_text(&self) -> Option<SettingsText<'_>> {
        if !self.settings.open {
            return None;
        }
        if self.settings.pane == super::SettingsPane::Ai && self.plugins.panel.local_models {
            return (self.plugins.panel.local_focus == 0)
                .then_some(SettingsText { edit: &self.plugins.local_model, secret: false });
        }
        let sheet = self.plugins.panel.sheet.as_ref()?;
        match sheet.focus {
            0 => Some(SettingsText { edit: &sheet.label, secret: false }),
            n => sheet.fields.get(n - 1).map(|edit| SettingsText { edit, secret: true }),
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::PluginId;

    use super::*;
    use crate::state::{Action, ConnectSheet, EditKey};

    fn connecting() -> Session {
        let mut session = Session::default();
        session.apply(Action::OpenPlugins, 0.0);
        let sheet = ConnectSheet {
            plugin: PluginId("mail".into()),
            label: TextEdit::default(),
            fields: vec![TextEdit::default()],
            focus: 0,
        };
        session.plugins.panel.sheet = Some(sheet);
        session
    }

    #[test]
    fn the_account_name_copies_and_a_secret_field_says_it_is_secret() {
        let mut session = connecting();
        session.apply(Action::Edit(EditKey::Insert("Work".into())), 0.0);
        let text = session.settings_text().expect("the name field is focused");
        assert_eq!((text.edit.text(), text.secret), ("Work", false));
        if let Some(sheet) = session.plugins.panel.sheet.as_mut() {
            sheet.focus = 1;
        }
        assert!(session.settings_text().is_some_and(|text| text.secret));
    }

    #[test]
    fn nothing_is_focused_once_settings_close() {
        let mut session = connecting();
        session.apply(Action::CloseSettings, 0.0);
        assert!(session.settings_text().is_none());
    }
}
