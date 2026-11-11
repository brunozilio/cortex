use domain::{Principal, PromptRequest, RunId};
use protocol::ServerEvent;

use crate::services::LocalServices;

pub type Submit<'a> = &'a dyn Fn(RunId, PromptRequest) -> Result<(), String>;

pub type Finish = Box<dyn FnOnce(bool) -> Option<ServerEvent> + Send + Sync>;

pub struct Context<'a> {
    pub client: u64,
    pub principal: &'a Principal,
    pub services: &'a LocalServices,
    pub submit: Submit<'a>,
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").field("client", &self.client).finish_non_exhaustive()
    }
}

impl Context<'_> {
    pub fn on(&self, feature: &str) -> bool {
        !self.services.off.contains(feature)
    }
}

#[cfg(test)]
pub mod testing {
    use std::sync::Arc;

    use agent_engine::fake::FakeEngine;

    use super::*;

    pub fn services(data: Option<&std::path::Path>) -> LocalServices {
        let mut services = LocalServices::with_engine(Arc::new(FakeEngine::new(Vec::new(), Vec::new())));
        services.data = data.map(std::path::Path::to_path_buf);
        services
    }

    pub fn with<T>(client: u64, services: &LocalServices, body: impl FnOnce(&Context<'_>) -> T) -> T {
        let refuse = |_: RunId, _: PromptRequest| Err::<(), String>("no runs in this test".into());
        body(&Context { client, principal: &Principal::LocalOwner, services, submit: &refuse })
    }

    pub fn local<T>(client: u64, body: impl FnOnce(&Context<'_>) -> T) -> T {
        with(client, &services(None), body)
    }
}
