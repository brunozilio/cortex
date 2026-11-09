//! Manually selected, stable browser laboratory profiles. No IO or secrets.

mod configuration;
pub use configuration::{GpuIdentity, Laboratory, Proxy, ProxyScheme, Screen, WebRtc};

use crate::Id;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug)]
pub enum ProfileMarker {}
pub type ProfileId = Id<ProfileMarker>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ProfileField {
    Name,
    Cpu,
    Memory,
    Seed,
    GpuVendor,
    GpuRenderer,
    GpuArchitecture,
    GpuDevice,
    GpuDescription,
    Fonts,
    Width,
    Height,
    Scale,
    Depth,
    ProxyHost,
    ProxyPort,
    ProxyUsername,
    ProxyPassword,
}
impl ProfileField {
    pub const ALL: [Self; 18] = [
        Self::Name,
        Self::Cpu,
        Self::Memory,
        Self::Seed,
        Self::GpuVendor,
        Self::GpuRenderer,
        Self::GpuArchitecture,
        Self::GpuDevice,
        Self::GpuDescription,
        Self::Fonts,
        Self::Width,
        Self::Height,
        Self::Scale,
        Self::Depth,
        Self::ProxyHost,
        Self::ProxyPort,
        Self::ProxyUsername,
        Self::ProxyPassword,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserProfile {
    pub id: ProfileId,
    pub name: String,
    pub settings: Laboratory,
}

impl BrowserProfile {
    pub fn new(name: String) -> Self {
        let id = ProfileId::new();
        let bytes = id.uuid().into_bytes();
        let seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).max(1);
        Self { id, name, settings: Laboratory { seed, ..Laboratory::default() } }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBook {
    pub schema: u32,
    pub profiles: Vec<BrowserProfile>,
    pub assignments: BTreeMap<String, ProfileId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub actual_values: BTreeSet<String>,
}

impl Default for ProfileBook {
    fn default() -> Self {
        Self { schema: 1, profiles: Vec::new(), assignments: BTreeMap::new(), actual_values: BTreeSet::new() }
    }
}

impl ProfileBook {
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > 1024 * 1024 {
            return Err("Profile library exceeds 1 MiB".into());
        }
        let book: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        book.validate()?;
        Ok(book)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.profiles.len() > 64 {
            return Err("Unsupported profile library schema or size".into());
        }
        let mut ids = HashSet::new();
        for profile in &self.profiles {
            if !ids.insert(profile.id) {
                return Err("Duplicate profile ID".into());
            }
            if profile.name.trim().is_empty() || profile.name.len() > 120 {
                return Err("Profile name must contain 1–120 bytes".into());
            }
            profile.settings.validate()?;
        }
        if self.assignments.values().any(|id| !ids.contains(id)) {
            return Err("Conversation refers to a missing profile".into());
        }
        if self.actual_values.iter().any(|name| self.assignments.contains_key(name)) {
            return Err("Conversation cannot select both a profile and actual values".into());
        }
        Ok(())
    }

    pub fn save(&mut self, profile: BrowserProfile) -> Result<(), String> {
        let mut next = self.clone();
        if let Some(existing) = next.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            next.profiles.push(profile);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn assign(&mut self, conversation: String, id: Option<ProfileId>) -> Result<(), String> {
        if let Some(id) = id {
            if !self.profiles.iter().any(|p| p.id == id) {
                return Err("Profile does not exist".into());
            }
            self.actual_values.remove(&conversation);
            self.assignments.insert(conversation, id);
        } else {
            self.assignments.remove(&conversation);
            self.actual_values.insert(conversation);
        }
        Ok(())
    }

    pub fn delete(&mut self, id: ProfileId) -> Result<(), String> {
        if self.assignments.values().any(|assigned| *assigned == id) {
            return Err("Unassign this profile before deleting it".into());
        }
        self.profiles.retain(|p| p.id != id);
        Ok(())
    }

    pub fn for_conversation(&self, conversation: &str) -> Option<&BrowserProfile> {
        let id = self.assignments.get(conversation)?;
        self.profiles.iter().find(|p| &p.id == id)
    }

    pub fn uses_actual_values(&self, conversation: &str) -> bool {
        self.actual_values.contains(conversation)
    }
}

#[cfg(test)]
mod tests;
