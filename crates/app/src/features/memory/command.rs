use crate::api::features::memory::{MemoryOp, MemoryReply, MemoryRequest};
use domain::{Author, Provenance};

use super::{now, of};
use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: MemoryRequest) -> Result<MemoryReply, String> {
    let memories = of(context.services);
    let MemoryRequest { path, op } = request;
    match op {
        MemoryOp::List => Ok(MemoryReply::Listing(memories.load(&path))),
        MemoryOp::Search { query } => {
            let scores = memories.search(&path, &query);
            Ok(MemoryReply::Found { path, query, scores })
        }
        MemoryOp::Edit { edits } => {
            let by = Provenance { author: Author::User, now: now(), origin: None };
            memories.edit(&path, &edits, by).map(MemoryReply::Listing)
        }
        MemoryOp::Undo => memories.undo(&path).map(MemoryReply::Listing),
        MemoryOp::Share { id } => {
            let file = memories.shared(&path, id)?;
            Ok(MemoryReply::Shared { path, file })
        }
        MemoryOp::Check => memories.check(&path).map(MemoryReply::Check),
        MemoryOp::Score => {
            let report = memories.score(&path);
            Ok(MemoryReply::Score { path, report })
        }
        MemoryOp::Seed => {
            let candidates = memories.offered(context, &path)?;
            Ok(MemoryReply::Seeds { path, candidates })
        }
        MemoryOp::KeepSeeds { picked } => memories.keep_seeds(context, &path, &picked).map(MemoryReply::Listing),
        MemoryOp::Optimize { run, model } => {
            (context.submit)(run, memories.optimization(&path, model))?;
            Ok(MemoryReply::Optimizing { run })
        }
    }
}
