//! Accounts kept in one JSON file (`0600`), replaced atomically on every
//! change: enough for a self-hosted server's users, with no database. The
//! server's command line (`superfik-server user add`) writes the same file
//! while the server runs, so a read takes the file again whenever it
//! changed since it was last read (its size, time or the file itself), and
//! every change holds the file's lock (`accounts.lock`, beside it) from
//! reading to replacing, so neither loses the other's.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use superfik_core::{OrgId, UserId};

use crate::store::{Account, AccountStore, StoreError, Tables, Token};

#[derive(Debug)]
pub struct FileAccounts {
    path: PathBuf,
    kept: Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    tables: Tables,
    stamp: Option<Stamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    inode: u64,
}

impl Stamp {
    fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(&meta);
        #[cfg(not(unix))]
        let inode = 0;
        Some(Self { len: meta.len(), modified: meta.modified().ok(), inode })
    }
}

fn failed(error: impl ToString) -> StoreError {
    StoreError(error.to_string())
}

impl FileAccounts {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let store = Self { path: path.to_path_buf(), kept: Mutex::default() };
        drop(store.fresh()?);
        Ok(store)
    }

    fn fresh(&self) -> Result<MutexGuard<'_, Kept>, StoreError> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        let stamp = Stamp::of(&self.path);
        if stamp.is_some() && stamp != kept.stamp {
            let json = std::fs::read_to_string(&self.path).map_err(failed)?;
            kept.tables = serde_json::from_str(&json).map_err(failed)?;
            kept.stamp = stamp;
        }
        Ok(kept)
    }

    fn read<T>(&self, look: impl FnOnce(&Tables) -> T) -> Result<T, StoreError> {
        Ok(look(&self.fresh()?.tables))
    }

    fn write(&self, change: impl FnOnce(&mut Tables)) -> Result<(), StoreError> {
        let lock = superfik_private_file::open(
            std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false),
            &self.path.with_extension("lock"),
        )
        .map_err(failed)?;
        lock.lock().map_err(failed)?;
        let mut kept = self.fresh()?;
        change(&mut kept.tables);
        let json = serde_json::to_string_pretty(&kept.tables).map_err(failed)?;
        superfik_private_file::replace(&self.path, json.as_bytes()).map_err(failed)?;
        kept.stamp = Stamp::of(&self.path);
        drop(lock);
        Ok(())
    }
}

impl AccountStore for FileAccounts {
    fn account(&self, user: UserId) -> Result<Option<Account>, StoreError> {
        self.read(|tables| tables.account(user))
    }
    fn account_named(&self, org: OrgId, name: &str) -> Result<Option<Account>, StoreError> {
        self.read(|tables| tables.account_named(org, name))
    }
    fn accounts(&self, org: OrgId) -> Result<Vec<Account>, StoreError> {
        self.read(|tables| tables.accounts(org))
    }
    fn put_account(&self, account: Account) -> Result<(), StoreError> {
        self.write(|tables| tables.put_account(account))
    }
    fn put_token(&self, hash: String, token: Token) -> Result<(), StoreError> {
        self.write(|tables| {
            tables.tokens.insert(hash, token);
        })
    }
    fn token(&self, hash: &str) -> Result<Option<Token>, StoreError> {
        self.read(|tables| tables.tokens.get(hash).copied())
    }
    fn drop_token(&self, hash: &str) -> Result<(), StoreError> {
        self.write(|tables| {
            tables.tokens.remove(hash);
        })
    }
    fn revoke_tokens(&self, user: UserId) -> Result<(), StoreError> {
        self.write(|tables| tables.revoke_tokens(user))
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::access::OrgRole;

    use super::*;

    fn olga(org: OrgId) -> Account {
        Account {
            user: UserId::new(),
            org,
            name: "olga".into(),
            role: OrgRole::Owner,
            password: Some("hash".into()),
            first_access: None,
            active: true,
        }
    }

    #[test]
    fn accounts_survive_reopening_and_the_file_is_private() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("accounts.json");
        let org = OrgId::new();
        let account = olga(org);
        FileAccounts::open(&path).expect("open").put_account(account.clone()).expect("put");
        assert_eq!(FileAccounts::open(&path).expect("reopen").accounts(org).expect("read"), [account]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn another_process_writing_the_file_is_seen() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("accounts.json");
        let org = OrgId::new();
        let server = FileAccounts::open(&path).expect("server");
        assert!(server.accounts(org).expect("read").is_empty());
        std::thread::sleep(std::time::Duration::from_millis(20));
        let account = olga(org);
        FileAccounts::open(&path).expect("command line").put_account(account.clone()).expect("put");
        assert_eq!(server.account_named(org, "olga").expect("read"), Some(account));
    }

    #[test]
    fn writers_at_once_lose_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("accounts.json");
        let org = OrgId::new();
        let writers: Vec<_> = (0..4)
            .map(|writer| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let store = FileAccounts::open(&path).expect("open");
                    for n in 0..10 {
                        store.put_account(Account { name: format!("u{writer}-{n}"), ..olga(org) }).expect("put");
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().expect("writer");
        }
        assert_eq!(FileAccounts::open(&path).expect("open").accounts(org).expect("read").len(), 40);
    }

    #[test]
    fn a_change_within_the_same_time_is_seen() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("accounts.json");
        let org = OrgId::new();
        let server = FileAccounts::open(&path).expect("server");
        server.put_account(olga(org)).expect("put");
        let before = std::fs::metadata(&path).and_then(|meta| meta.modified()).expect("time");
        let ana = Account { name: "ana".into(), ..olga(org) };
        FileAccounts::open(&path).expect("command line").put_account(ana.clone()).expect("put");
        std::fs::File::options().write(true).open(&path).and_then(|file| file.set_modified(before)).expect("touch");
        assert_eq!(server.account_named(org, "ana").expect("read"), Some(ana));
    }
}
