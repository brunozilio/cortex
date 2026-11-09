//! The organization's policy (`superfik_core::policy`) and what members
//! spend on models, kept by its server. Admins set the policy and see
//! everyone's spending; each member reports their runs' cost and hears
//! their own total for the month.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::access::administers;
use superfik_core::policy::{Policy, month_of};
use superfik_core::{Principal, UserId};
use superfik_protocol::{Addressed, ClientCommand, MemberUsage, Recipient, ServerEvent};

use crate::Organization;

pub type MonthUsage = HashMap<UserId, (f64, u32)>;

pub trait Governance: Send + Sync {
    fn policy(&self) -> Policy;
    fn set_policy(&self, policy: &Policy) -> std::io::Result<()>;
    fn usage(&self, month: &str) -> MonthUsage;
    fn add_usage(&self, month: &str, user: UserId, cost_usd: f64) -> std::io::Result<f64>;
}

#[derive(Debug)]
pub struct FileGovernance {
    root: PathBuf,
    writing: Mutex<()>,
}

impl FileGovernance {
    pub fn new(root: PathBuf) -> Self {
        Self { root, writing: Mutex::new(()) }
    }

    fn write(&self, file: PathBuf, json: Vec<u8>) -> std::io::Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = file.with_extension("tmp");
        std::fs::write(&temporary, json)?;
        std::fs::rename(temporary, file)
    }

    fn usage_file(&self, month: &str) -> PathBuf {
        self.root.join("usage").join(format!("{month}.json"))
    }
}

fn read<T: serde::de::DeserializeOwned + Default>(file: &std::path::Path) -> T {
    std::fs::read(file).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default()
}

impl Governance for FileGovernance {
    fn policy(&self) -> Policy {
        read(&self.root.join("policy.json"))
    }

    fn set_policy(&self, policy: &Policy) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let json = serde_json::to_vec_pretty(policy).map_err(std::io::Error::other)?;
        self.write(self.root.join("policy.json"), json)
    }

    fn usage(&self, month: &str) -> MonthUsage {
        read(&self.usage_file(month))
    }

    fn add_usage(&self, month: &str, user: UserId, cost_usd: f64) -> std::io::Result<f64> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut usage: MonthUsage = read(&self.usage_file(month));
        let entry = usage.entry(user).or_default();
        *entry = (entry.0 + cost_usd, entry.1 + 1);
        let total = entry.0;
        self.write(self.usage_file(month), serde_json::to_vec_pretty(&usage).map_err(std::io::Error::other)?)?;
        Ok(total)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryGovernance {
    policy: Mutex<Policy>,
    usage: Mutex<BTreeMap<String, MonthUsage>>,
}

impl Governance for InMemoryGovernance {
    fn policy(&self) -> Policy {
        self.policy.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn set_policy(&self, policy: &Policy) -> std::io::Result<()> {
        *self.policy.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = policy.clone();
        Ok(())
    }

    fn usage(&self, month: &str) -> MonthUsage {
        let usage = self.usage.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        usage.get(month).cloned().unwrap_or_default()
    }

    fn add_usage(&self, month: &str, user: UserId, cost_usd: f64) -> std::io::Result<f64> {
        let mut usage = self.usage.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = usage.entry(month.to_owned()).or_default().entry(user).or_default();
        *entry = (entry.0 + cost_usd, entry.1 + 1);
        Ok(entry.0)
    }
}

pub fn answer(
    org: &Organization,
    store: &dyn Governance,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = |why: &str| Some(vec![to_sender(ServerEvent::error(why))]);
    if !matches!(
        command,
        ClientCommand::GetPolicy
            | ClientCommand::SetPolicy { .. }
            | ClientCommand::ReportUsage { .. }
            | ClientCommand::ListUsage
    ) {
        return None;
    }
    let Principal::Member(member) = by else { return refused("not allowed") };
    let Some(role) = org.directory.role(member) else { return refused("not allowed") };
    let month = month_of(i64::try_from(org.now()).unwrap_or(i64::MAX));
    let spent = |usd| to_sender(ServerEvent::Spent { month: month.clone(), usd });
    match command {
        ClientCommand::GetPolicy => {
            let mine = store.usage(&month).get(&member.user).map_or(0.0, |(usd, _)| *usd);
            Some(vec![to_sender(ServerEvent::Policy(store.policy())), spent(mine)])
        }
        ClientCommand::SetPolicy { policy } => {
            if !administers(role) {
                return refused("only owners and admins set the policy");
            }
            if let Err(error) = store.set_policy(policy) {
                return refused(&format!("could not keep the policy: {error}"));
            }
            Some(org.to_readers(&ServerEvent::Policy(policy.clone())))
        }
        ClientCommand::ReportUsage { cost_usd, .. } => {
            if !cost_usd.is_finite() || *cost_usd < 0.0 {
                return Some(Vec::new());
            }
            match store.add_usage(&month, member.user, *cost_usd) {
                Ok(total) => Some(vec![spent(total)]),
                Err(error) => refused(&format!("could not keep the usage: {error}")),
            }
        }
        _ => {
            if !administers(role) {
                return refused("only owners and admins see everyone's usage");
            }
            Some(vec![to_sender(ServerEvent::Usage(usage_of(org, &month, store.usage(&month))))])
        }
    }
}

fn usage_of(org: &Organization, month: &str, usage: MonthUsage) -> Vec<MemberUsage> {
    org.directory
        .members()
        .unwrap_or_default()
        .into_iter()
        .map(|account| {
            let (cost_usd, runs) = usage.get(&account.user).copied().unwrap_or_default();
            MemberUsage { user: account.user, name: account.name, month: month.to_owned(), cost_usd, runs }
        })
        .collect()
}
