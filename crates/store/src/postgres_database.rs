use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use postgres::types::Json;
use postgres::{Client, Config, NoTls};
use serde_json::Value;

use crate::{Database, Health};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

pub struct PostgresDatabase {
    url: String,
    client: Mutex<Option<Client>>,
}

impl std::fmt::Debug for PostgresDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresDatabase").finish_non_exhaustive()
    }
}

impl PostgresDatabase {
    pub fn new(url: String) -> Self {
        Self { url, client: Mutex::new(None) }
    }

    fn connect(&self) -> Result<Client, String> {
        let mut config: Config = self.url.parse().map_err(|error: postgres::Error| error.to_string())?;
        config.connect_timeout(CONNECT_TIMEOUT);
        let mut client = config.connect(NoTls).map_err(|error| error.to_string())?;
        crate::migrations::apply(&mut client).map_err(|error| error.to_string())?;
        Ok(client)
    }

    fn with<T>(&self, work: impl FnOnce(&mut Client) -> Result<T, postgres::Error>) -> Result<T, String> {
        let mut slot = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        let usable = slot.as_mut().is_some_and(|client| client.is_valid(CONNECT_TIMEOUT).is_ok());
        if !usable {
            *slot = Some(self.connect()?);
        }
        let client = slot.as_mut().ok_or("no database connection")?;
        work(client).map_err(|error| error.to_string())
    }
}

impl Database for PostgresDatabase {
    fn health(&self) -> Health {
        match self.with(|_| Ok(())) {
            Ok(()) => Health::Ready,
            Err(reason) => Health::Broken(reason),
        }
    }

    fn preference(&self, key: &str) -> Result<Option<Value>, String> {
        self.with(|client| {
            let row = client.query_opt("select value from preferences where key = $1", &[&key])?;
            Ok(row.map(|row| row.get::<_, Json<Value>>(0).0))
        })
    }

    fn keep_preference(&self, key: &str, value: &Value) -> Result<(), String> {
        self.with(|client| {
            client.execute(
                "insert into preferences (key, value) values ($1, $2) \
                 on conflict (key) do update set value = excluded.value, updated_at = now()",
                &[&key, &Json(value)],
            )?;
            Ok(())
        })
    }
}
