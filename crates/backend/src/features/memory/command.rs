use std::path::PathBuf;

use domain::{Author, MemoryEdit, MemoryId, ModelOption, Provenance, RunId};
use serde_json::{Value, json};

use super::{now, of};
use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let memories = of(context.services);
    let owner = context.principal;
    let path: PathBuf = input["path"].as_str().map(PathBuf::from).ok_or("the memory feature needs a path")?;
    match input["op"].as_str().unwrap_or("list") {
        "list" => Ok(memories.load(owner, &path)),
        "search" => Ok(memories.search_listing(owner, &path, input["query"].as_str().unwrap_or_default())),
        "edit" => {
            let edits: Vec<MemoryEdit> = serde_json::from_value(input["edits"].clone()).map_err(|e| e.to_string())?;
            memories.edit(owner, &path, &edits, Provenance { author: Author::User, now: now(), origin: None })
        }
        "undo" => memories.undo(owner, &path),
        "share" => {
            let id = MemoryId(input["id"].as_u64().and_then(|id| u32::try_from(id).ok()).ok_or("say which memory")?);
            let shared = memories.shared(owner, &path, id)?;
            Ok(json!({ "path": path, "file": shared }))
        }
        "check" => memories.check(owner, &path),
        "seed" => memories.seed(context, &path),
        "seed_keep" => {
            let picked: Vec<usize> = serde_json::from_value(input["picked"].clone()).map_err(|e| e.to_string())?;
            memories.keep_seeds(context, &path, &picked)
        }
        "optimize" => {
            let run: RunId = serde_json::from_value(input["run"].clone()).map_err(|e| e.to_string())?;
            let model: ModelOption = serde_json::from_value(input["model"].clone()).map_err(|e| e.to_string())?;
            (context.submit)(run, memories.optimization(owner, &path, model))?;
            Ok(json!({ "run": run }))
        }
        other => Err(format!("the memory feature has no {other:?}")),
    }
}
