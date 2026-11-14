use domain::{PromptRequest, RunId};

use crate::feature::{Context, Finish};

pub fn prepare(context: &Context<'_>, _run: RunId, request: PromptRequest) -> (PromptRequest, Option<Finish>) {
    let optimizing = request.memory.as_ref().is_some_and(|brief| brief.optimizing);
    if optimizing || request.command.is_some() || request.prompt.trim().is_empty() {
        return (request, None);
    }
    let code_map = super::pointers(context, &request.cwd, &request.prompt);
    (PromptRequest { code_map, ..request }, None)
}

#[cfg(test)]
mod tests {
    use domain::ModelOption;

    use super::*;
    use crate::feature::testing;

    #[test]
    fn each_prompt_carries_the_pointers_that_match_it() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join(".git")).expect("git");
        std::fs::write(dir.path().join("lib.rs"), "pub fn compact_history() {}\n").expect("write");
        let data = tempfile::tempdir().expect("data");
        let services = testing::services(Some(data.path()));
        let request = PromptRequest::plain(dir.path().into(), "speed up compact_history", ModelOption::new("m", "M"));

        let (prepared, finish) = testing::with(1, &services, |context| prepare(context, RunId::new(), request));

        assert!(finish.is_none());
        let map = prepared.code_map.expect("map");
        assert!(map.contains("lib.rs") && map.contains("compact_history"), "{map}");
    }
}
