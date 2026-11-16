use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use postgres::{Client, Config, NoTls};

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
}

impl Database for PostgresDatabase {
    fn health(&self) -> Health {
        let mut slot = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = slot.as_mut()
            && client.is_valid(CONNECT_TIMEOUT).is_ok()
        {
            return Health::Ready;
        }
        match self.connect() {
            Ok(client) => {
                *slot = Some(client);
                Health::Ready
            }
            Err(reason) => {
                *slot = None;
                Health::Broken(reason)
            }
        }
    }
}
