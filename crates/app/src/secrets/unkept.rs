use domain::SecretId;

use super::{SecretStore, Secrets};

#[derive(Debug)]
pub struct Unkept {
    reason: String,
}

impl Unkept {
    pub fn because(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

pub fn refusal(reason: &str) -> String {
    format!("keys cannot be kept on this machine: {reason}")
}

impl SecretStore for Unkept {
    fn get(&self, _: SecretId) -> Option<Secrets> {
        None
    }

    fn set(&self, _: SecretId, _: &Secrets) -> Result<(), String> {
        Err(refusal(&self.reason))
    }

    fn delete(&self, _: SecretId) {}

    fn update(&self, _: SecretId, _: &mut dyn FnMut(&mut Secrets)) -> Result<(), String> {
        Err(refusal(&self.reason))
    }

    fn unkept(&self) -> Option<String> {
        Some(self.reason.clone())
    }
}
