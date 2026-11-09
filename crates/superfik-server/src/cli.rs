//! The `superfik-server` command line: run the server, set it up, manage
//! its users and settings, and install it as a service. Parsing and every
//! command but `serve` are here, over the data folder, so they are tested
//! without a process; `main` only runs what they return.

use superfik_core::access::OrgRole;

mod run;

pub use run::{as_data_owner, organization, run};

pub const USAGE: &str = "\
superfik-server: the Superfik server for a team.

Usage:
  superfik-server [serve]                 Run the server.
  superfik-server init [--owner NAME]     Set up the data folder and its first owner
                                          (\"admin\" unless named); prints their code.
  superfik-server user list               List the users.
  superfik-server user add NAME [--role owner|admin|member|guest]
                                          Add a user (member by default); prints the
                                          first-access code they sign in with once.
  superfik-server user code NAME          A new first-access code (a forgotten password:
                                          their password and sign-ins end).
  superfik-server user role NAME ROLE     Change a user's role.
  superfik-server user remove NAME        Remove a user.
  superfik-server config                  Show the data folder and the domain.
  superfik-server config domain DOMAIN    Where users reach the server (\"-\" clears it).
  superfik-server install [--domain DOMAIN] [--user USER]
                                          Linux, as root, from the release archive: the
                                          systemd service (as USER, \"superfik\" unless
                                          named), agents' tools as superfik-runner, and a
                                          Caddy site for DOMAIN with HTTPS; then (re)start it.

Run the other commands as the data folder's owner (sudo -u superfik superfik-server …).
  superfik-server --version

Environment: SUPERFIK_DATA (data folder), SUPERFIK_LISTEN (address), and the others
in docs/self-hosting.md.
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Help,
    Version,
    Init { owner: String },
    Users,
    AddUser { name: String, role: OrgRole },
    NewCode { name: String },
    SetRole { name: String, role: OrgRole },
    RemoveUser { name: String },
    ShowConfig,
    Domain(Option<String>),
    Install { domain: Option<String>, user: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}\n\n{USAGE}")]
pub struct Usage(String);

pub fn parse(args: &[String]) -> Result<Command, Usage> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let cleared = |value: &str| Some(value.to_owned()).filter(|value| value != "-");
    Ok(match words.as_slice() {
        [] | ["serve"] => Command::Serve,
        ["-h" | "--help" | "help", ..] => Command::Help,
        ["-V" | "--version" | "version"] => Command::Version,
        ["init", rest @ ..] => {
            let mut options = options(rest, &["--owner"])?;
            Command::Init { owner: options.remove("--owner").unwrap_or_else(|| "admin".to_owned()) }
        }
        ["user", "list"] | ["users"] => Command::Users,
        ["user", "add", name, rest @ ..] => {
            let role = options(rest, &["--role"])?.remove("--role");
            Command::AddUser {
                name: (*name).to_owned(),
                role: role.map_or(Ok(OrgRole::Member), |role| role_named(&role))?,
            }
        }
        ["user", "code", name] => Command::NewCode { name: (*name).to_owned() },
        ["user", "role", name, role] => Command::SetRole { name: (*name).to_owned(), role: role_named(role)? },
        ["user", "remove", name] => Command::RemoveUser { name: (*name).to_owned() },
        ["config"] => Command::ShowConfig,
        ["config", "domain", domain] => Command::Domain(cleared(domain).map(|domain| host(&domain)).transpose()?),
        ["install", rest @ ..] => {
            let mut options = options(rest, &["--domain", "--user"])?;
            let domain = options.remove("--domain").map(|domain| host(&domain)).transpose()?;
            let user = options.remove("--user").unwrap_or_else(|| "superfik".to_owned());
            let user = crate::install::service_user(&user).ok_or_else(|| {
                Usage(format!("--user {user:?} is not a system user name (nor root, nor the runner)"))
            })?;
            Command::Install { domain, user }
        }
        _ => return Err(Usage(format!("unknown command: {}", words.join(" ")))),
    })
}

fn options(words: &[&str], allowed: &[&str]) -> Result<std::collections::BTreeMap<String, String>, Usage> {
    let mut found = std::collections::BTreeMap::new();
    let mut words = words.iter();
    while let Some(word) = words.next() {
        if !allowed.contains(word) {
            return Err(Usage(format!("unexpected {word:?}")));
        }
        let value = words.next().filter(|value| !value.starts_with("--"));
        let value = value.ok_or_else(|| Usage(format!("{word} needs a value")))?;
        if found.insert((*word).to_owned(), (*value).to_owned()).is_some() {
            return Err(Usage(format!("{word} is given twice")));
        }
    }
    Ok(found)
}

fn host(domain: &str) -> Result<String, Usage> {
    crate::install::domain_name(domain)
        .ok_or_else(|| Usage(format!("{domain:?} is not a domain name like superfik.example.com")))
}

fn role_named(role: &str) -> Result<OrgRole, Usage> {
    match role.to_lowercase().as_str() {
        "owner" => Ok(OrgRole::Owner),
        "admin" => Ok(OrgRole::Admin),
        "member" => Ok(OrgRole::Member),
        "guest" => Ok(OrgRole::Guest),
        other => Err(Usage(format!("no role called {other}"))),
    }
}

#[cfg(test)]
mod tests;
