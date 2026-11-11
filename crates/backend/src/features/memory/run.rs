use domain::{PromptRequest, RunId};
use protocol::ServerEvent;

use super::NAME;
use super::mcp::MemoryEndpoint;
use crate::feature::{Context, Finish};

pub const TOOL: &str = "memory_search";

pub fn prepare(context: &Context<'_>, run: RunId, mut request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    let Some(brief) = &request.memory else { return (request, None) };
    let memories = super::of(context.services);
    let owner = context.principal.clone();
    let tool = !brief.optimizing && context.on(TOOL);
    if tool {
        request.credentials.mcp.push(MemoryEndpoint::for_run(memories.clone(), owner.clone(), request.cwd.clone()));
    }
    let request = memories.brief(&owner, run, &request, tool);
    let kept = request.clone();
    let finish: Finish = Box::new(move |completed| {
        let output = memories.finish(&owner, &kept, completed)?;
        Some(ServerEvent::Feature { feature: NAME.into(), output })
    });
    (request, Some(finish))
}
