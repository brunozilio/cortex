use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use serde_json::Value;

use crate::{Database, Health};

#[derive(Debug, Default)]
pub struct Memory {
    preferences: Mutex<HashMap<String, Value>>,
}

impl Database for Memory {
    fn health(&self) -> Health {
        Health::Ready
    }

    fn preference(&self, key: &str) -> Result<Option<Value>, String> {
        Ok(self.preferences.lock().unwrap_or_else(PoisonError::into_inner).get(key).cloned())
    }

    fn keep_preference(&self, key: &str, value: &Value) -> Result<(), String> {
        self.preferences.lock().unwrap_or_else(PoisonError::into_inner).insert(key.to_owned(), value.clone());
        Ok(())
    }
}
