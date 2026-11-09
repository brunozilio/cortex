//! Settings › Server (ADR 0017): connecting the app to a team's server,
//! and, for its owners and admins, running it.
//!
//! - **Connecting.** `Join` asks the shell (`Effect::JoinServer`) to sign
//!   in with a password or a first-access code, keep the token and swap the
//!   app over to that server; it answers `Joined`. `Leave` goes back to
//!   this computer's own server (`Effect::LeaveServer`). Plain HTTP to
//!   another computer waits for `AllowInsecure` (`server_urls`).
//! - **Running it.** The domain (`SaveServerSettings`, holding at once)
//!   and the users.

use superfik_core::access::OrgRole;
use superfik_protocol::{ClientCommand, ServerSettings};
use superfik_ui::TextEdit;

use super::{Effect, Session};

mod action;
mod address;
mod users;

pub use action::{JoinRequest, Joined, ServerAction, ServerField};
use address::bare_remote;
pub use address::{AddressProblem, server_urls};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerLink {
    pub address: TextEdit,
    pub name: TextEdit,
    pub code: TextEdit,
    pub password: TextEdit,
    pub domain: TextEdit,
    pub new_user: TextEdit,
    pub new_role: Option<OrgRole>,
    pub focus: Option<ServerField>,
    pub first_access: bool,
    pub joining: bool,
    pub said: Option<String>,
    pub problem: Option<AddressProblem>,
    pub insecure_allowed: Option<String>,
    pub http_hint: bool,
    pub joined: Option<Joined>,
    pub settings: Option<ServerSettings>,
    pub code_shown: Option<(String, String)>,
}

impl ServerLink {
    pub fn edit(&self, field: ServerField) -> &TextEdit {
        match field {
            ServerField::Address => &self.address,
            ServerField::Name => &self.name,
            ServerField::Code => &self.code,
            ServerField::Password => &self.password,
            ServerField::Domain => &self.domain,
            ServerField::NewUser => &self.new_user,
        }
    }

    fn edit_mut(&mut self, field: ServerField) -> &mut TextEdit {
        match field {
            ServerField::Address => &mut self.address,
            ServerField::Name => &mut self.name,
            ServerField::Code => &mut self.code,
            ServerField::Password => &mut self.password,
            ServerField::Domain => &mut self.domain,
            ServerField::NewUser => &mut self.new_user,
        }
    }

    fn card_of(&self, field: ServerField) -> Vec<ServerField> {
        use ServerField::*;
        match field {
            Address | Name | Code | Password if self.first_access => vec![Address, Name, Code, Password],
            Address | Name | Code | Password => vec![Address, Name, Password],
            Domain => vec![Domain],
            NewUser => vec![NewUser],
        }
    }

    fn request(&self) -> Option<JoinRequest> {
        let filled = |edit: &TextEdit| Some(edit.text().trim().to_owned()).filter(|text| !text.is_empty());
        let address = filled(&self.address)?;
        Some(JoinRequest {
            insecure: self.insecure_allowed.as_ref() == Some(&address),
            address,
            name: filled(&self.name)?,
            password: Some(self.password.text().to_owned()).filter(|text| !text.is_empty())?,
            code: if self.first_access { Some(filled(&self.code)?) } else { None },
        })
    }

    fn join(&mut self) -> Vec<Effect> {
        if self.joining {
            return Vec::new();
        }
        let Some(request) = self.request() else { return Vec::new() };
        self.problem = server_urls(&request.address, request.insecure).err();
        self.http_hint = false;
        if self.problem.is_some() {
            return Vec::new();
        }
        self.joining = true;
        self.said = None;
        self.focus = None;
        vec![Effect::JoinServer(request)]
    }
}

impl Session {
    pub fn administers_server(&self) -> bool {
        self.remote && self.manages_organization()
    }

