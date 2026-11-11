use domain::{PromptRequest, RunId};

use crate::feature::{Context, Finish};

pub fn prepare(context: &Context<'_>, _run: RunId, mut request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    request.credentials.mcp.extend(super::for_run(context.client));
    (request, None)
}
