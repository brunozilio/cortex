use crate::api::features::shell::{BackgroundCommand, ShellReply, ShellRequest};
use agent_engine::tools::builtin::shell::shells;
use agent_engine::tools::watch;

use crate::features::context::Context;

const SHOWN: usize = 20_000;
const TYPED_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

pub fn handle(context: &Context<'_>, request: ShellRequest) -> Result<ShellReply, String> {
    let client = context.client;
    match request {
        ShellRequest::List => Ok(listed(client)),
        ShellRequest::Kill { id } => {
            if watch::stop(client, &id).is_none() {
                shells::kill(client, &id);
            }
            Ok(listed(client))
        }
        ShellRequest::Run { command, path } => {
            let (output, failed) = platform::typed::run(&path, &command, TYPED_LIMIT)?;
            Ok(ShellReply::Ran { output, failed })
        }
        ShellRequest::Output { id } => shells::tail(client, &id, SHOWN)
            .map(ShellReply::Output)
            .ok_or_else(|| format!("no background command {id}")),
    }
}

fn listed(client: u64) -> ShellReply {
    let watches = watch::list(client).into_iter().map(|(id, explained)| (id, explained, true));
    let commands = shells::list(client).into_iter().chain(watches);
    ShellReply::Listed(commands.map(|(id, command, running)| BackgroundCommand { id, command, running }).collect())
}

pub fn forget(client: u64) {
    shells::kill_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::context::testing::local;

    #[test]
    fn a_client_sees_only_its_own_commands_and_killing_one_it_never_started_just_lists() {
        assert_eq!(local(78_001, |context| handle(context, ShellRequest::List)), Ok(ShellReply::Listed(Vec::new())));
        let killed = local(78_001, |context| handle(context, ShellRequest::Kill { id: "bash_none".into() }));
        assert_eq!(killed, Ok(ShellReply::Listed(Vec::new())));
        let missing = local(78_001, |context| handle(context, ShellRequest::Output { id: "bash_none".into() }));
        assert!(missing.is_err(), "no output of a command this client never started");
        forget(78_001);
    }

    #[cfg(unix)]
    #[test]
    fn a_command_the_user_typed_runs_in_their_folder() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        let ran = |command: &str| {
            let request = ShellRequest::Run { command: command.into(), path: dir.path().to_path_buf() };
            local(78_002, |context| handle(context, request)).expect("ran")
        };
        assert_eq!(ran("ls"), ShellReply::Ran { output: "a.txt".into(), failed: false });
        assert!(matches!(ran("exit 3"), ShellReply::Ran { failed: true, .. }));
    }

    #[cfg(unix)]
    #[test]
    fn a_background_command_s_output_is_shown_to_the_client_that_started_it() {
        let mut workspace = agent_engine::tools::Workspace::new(std::env::temp_dir());
        workspace.credentials.client = 78_003;
        let tools = agent_engine::tools::builtin::shell::tools();
        let bash = tools.iter().find(|tool| tool.spec().name == "Bash").expect("the Bash tool");
        let background = serde_json::json!({ "command": "echo ready; sleep 30", "run_in_background": true });
        let started = bash.run(&background, &workspace);
        let id = started.content.split_whitespace().find(|word| word.starts_with("bash_")).expect("an id");
        let id = id.trim_end_matches(|c: char| !c.is_alphanumeric()).to_owned();
        let ask = || local(78_003, |context| handle(context, ShellRequest::Output { id: id.clone() }));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while ask() != Ok(ShellReply::Output("ready".into())) {
            assert!(std::time::Instant::now() < deadline, "{:?}", ask());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let listed = local(78_003, |context| handle(context, ShellRequest::List)).expect("listed");
        assert!(matches!(&listed, ShellReply::Listed(commands) if commands[0].id == id && commands[0].running));
        forget(78_003);
    }
}