    pub(super) fn server_action(&mut self, action: ServerAction) -> Vec<Effect> {
        let link = &mut self.server_link;
        match action {
            ServerAction::Focus(field) => link.focus = Some(field),
            ServerAction::Edit(key) => {
                if let Some(field) = link.focus {
                    super::composer::apply_key(link.edit_mut(field), key);
                }
            }
            ServerAction::Step(forward) => {
                if let Some(field) = link.focus {
                    let card = link.card_of(field);
                    let at = card.iter().position(|f| *f == field).unwrap_or(0);
                    let next = if forward { at + 1 } else { at + card.len() - 1 } % card.len();
                    link.focus = card.get(next).copied();
                }
            }
            ServerAction::Blur => link.focus = None,
            ServerAction::ToggleFirstAccess => {
                link.first_access = !link.first_access;
                link.said = None;
                if link.focus == Some(ServerField::Code) {
                    link.focus = Some(ServerField::Password);
                }
            }
            ServerAction::Submit => {
                return match link.focus {
                    Some(ServerField::Domain) => self.save_server_settings(),
                    Some(ServerField::NewUser) => self.server_action(ServerAction::AddUser),
                    Some(_) => self.server_action(ServerAction::Join),
                    None => Vec::new(),
                };
            }
            ServerAction::Join => return link.join(),
            ServerAction::AllowInsecure => {
                if link.problem != Some(AddressProblem::Insecure) {
                    return Vec::new();
                }
                link.insecure_allowed = Some(link.address.text().trim().to_owned());
                return link.join();
            }
            ServerAction::Joined(Ok(joined)) => {
                link.joining = false;
                link.first_access = false;
                link.said = None;
                link.http_hint = false;
                link.code = TextEdit::default();
                link.password = TextEdit::default();
                link.joined = Some(joined);
            }
            ServerAction::Joined(Err(why)) => {
                link.joining = false;
                link.http_hint = bare_remote(link.address.text());
                link.said = Some(why);
            }
            ServerAction::Leave => {
                link.joining = false;
                link.joined = None;
                link.settings = None;
                link.code_shown = None;
                return vec![Effect::LeaveServer];
            }
            ServerAction::Left => {
                link.joined = None;
                link.settings = None;
            }
            ServerAction::AddUser => {
                let name = link.new_user.text().trim().to_owned();
                if name.is_empty() {
                    return Vec::new();
                }
                let role = link.new_role.unwrap_or(OrgRole::Member);
                link.new_user = TextEdit::default();
                return command(ClientCommand::AddUser { name, role });
            }
            ServerAction::NewUserRole(role) => link.new_role = Some(role),
            ServerAction::NewCode(user) => return self.new_code(user),
            ServerAction::ToggleAdmin(user) => return self.toggle_admin(user),
            ServerAction::Remove(user) => return self.remove_user(user),
            ServerAction::CopyCode => {
                return link.code_shown.as_ref().map(|(_, code)| Effect::CopyText(code.clone())).into_iter().collect();
            }
            ServerAction::DismissCode => link.code_shown = None,
            ServerAction::SaveSettings => return self.save_server_settings(),
        }
        Vec::new()
    }

    fn save_server_settings(&mut self) -> Vec<Effect> {
        let link = &mut self.server_link;
        let domain = link.domain.text().trim().to_owned();
        link.focus = None;
        command(ClientCommand::SaveServerSettings { domain })
    }

    pub(super) fn server_pane_open(&self) -> bool {
        self.settings.open && self.settings.pane == super::SettingsPane::Server
    }

    pub(super) fn server_settings_arrived(&mut self, settings: ServerSettings) {
        if self.server_link.focus != Some(ServerField::Domain) {
            self.server_link.domain = TextEdit::new(settings.domain.as_deref().unwrap_or_default());
        }
        self.server_link.settings = Some(settings);
    }

    pub(super) fn first_access_code(&mut self, name: String, code: String) {
        self.server_link.code_shown = Some((name, code));
    }

    pub(super) fn server_pane_shown(&self) -> Vec<Effect> {
        if self.administers_server() { command(ClientCommand::LoadServerSettings) } else { Vec::new() }
    }
}

fn command(command: ClientCommand) -> Vec<Effect> {
    vec![Effect::Command(Box::new(command))]
}

#[cfg(test)]
mod tests;
