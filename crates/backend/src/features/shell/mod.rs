use agent_engine::tools::builtin::shell::shells;
use agent_engine::tools::watch;
use serde_json::{Value, json};

use crate::feature::Context;

const SHOWN: usize = 20_000;
const TYPED_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    if let Some(id) = input["kill"].as_str()
        && watch::stop(context.client, id).is_none()
    {
        shells::kill(context.client, id);
    }
    if let Some(command) = input["run"].as_str() {
        let cwd = input["path"].as_str().ok_or("a typed command needs the folder to run in")?;
        let (output, failed) = platform::typed::run(std::path::Path::new(cwd), command, TYPED_LIMIT)?;
        return Ok(json!({ "output": output, "failed": failed }));
    }
    if let Some(id) = input["output"].as_str() {
        let seen = shells::tail(context.client, id, SHOWN).ok_or_else(|| format!("no background command {id}"))?;
        return Ok(json!({ "output": seen }));
    }
    let watches = watch::list(context.client).into_iter().map(|(id, explained)| (id, explained, true));
    let listed: Vec<_> = shells::list(context.client).into_iter().chain(watches).collect();
    Ok(json!({ "shells": listed }))
}

pub fn forget(client: u64) {
    shells::kill_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_sees_only_its_own_commands() {
        let listed = crate::feature::testing::local(78_001, |context| handle(context, json!({ "kill": "bash_none" })))
            .expect("listed");
        assert_eq!(listed, json!({ "shells": [] }));
        let missing =
            crate::feature::testing::local(78_001, |context| handle(context, json!({ "output": "bash_none" })));
        assert!(missing.is_err(), "no output of a command this client never started");
        forget(78_001);
    }

    #[cfg(unix)]
    #[test]
    fn a_command_the_user_typed_runs_in_their_folder() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let ran = |command: &str| {
            let input = json!({ "run": command, "path": dir.path() });
            crate::feature::testing::local(78_002, |context| handle(context, input)).expect("ran")
        };
        assert_eq!(ran("ls"), json!({ "output": "a.txt", "failed": false }));
        assert_eq!(ran("exit 3")["failed"], true);
        let nowhere = crate::feature::testing::local(78_002, |context| handle(context, json!({ "run": "ls" })));
        assert!(nowhere.is_err(), "a typed command needs its folder");
    }

    #[cfg(unix)]
    #[test]
    fn a_background_command_s_output_is_shown_to_the_client_that_started_it() {
        let mut workspace = agent_engine::tools::Workspace::new(std::env::temp_dir());
        workspace.credentials.client = 78_003;
        let tools = agent_engine::tools::builtin::shell::tools();
        let bash = tools.iter().find(|tool| tool.spec().name == "Bash").expect("the Bash tool");
        let started = bash.run(&json!({ "command": "echo ready; sleep 30", "run_in_background": true }), &workspace);
        let id = started.content.split_whitespace().find(|word| word.starts_with("bash_")).expect("an id");
        let id = id.trim_end_matches(|c: char| !c.is_alphanumeric()).to_owned();
        let ask = || crate::feature::testing::local(78_003, |context| handle(context, json!({ "output": id })));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while ask().map(|seen| seen["output"] != "ready").unwrap_or(true) {
            assert!(std::time::Instant::now() < deadline, "{:?}", ask());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        forget(78_003);
    }
}
