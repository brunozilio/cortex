use domain::{MemoryBrief, PromptRequest};

pub fn as_sent(request: &PromptRequest) -> PromptRequest {
    let memory =
        request.memory.as_ref().map(|brief| MemoryBrief { origin: brief.origin.clone(), ..MemoryBrief::default() });
    PromptRequest { extensions: None, memory, ..request.clone() }
}

#[cfg(test)]
mod tests {
    use domain::{Extensions, ModelOption};

    use super::*;

    #[test]
    fn a_client_cannot_hand_a_run_its_own_tools_or_files() {
        let mut request = PromptRequest::plain("/srv/u/rocket".into(), "hi", ModelOption::new("m", "M"));
        request.extensions =
            Some(Extensions { dirs: vec!["/etc".into()], plugins: vec!["/tmp/evil".into()], ..Extensions::default() });
        request.memory =
            Some(MemoryBrief { output: "/etc/passwd".into(), origin: Some("Launch".into()), ..MemoryBrief::default() });
        let sent = as_sent(&request);
        assert_eq!(sent.extensions, None);
        assert_eq!(sent.memory, Some(MemoryBrief::new(Some("Launch".into()))));
        assert_eq!(sent.prompt, "hi");
    }
}
