use domain::memory::MemorySearches;
use domain::{PromptRequest, RunId};
use protocol::ServerEvent;

use super::NAME;
use super::mcp::MemoryEndpoint;
use crate::feature::{Context, Finish};

pub const TOOL: &str = "memory_search";

pub fn prepare(context: &Context<'_>, run: RunId, request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    let Some(brief) = &request.memory else { return (request, None) };
    let memories = super::of(context.services);
    let owner = context.principal.clone();
    let wanted = !brief.optimizing && context.on(TOOL);
    let mut request = memories.brief(&owner, run, &request, wanted);
    if let Some(brief) = request.memory.as_ref().filter(|brief| brief.tool) {
        let shown = if request.resume.is_none() { brief.recalled.iter().map(|m| m.id).collect() } else { Vec::new() };
        let searches = MemorySearches::new(shown);
        request.credentials.mcp.push(MemoryEndpoint::for_run(
            memories.clone(),
            owner.clone(),
            request.cwd.clone(),
            searches,
        ));
    }
    let kept = request.clone();
    let finish: Finish = Box::new(move |completed| {
        let output = memories.finish(&owner, &kept, completed)?;
        Some(ServerEvent::Feature { feature: NAME.into(), output })
    });
    (request, Some(finish))
}
