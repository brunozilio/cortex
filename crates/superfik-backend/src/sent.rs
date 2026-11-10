//! What a client may say about a prompt. The tools, files and recall a run
//! gets are the backend's to grant: whatever a client put there is dropped
//! before the run starts.

use superfik_core::{MemoryBrief, PromptRequest};

pub fn as_sent(request: &PromptRequest) -> PromptRequest {
    let memory =
        request.memory.as_ref().map(|brief| MemoryBrief { origin: brief.origin.clone(), ..MemoryBrief::default() });
    PromptRequest { plugins: None, memory, ..request.clone() }
}

#[cfg(test)]
mod tests {
    use superfik_core::{EngineKind, ModelOption, PluginBrief};

    use super::*;

    #[test]
    fn a_client_cannot_hand_a_run_its_own_tools_or_files() {
        let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
        let mut request = PromptRequest::plain("/srv/u/rocket".into(), "hi", model);
        request.plugins = Some(PluginBrief::default());
        request.memory = Some(MemoryBrief {
            output: "/etc/passwd".into(),
            optimizing: true,
            origin: Some("Launch".into()),
            ..MemoryBrief::default()
        });
        let sent = as_sent(&request);
        assert_eq!(sent.plugins, None);
        assert_eq!(sent.memory, Some(MemoryBrief::new(Some("Launch".into()))));
        assert_eq!(sent.prompt, "hi");
    }
}
