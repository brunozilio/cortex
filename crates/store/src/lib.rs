mod memory;
mod migrations;
mod postgres_database;

use std::sync::Arc;

use serde_json::Value;

pub use memory::Memory;
pub use migrations::MIGRATIONS;
pub use postgres_database::PostgresDatabase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Ready,
    Missing,
    Broken(String),
}

impl Health {
    pub fn label(&self) -> &'static str {
        match self {
            Health::Ready => "ready",
            Health::Missing => "missing",
            Health::Broken(_) => "broken",
        }
    }
}

pub trait Database: Send + Sync {
    fn health(&self) -> Health;
    fn preference(&self, key: &str) -> Result<Option<Value>, String>;
    fn keep_preference(&self, key: &str, value: &Value) -> Result<(), String>;
}

const NO_DATABASE: &str = "the console has no database: run `docker compose up -d` and set DATABASE_URL";

#[derive(Debug, Default, Clone, Copy)]
pub struct Unconfigured;

impl Database for Unconfigured {
    fn health(&self) -> Health {
        Health::Missing
    }

    fn preference(&self, _key: &str) -> Result<Option<Value>, String> {
        Err(NO_DATABASE.into())
    }

    fn keep_preference(&self, _key: &str, _value: &Value) -> Result<(), String> {
        Err(NO_DATABASE.into())
    }
}

pub fn open(url: Option<String>) -> Arc<dyn Database> {
    match url.filter(|url| !url.trim().is_empty()) {
        Some(url) => Arc::new(PostgresDatabase::new(url)),
        None => Arc::new(Unconfigured),
    }
}

pub fn from_env() -> Arc<dyn Database> {
    open(std::env::var("DATABASE_URL").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_url_the_database_is_missing() {
        assert_eq!(open(None).health(), Health::Missing);
        assert_eq!(open(Some("  ".into())).health(), Health::Missing);
    }

    #[test]
    fn a_url_that_cannot_be_parsed_leaves_the_database_broken_with_the_reason() {
        let health = open(Some("not a url at all".into())).health();
        assert!(matches!(&health, Health::Broken(reason) if !reason.is_empty()), "{health:?}");
        assert_eq!(health.label(), "broken");
    }
}
