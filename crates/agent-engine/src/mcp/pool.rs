use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use domain::PromptRequest;
use platform::sync::Locked;

use super::{McpServer, McpSources, ServerSpec};
use crate::model::http::Http;
use crate::tools::host_judge::HostJudge;

#[derive(Default)]
pub struct McpPool {
    open: Mutex<Vec<(ServerSpec, Arc<McpServer>)>>,
}

impl std::fmt::Debug for McpPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<String> = self.open.locked().iter().map(|(spec, _)| spec.name().to_owned()).collect();
        f.debug_struct("McpPool").field("open", &names).finish()
    }
}

impl McpPool {
    pub fn connect(
        &self,
        request: &PromptRequest,
        sources: &McpSources,
        http: &Arc<dyn Http>,
        stop: &platform::Stop,
        judge: &Arc<HostJudge>,
    ) -> Vec<Arc<McpServer>> {
        let mut connected = super::in_process(request, stop);
        let mut previous = std::mem::take(&mut *self.open.locked());
        let mut kept = Vec::new();
        for spec in super::wanted(request, sources, judge) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let reused = previous
                .iter()
                .position(|(known, server)| *known == spec && server.alive())
                .map(|at| previous.swap_remove(at).1);
            let server = match reused {
                Some(server) => Some(server),
                None => super::ready(spec.name(), McpServer::open_stopped(&spec, http, stop), stop),
            };
            let Some(server) = server else { continue };
            if pooled(&spec) {
                kept.push((spec, Arc::clone(&server)));
            }
            connected.push(server);
        }
        if stop.load(Ordering::Relaxed) {
            kept.append(&mut previous);
        }
        self.open.locked().append(&mut kept);
        close(previous);
        connected
    }

    pub fn close(&self) {
        close(std::mem::take(&mut *self.open.locked()));
    }

    pub fn len(&self) -> usize {
        self.open.locked().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn pooled(spec: &ServerSpec) -> bool {
    !matches!(spec, ServerSpec::Script { .. })
}

fn close(servers: Vec<(ServerSpec, Arc<McpServer>)>) {
    for (_, server) in servers {
        server.close();
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
