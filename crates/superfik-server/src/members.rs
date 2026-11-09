//! User and server-settings commands, answered by the organization's
//! server itself (the command handler knows nothing of accounts): the
//! list of users, adding one with a first-access code, a new code, roles,
//! removal, and the domain in Settings › Server.

use superfik_core::access::{Colleague, administers};
use superfik_core::{Member, Principal};
use superfik_identity::{Account, Actor, IdentityError};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;
use crate::settings::given;

pub fn answer(org: &Organization, by: &Principal, command: &ClientCommand) -> Option<Vec<Addressed>> {
    let handled = matches!(
        command,
        ClientCommand::ListMembers
            | ClientCommand::AddUser { .. }
            | ClientCommand::NewFirstAccessCode { .. }
            | ClientCommand::RemoveUser { .. }
            | ClientCommand::SetUserRole { .. }
            | ClientCommand::LoadServerSettings
            | ClientCommand::SaveServerSettings { .. }
    );
    if !handled {
        return None;
    }
    let to_sender = |event| vec![Addressed { to: Recipient::Principal(by.clone()), event }];
    let Principal::Member(member) = by else { return Some(to_sender(ServerEvent::error("not allowed"))) };
    let Some(role) = org.directory.role(member) else {
        return Some(to_sender(ServerEvent::error(IdentityError::Forbidden.to_string())));
    };
    let actor = Actor::Member(*member);
    let now = org.now();
    let outcome = match command {
        ClientCommand::ListMembers => return Some(to_sender(listed(org))),
        ClientCommand::LoadServerSettings | ClientCommand::SaveServerSettings { .. } if !administers(role) => {
            Err(IdentityError::Forbidden.to_string())
        }
        ClientCommand::LoadServerSettings => Ok(vec![ServerEvent::ServerSettings(org.settings().shown())]),
        ClientCommand::SaveServerSettings { domain } => org
            .change_settings(|file| file.domain = given(domain))
            .map(|file| vec![ServerEvent::ServerSettings(file.shown())])
            .map_err(|error| error.to_string()),
        ClientCommand::AddUser { name, role } => org
            .directory
            .add(actor, name, *role, now)
            .map(|(added, code)| vec![code_for(org, added, code)])
            .map_err(|error| error.to_string()),
        ClientCommand::NewFirstAccessCode { user } => org
            .directory
            .new_code(actor, *user, now)
            .map(|code| vec![code_for(org, Member { user: *user, org: org.directory.org() }, code)])
            .map_err(|error| error.to_string()),
        ClientCommand::RemoveUser { user } => {
            org.directory.remove(actor, *user).map(|()| Vec::new()).map_err(|error| error.to_string())
        }
        ClientCommand::SetUserRole { user, role } => {
            org.directory.set_role(actor, *user, *role).map(|()| Vec::new()).map_err(|error| error.to_string())
        }
        _ => return None,
    };
    Some(match outcome {
        Ok(events) => {
            let mut answers: Vec<Addressed> = events.into_iter().flat_map(to_sender).collect();
            if !matches!(command, ClientCommand::LoadServerSettings | ClientCommand::SaveServerSettings { .. }) {
                answers.extend(org.to_readers(&listed(org)));
            }
            answers
        }
        Err(message) => to_sender(ServerEvent::error(message)),
    })
}

fn code_for(org: &Organization, member: Member, code: String) -> ServerEvent {
    let name = org.directory.role(&member).and(org.directory.members().ok()).and_then(|members| {
        members.into_iter().find(|account| account.user == member.user).map(|account| account.name)
    });
    ServerEvent::FirstAccessCode { user: member.user, name: name.unwrap_or_default(), code }
}

fn listed(org: &Organization) -> ServerEvent {
    match org.directory.members() {
        Ok(members) => ServerEvent::Members(members.into_iter().map(colleague).collect()),
        Err(error) => ServerEvent::error(error.to_string()),
    }
}

fn colleague(account: Account) -> Colleague {
    Colleague { pending: account.pending(), user: account.user, name: account.name, role: account.role }
}
