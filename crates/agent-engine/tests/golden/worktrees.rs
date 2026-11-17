use domain::PermissionMode;
use serde_json::json;

use crate::models::{call, no_findings, says};
use crate::stage::Stage;

fn git(stage: &Stage, args: &[&str]) {
    let done = std::process::Command::new("git").arg("-C").arg(&stage.project).args(args).output().expect("git");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

#[cfg(unix)]
#[test]
fn a_main_agent_isolated_in_a_worktree_writes_there_and_the_worktree_is_retained() {
    let stage = Stage::new();
    git(&stage, &["init", "-q", "-b", "main"]);
    let agents = stage.project.join(brand::DIR).join("agents");
    std::fs::create_dir_all(&agents).expect("agents");
    std::fs::write(agents.join("builder.md"), "---\nname: builder\nisolation: worktree\n---\nBuild.").expect("agent");
    std::fs::write(stage.project.join(brand::DIR).join("settings.json"), r#"{"agent":"builder"}"#).expect("settings");
    git(&stage, &["add", "."]);
    git(&stage, &["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "base"]);
    stage.script([
        call("w1", "Write", json!({"file_path": "work.txt", "content": "isolated\n"})),
        says("Created."),
        no_findings(),
    ]);
    let mut request = stage.request("Create the work file");
    request.policy.permission = PermissionMode::Bypass;
    request.policy.safety_check = false;

    stage.play("prompt", &request);

    let kept = std::fs::read_dir(stage.project.join(brand::DIR).join("worktrees")).expect("worktrees").count();
    stage.note(
        "files",
        [
            format!("work.txt in the project: {}", stage.project.join("work.txt").exists()),
            format!("worktrees kept: {kept}"),
        ],
    );
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w1 Write "work.txt" input="{\"content\":\"isolated\\n\",\"file_path\":\"work.txt\"}"
        Edited(Edited { path: "<project>/.<brand>/worktrees/<uuid>/work.txt", line: 1, removed: 0, added: 1 })
        ToolFinished w1
        ToolResult w1 ok "Created work.txt · Wrote 1 lines\n    1 + isolated"
        Text "Created."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentSpawned #0 kind=review summary="Reviewing the changes" prompt="# Task\nCreate the work file\n\n# Changed files\n- work.txt"
        AgentUpdate #0 Running(0) summary=None
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("0 findings")
        Reviewed([])
        Notice("Changes retained in worktree: <project>/.<brand>/worktrees/<uuid>")
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Create the work file"
          reminder "# Runtime reminder\nThe user enabled autonomous execution. Stay within the authorized task and verify the result.\nCollect background agent reports before completing. U…
        assistant:
          tool_use w1 Write "{\"content\":\"isolated\\n\",\"file_path\":\"work.txt\"}"
        user:
          tool_result w1 ok "Created work.txt (1 lines)."
        assistant:
          text "Created."
        == prompt: log appended ==
        prompted "Create the work file"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted w1
        event Edited
        event ToolFinished w1
        event ToolResult w1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event AgentSpawned
        event AgentUpdate
        called model=claude-opus-5-5 stop=EndTurn messages=1 agent=#0
        event Agent #0 | Usage
        event AgentUpdate
        event Reviewed
        event Notice
        event Completed
        == files ==
        work.txt in the project: false
        worktrees kept: 1
        "##,
    );
}
