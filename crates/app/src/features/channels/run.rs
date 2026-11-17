use domain::{PromptRequest, RunId};

use crate::features::context::{Context, Finish};

pub fn prepare(context: &Context<'_>, _run: RunId, mut request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    request.host.credentials.mcp.extend(super::for_run(context.client));
    (request, None)
}
