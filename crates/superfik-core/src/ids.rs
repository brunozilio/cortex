//! Strongly typed identifiers.
//!
//! `Id<T>` wraps a UUID with a phantom marker so a `SessionId` can never be
//! passed where a `WorkspaceId` is expected.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id<T> {
    value: Uuid,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub fn new() -> Self {
        Self::from_uuid(Uuid::new_v4())
    }

    pub const fn from_uuid(value: Uuid) -> Self {
        Self { value, marker: PhantomData }
    }

    pub const fn uuid(&self) -> Uuid {
        self.value
    }
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.value)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Debug)]
pub enum WorkspaceMarker {}
#[derive(Debug)]
pub enum UserMarker {}
#[derive(Debug)]
pub enum TurnMarker {}
#[derive(Debug)]
pub enum AgentMarker {}
#[derive(Debug)]
pub enum ConversationMarker {}
#[derive(Debug)]
pub enum BrowserMarker {}
#[derive(Debug)]
pub enum OrgMarker {}
#[derive(Debug)]
pub enum TeamMarker {}
#[derive(Debug)]
pub enum ProjectMarker {}

pub type WorkspaceId = Id<WorkspaceMarker>;
pub type UserId = Id<UserMarker>;
pub type TurnId = Id<TurnMarker>;
pub type AgentId = Id<AgentMarker>;
pub type ConversationId = Id<ConversationMarker>;
pub type BrowserId = Id<BrowserMarker>;
pub type OrgId = Id<OrgMarker>;
pub type TeamId = Id<TeamMarker>;
pub type ProjectId = Id<ProjectMarker>;

impl OrgId {
    pub const LOCAL: Self = Self::from_uuid(Uuid::nil());
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_copy() {
        let a = WorkspaceId::new();
        let b = a;
        assert_eq!(a, b);
        assert_ne!(a, WorkspaceId::new());
    }

    #[test]
    fn ids_serialize_as_plain_uuid() {
        let id = TurnId::new();
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, format!("\"{}\"", id.uuid()));
        let back: TurnId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);
    }
}
