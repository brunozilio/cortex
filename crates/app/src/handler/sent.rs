use domain::{MemoryBrief, PromptRequest};

pub fn as_sent(request: &PromptRequest) -> PromptRequest {
    let mut sent = request.clone();
    sent.host.memory = request
        .host
        .memory
        .as_ref()
        .map(|brief| MemoryBrief { origin: brief.origin.clone(), ..MemoryBrief::default() });
    sent.host.extensions = None;
    sent
}

#[cfg(test)]
mod tests {
    use domain::{Extensions, ModelOption};

    use super::*;

    #[test]
    fn a_client_cannot_hand_a_run_its_own_tools_files_or_trust() {
        let mut request = PromptRequest::plain("/srv/u/rocket".into(), "hi", ModelOption::new("m", "M"));
        request.host.extensions = Some(Extensions {
            dirs: vec!["/etc".into()],
            plugins: vec!["/tmp/evil".into()],
            trust: domain::trust::Trust::Trusted,
            ..Extensions::default()
        });
        request.host.memory =
            Some(MemoryBrief { output: "/etc/passwd".into(), origin: Some("Launch".into()), ..MemoryBrief::default() });
        let sent = as_sent(&request);
        assert_eq!(sent.host.extensions, None);
        assert_eq!(sent.host.memory, Some(MemoryBrief::new(Some("Launch".into()))));
        assert_eq!(sent.prompt.text, "hi");
    }
}
