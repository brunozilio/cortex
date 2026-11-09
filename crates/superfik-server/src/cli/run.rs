//! Running the commands `parse` read, over the data folder: users,
//! settings, and the organization `serve` runs.

use std::path::Path;
use std::sync::Arc;

use superfik_core::access::OrgRole;
use superfik_identity::{Actor, Directory, FileAccounts};

use super::{Command, USAGE};
use crate::config::Config;
use crate::settings::{SettingsFile, given};

fn role_name(role: OrgRole) -> &'static str {
    match role {
        OrgRole::Owner => "owner",
        OrgRole::Admin => "admin",
        OrgRole::Member => "member",
        OrgRole::Guest => "guest",
    }
}

type Failure = Box<dyn std::error::Error>;

fn directory(config: &Config) -> Result<Directory<FileAccounts>, Failure> {
    config.prepare()?;
    let org = SettingsFile::open(&config.settings_path())?.read().org;
    Ok(Directory::new(org, FileAccounts::open(&config.accounts_path())?))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

fn code_said(name: &str, code: &str) -> String {
    format!(
        "First-access code for {name}: {code}\n\
         It works once, for 7 days: in Superfik, Settings › Server, they enter the server's\n\
         address, \"{name}\" and this code, and choose their password."
    )
}

pub fn run(command: &Command, config: &Config) -> Result<String, Failure> {
    let actor = Actor::Console;
    Ok(match command {
        Command::Help => USAGE.to_owned(),
        Command::Version => format!("superfik-server {}", env!("CARGO_PKG_VERSION")),
        Command::Init { owner } => {
            let directory = directory(config)?;
            if directory.has_owner()? {
                format!("{} is set up already; add users with `superfik-server user add`.", config.data.display())
            } else {
                let (_, code) = directory.add(actor, owner, OrgRole::Owner, now())?;
                format!("Set up {}.\n{}", config.data.display(), code_said(owner, &code))
            }
        }
        Command::Users => {
            let members = directory(config)?.members()?;
            if members.is_empty() {
                "No users yet: `superfik-server init` adds the first owner.".to_owned()
            } else {
                members
                    .iter()
                    .map(|account| {
                        let pending = if account.pending() { "  (first access pending)" } else { "" };
                        format!("{:<24} {}{pending}", account.name, role_name(account.role))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Command::AddUser { name, role } => {
            let (_, code) = directory(config)?.add(actor, name, *role, now())?;
            code_said(name, &code)
        }
        Command::NewCode { name } => {
            let directory = directory(config)?;
            let user = directory.named(name)?.user;
            code_said(name, &directory.new_code(actor, user, now())?)
        }
        Command::SetRole { name, role } => {
            let directory = directory(config)?;
            directory.set_role(actor, directory.named(name)?.user, *role)?;
            format!("{name} is now {}.", role_name(*role))
        }
        Command::RemoveUser { name } => {
            let directory = directory(config)?;
            directory.remove(actor, directory.named(name)?.user)?;
            format!("Removed {name}.")
        }
        Command::ShowConfig | Command::Domain(_) => {
            config.prepare()?;
            let mut settings = SettingsFile::open(&config.settings_path())?;
            let file = match command {
                Command::Domain(domain) => settings.change(|file| file.domain = domain.as_deref().and_then(given))?,
                _ => settings.read(),
            };
            format!("Data:   {}\nDomain: {}", config.data.display(), file.domain.as_deref().unwrap_or("(none)"))
        }
        Command::Serve | Command::Install { .. } => String::new(),
    })
}

pub fn as_data_owner(me: Option<u32>, owner: Option<u32>, data: &Path) -> Result<(), String> {
    match (me, owner) {
        (Some(me), Some(owner)) if me != owner => Err(format!(
            "{} belongs to another user (uid {owner}); run this as that user: \
             sudo -u '#{owner}' superfik-server …",
            data.display()
        )),
        _ => Ok(()),
    }
}

pub fn organization(config: &Config, clock: crate::Clock) -> Result<crate::Organization, Failure> {
    config.prepare()?;
    let accounts: Arc<dyn superfik_identity::AccountStore> = Arc::new(FileAccounts::open(&config.accounts_path())?);
    let settings = SettingsFile::open(&config.settings_path())?;
    let teams = Arc::new(crate::teams::FileTeams::new(config.data.join("teams.json")));
    Ok(crate::Organization::with_settings(accounts, settings, clock).with_teams(teams))
}
