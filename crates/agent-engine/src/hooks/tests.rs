use serde_json::{Value, json};

use super::{CommandOutput, Group, Handled, HookOutcome, absorbed};

fn printed(stdout: Value) -> CommandOutput {
    CommandOutput { code: Some(0), stdout: stdout.to_string(), stderr: String::new() }
}

fn exited(code: i32, stderr: &str) -> CommandOutput {
    CommandOutput { code: Some(code), stdout: String::new(), stderr: stderr.into() }
}

fn absorb(event: &str, result: CommandOutput) -> (Result<Handled, String>, HookOutcome, Value) {
    let (mut decision, mut input) = (HookOutcome::default(), json!({"tool_input": {"command": "ls"}}));
    let event = super::HookEvent::named(event).expect("a hook event");
    let handled = absorbed(event, result, &mut decision, &mut input);
    (handled, decision, input)
}

#[test]
fn a_group_runs_for_every_subject_unless_its_matcher_names_some() {
    let group = |matcher: &str| Group { matcher: matcher.into(), hooks: Vec::new() };
    assert_eq!(group("").applies_to("Bash"), Ok(true));
    assert_eq!(group("*").applies_to("Bash"), Ok(true));
    assert_eq!(group("Bash|Edit").applies_to("Edit"), Ok(true));
    assert_eq!(group("Bash|Edit").applies_to("Read"), Ok(false));
    assert!(group("(").applies_to("Read").is_err(), "a matcher that is no pattern fails the hook");
}

#[test]
fn exit_code_two_blocks_with_what_the_hook_wrote_to_stderr() {
    let (handled, decision, _) = absorb("PreToolUse", exited(2, "not on main"));
    assert_eq!(handled, Ok(Handled::Blocked));
    assert_eq!(decision.blocked.as_deref(), Some("not on main"));
}

#[test]
fn any_other_failure_is_told_as_context_and_the_next_handler_still_runs() {
    let (handled, decision, _) = absorb("PreToolUse", exited(1, "crashed"));
    assert_eq!(handled, Ok(Handled::GoOn));
    assert_eq!(decision.blocked, None);
    assert_eq!(decision.context, "Hook failed: crashed\n");
    let (handled, decision, _) = absorb("ModelRequest", exited(1, "crashed"));
    assert_eq!((handled, decision.context.as_str()), (Ok(Handled::GoOn), ""), "a model hook fails quietly");
}

#[test]
fn a_printed_decision_blocks_with_the_first_reason_it_gives() {
    let (handled, decision, _) = absorb("Stop", printed(json!({"decision": "block", "reason": "tests fail"})));
    assert_eq!(handled, Ok(Handled::Blocked));
    assert_eq!(decision.blocked.as_deref(), Some("tests fail"));
    let (_, decision, _) = absorb("Stop", printed(json!({"continue": false, "stopReason": "enough"})));
    assert_eq!(decision.blocked.as_deref(), Some("enough"));
    let (_, decision, _) = absorb("Stop", printed(json!({"continue": false})));
    assert_eq!(decision.blocked.as_deref(), Some("Blocked by configured hook"));
}

#[test]
fn a_permission_answer_is_kept_and_a_denial_blocks() {
    let allow = json!({"hookSpecificOutput": {"permissionDecision": "allow"}});
    let (handled, decision, _) = absorb("PreToolUse", printed(allow));
    let allowed = Some(domain::permission_rules::PermissionDecision::Allow);
    assert_eq!((handled, decision.permission), (Ok(Handled::GoOn), allowed));
    let deny = json!({"hookSpecificOutput": {"decision": {"behavior": "deny"}}});
    let (handled, decision, _) = absorb("PermissionRequest", printed(deny));
    assert_eq!(handled, Ok(Handled::Blocked));
    assert_eq!(decision.blocked.as_deref(), Some("Permission hook denied the action"));
    let unknown = json!({"hookSpecificOutput": {"permissionDecision": "maybe"}});
    assert_eq!(absorb("PreToolUse", printed(unknown)).0, Err("invalid permission decision from hook".into()));
}

#[test]
fn an_updated_input_replaces_what_the_next_handler_sees_only_where_a_tool_is_about_to_run() {
    let updated = || json!({"hookSpecificOutput": {"updatedInput": {"command": "ls -a"}}});
    let (handled, decision, input) = absorb("PreToolUse", printed(updated()));
    assert_eq!(handled, Ok(Handled::GoOn));
    assert_eq!(decision.updated_input, Some(json!({"command": "ls -a"})));
    assert_eq!(input["tool_input"], json!({"command": "ls -a"}));
    let (_, decision, input) = absorb("PostToolUse", printed(updated()));
    assert_eq!((decision.updated_input, &input["tool_input"]), (None, &json!({"command": "ls"})));
    let asked = json!({"hookSpecificOutput": {"decision": {"updatedInput": {"command": "pwd"}}}});
    assert_eq!(absorb("PermissionRequest", printed(asked)).1.updated_input, Some(json!({"command": "pwd"})));
    let text = json!({"hookSpecificOutput": {"updatedInput": "ls -a"}});
    assert_eq!(absorb("PreToolUse", printed(text)).0, Err("hook updatedInput must be an object".into()));
}

#[test]
fn what_a_hook_says_to_the_model_is_gathered_line_by_line() {
    let said = json!({"systemMessage": "one", "hookSpecificOutput": {"additionalContext": "two"}});
    assert_eq!(absorb("PostToolUse", printed(said)).1.context, "one\ntwo\n");
    let plain = |event: &str| {
        let result = CommandOutput { code: Some(0), stdout: "today is monday".into(), stderr: String::new() };
        absorb(event, result).1.context
    };
    assert_eq!(plain("SessionStart"), "today is monday");
    assert_eq!(plain("RunStart"), "today is monday");
    assert_eq!(plain("UserPromptSubmit"), "today is monday");
    assert_eq!(plain("RunEnd"), "", "a run's end has nobody left to tell");
    assert_eq!(plain("PreToolUse"), "", "plain text from any other hook is not context");
}

#[test]
fn a_tool_s_output_and_a_model_s_reply_are_rewritten_only_by_their_own_event() {
    let rewrite = || json!({"hookSpecificOutput": {"updatedToolOutput": "clean", "model": "small", "text": "said"}});
    let (_, after_tool, _) = absorb("PostToolUse", printed(rewrite()));
    assert_eq!((after_tool.updated_output, after_tool.model, after_tool.text), (Some(json!("clean")), None, None));
    let (_, request, _) = absorb("ModelRequest", printed(rewrite()));
    assert_eq!((request.updated_output, request.model.as_deref(), request.text), (None, Some("small"), None));
    let (_, response, _) = absorb("ModelResponse", printed(rewrite()));
    assert_eq!((response.model, response.text.as_deref()), (None, Some("said")));
}
