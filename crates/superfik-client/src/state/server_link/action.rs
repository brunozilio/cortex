//! What the Server pane is written with and sends: its fields, the
//! request the shell signs in with, and the actions the reducer takes.

use superfik_core::UserId;
use superfik_core::access::OrgRole;

use crate::state::EditKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerField {
    Address,
    Name,
    Code,
    Password,
    Domain,
    NewUser,
}

impl ServerField {
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Address => "superfik.acme.io",
            Self::Name => "ana",
            Self::Code => "XXXX-XXXX-XXXX",
            Self::Password => "••••••••••",
            Self::Domain => "superfik.acme.io",
            Self::NewUser => "username",
        }
    }

    pub fn secret(self) -> bool {
        matches!(self, Self::Password | Self::Code)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct JoinRequest {
    pub address: String,
    pub name: String,
    pub password: String,
    pub code: Option<String>,
    pub insecure: bool,
}

impl std::fmt::Debug for JoinRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinRequest")
            .field("address", &self.address)
            .field("name", &self.name)
            .field("first_access", &self.code.is_some())
            .field("insecure", &self.insecure)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub address: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerAction {
    Focus(ServerField),
    Edit(EditKey),
    Submit,
    Step(bool),
    Blur,
    ToggleFirstAccess,
    Join,
    AllowInsecure,
    Leave,
    Joined(Result<Joined, String>),
    Left,
    AddUser,
    NewUserRole(OrgRole),
    NewCode(UserId),
    ToggleAdmin(UserId),
    Remove(UserId),
    CopyCode,
    DismissCode,
    SaveSettings,
}
