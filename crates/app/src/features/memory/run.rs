use crate::api::Unsolicited;
use domain::memory::MemorySearches;
use domain::{PromptRequest, RunId};

use super::mcp::MemoryEndpoint;
use crate::features::context::{Context, Finish};

pub fn prepare(context: &Context<'_>, run: RunId, request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    let Some(brief) = &request.host.memory else { return (request, None) };
    let memories = super::of(context.services);
    let wanted = !brief.optimizing && context.on(domain::features::MEMORY_SEARCH);
    let mut request = memories.brief(run, &request, wanted);
    if let Some(brief) = request.host.memory.as_ref().filter(|brief| brief.tool) {
        let first = request.session.resume.is_none();
        let shown = if first { brief.recalled.iter().map(|m| m.id).collect() } else { Vec::new() };
        let searches = MemorySearches::new(shown);
        let endpoint = MemoryEndpoint::for_run(memories.clone(), request.session.cwd.clone(), searches);
        request.host.credentials.mcp.push(endpoint);
    }
    let kept = request.clone();
    let finish: Finish = Box::new(move |outcome| {
        Some(match memories.finish(&kept, outcome)? {
            Ok(applied) => Unsolicited::MemoryApplied(applied),
            Err(message) => Unsolicited::MemoryFailed { path: kept.session.cwd.clone(), message },
        })
    });
    (request, Some(finish))
}
