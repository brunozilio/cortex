use std::path::PathBuf;

use crate::api::features::memory::{MemoryListing, MemoryOp, MemoryReply, MemoryRequest};
use crate::api::{ClientCommand, FeatureRequest};
use domain::ModelOption;
use domain::memory::seed::Candidate;
use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId, MemoryKind};

use crate::client::features::unexpected;
use crate::{Client, Error, Run};

impl Client {
    pub fn memory(&self, cwd: impl Into<PathBuf>) -> ProjectMemory<'_> {
        ProjectMemory { client: self, path: cwd.into() }
    }
}

#[derive(Debug)]
pub struct ProjectMemory<'a> {
    client: &'a Client,
    path: PathBuf,
}

impl ProjectMemory<'_> {
    pub fn list(&self) -> Result<Vec<Memory>, Error> {
        self.listing(MemoryOp::List).map(|listing| listing.memories)
    }

    pub fn search(&self, query: &str) -> Result<Vec<(Memory, f32)>, Error> {
        let MemoryReply::Found { scores, .. } = self.ask(MemoryOp::Search { query: query.to_owned() })? else {
            return Err(unexpected());
        };
        let memories = self.list()?;
        Ok(scores
            .into_iter()
            .filter_map(|(id, score)| Some((memories.iter().find(|m| m.id == id)?.clone(), score)))
            .collect())
    }

    pub fn remember(&self, kind: MemoryKind, text: &str, tags: &[&str]) -> Result<Vec<Memory>, Error> {
        let tags = tags.iter().map(|tag| (*tag).to_owned()).collect();
        self.edit(vec![MemoryEdit::Remember { kind, text: text.to_owned(), tags, touches: Vec::new() }])
    }

    pub fn check(&self) -> Result<Option<String>, Error> {
        match self.ask(MemoryOp::Check)? {
            MemoryReply::Check(check) => Ok(check.prompt),
            _ => Err(unexpected()),
        }
    }

    pub fn score(&self) -> Result<String, Error> {
        match self.ask(MemoryOp::Score)? {
            MemoryReply::Score { report, .. } => Ok(report),
            _ => Err(unexpected()),
        }
    }

    pub fn seed(&self) -> Result<Vec<Candidate>, Error> {
        match self.ask(MemoryOp::Seed)? {
            MemoryReply::Seeds { candidates, .. } => Ok(candidates),
            _ => Err(unexpected()),
        }
    }

    pub fn keep_seeds(&self, picked: &[usize]) -> Result<Vec<Memory>, Error> {
        self.listing(MemoryOp::KeepSeeds { picked: picked.to_vec() }).map(|listing| listing.memories)
    }

    pub fn forget(&self, id: MemoryId) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Forget { id }])
    }

    pub fn revise(&self, id: MemoryId, text: &str) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Revise { id, text: text.to_owned() }])
    }

    pub fn edit(&self, edits: Vec<MemoryEdit>) -> Result<Vec<Memory>, Error> {
        self.listing(MemoryOp::Edit { edits }).map(|listing| listing.memories)
    }

    pub fn restore(&self, id: MemoryId) -> Result<Vec<Memory>, Error> {
        self.edit(vec![MemoryEdit::Restore { id }])
    }

    pub fn share(&self, id: MemoryId) -> Result<PathBuf, Error> {
        match self.ask(MemoryOp::Share { id })? {
            MemoryReply::Shared { file, .. } => Ok(file),
            _ => Err(unexpected()),
        }
    }

    pub fn forgotten(&self) -> Result<Vec<Forgotten>, Error> {
        self.listing(MemoryOp::List).map(|listing| listing.forgotten)
    }

    pub fn optimize(&self, model: ModelOption) -> Result<Run, Error> {
        let path = self.path.clone();
        Run::start(&self.client.dispatcher, |run| {
            let op = MemoryOp::Optimize { run, model };
            ClientCommand::Feature(FeatureRequest::Memory(MemoryRequest { path, op }))
        })
    }

    pub fn undo(&self) -> Result<Vec<Memory>, Error> {
        self.listing(MemoryOp::Undo).map(|listing| listing.memories)
    }

    fn listing(&self, op: MemoryOp) -> Result<MemoryListing, Error> {
        match self.ask(op)? {
            MemoryReply::Listing(listing) => Ok(listing),
            _ => Err(unexpected()),
        }
    }

    fn ask(&self, op: MemoryOp) -> Result<MemoryReply, Error> {
        self.client.ask_memory(MemoryRequest { path: self.path.clone(), op })
    }
}
