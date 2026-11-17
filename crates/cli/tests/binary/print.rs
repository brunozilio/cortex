use crate::{Sandbox, command};

fn no_model() -> String {
    format!("{0}: no model is available: run `{0} login` first", command())
}

fn hooked_project(sandbox: &Sandbox) {
    let hooks = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"touch ran"}]}]}}"#;
    sandbox.write(&sandbox.project_dir().join("settings.json"), hooks);
}

#[test]
fn text_output_without_a_provider_fails_with_one_line_on_stderr() {
    let ran = Sandbox::fresh().run(&["-p", "explain", "this"]);
    ran.failed_with(1).quiet_on_stdout();
    assert_eq!(ran.stderr.trim_end(), no_model());
}

#[test]
fn json_output_without_a_provider_still_ends_with_a_failed_result() {
    let ran = Sandbox::fresh().run(&["-p", "--output-format", "json", "hi"]);
    ran.failed_with(1).complained(&no_model());
    let result = ran.json();
    assert_eq!(result["type"], "result");
    assert_eq!(result["subtype"], "error");
    assert_eq!(result["is_error"], true);
    assert!(result["error"].as_str().is_some_and(|error| error.starts_with("no model is available")), "{ran:?}");
    assert_eq!(result["result"], "");
}

#[test]
fn stream_json_output_without_a_provider_ends_with_a_failed_result_line() {
    let ran = Sandbox::fresh().run(&["--print", "--output-format", "stream-json", "hi"]);
    ran.failed_with(1).complained(&no_model());
    let lines: Vec<serde_json::Value> =
        ran.stdout.lines().map(|line| serde_json::from_str(line).expect("each line is JSON")).collect();
    let last = lines.last().expect("a result line");
    assert_eq!(last["type"], "result");
    assert_eq!(last["is_error"], true);
}

#[test]
fn stream_json_input_is_read_from_stdin_and_answered_with_a_result() {
    let input = concat!(r#"{"type":"user","message":{"content":"first"}}"#, "\n");
    let ran = Sandbox::fresh()
        .run_with_input(&["-p", "--input-format", "stream-json", "--output-format", "stream-json"], input);
    ran.failed_with(1).complained(&no_model());
    assert_eq!(ran.stdout.lines().count(), 1, "{ran:?}");
    assert_eq!(ran.json()["is_error"], true);
}

#[test]
fn an_unknown_input_format_is_refused_naming_the_known_ones() {
    let ran = Sandbox::fresh().run(&["-p", "--input-format", "xml", "hi"]);
    ran.failed_with(1).quiet_on_stdout().complained("unknown input format xml: text or stream-json");
}

#[test]
fn an_unknown_output_format_is_a_usage_error_naming_the_known_ones() {
    let ran = Sandbox::fresh().run(&["-p", "--output-format", "yaml", "hi"]);
    ran.failed_with(2).quiet_on_stdout().complained("unknown output format yaml: text, json or stream-json");
}

#[test]
fn print_only_options_are_refused_without_print() {
    let sandbox = Sandbox::fresh();
    for args in [
        ["--output-format", "json", "hi"].as_slice(),
        &["--input-format", "stream-json", "hi"],
        &["--max-turns", "3", "hi"],
        &["--verification", "tests pass", "hi"],
        &["--trust", "hi"],
    ] {
        let ran = sandbox.run(args);
        ran.failed_with(2).quiet_on_stdout().complained("--print");
    }
}

#[test]
fn print_without_a_prompt_says_it_needs_one() {
    let ran = Sandbox::fresh().run(&["-p"]);
    ran.failed_with(1).quiet_on_stdout().complained("--print needs a prompt");
}

#[test]
fn resume_without_an_id_is_refused_when_printing() {
    let ran = Sandbox::fresh().run(&["-p", "--resume", "--", "hi"]);
    ran.failed_with(1).quiet_on_stdout().complained("--resume needs a session id with --print");
}

#[test]
fn an_unknown_mode_is_a_usage_error_naming_the_modes() {
    let ran = Sandbox::fresh().run(&["-p", "--mode", "reckless", "hi"]);
    ran.failed_with(2).quiet_on_stdout().complained("`reckless` is not ask, edits, plan or bypass");
}

#[test]
fn trust_for_one_print_run_is_not_kept() {
    let sandbox = Sandbox::fresh();
    hooked_project(&sandbox);
    sandbox.run(&["-p", "--trust", "hi"]).failed_with(1).complained(&no_model());
    let after = sandbox.run(&["doctor"]);
    after.said("is not trusted: 1 project item skipped (Hooks in settings (1))");
    assert!(!sandbox.project().join("ran").exists(), "the project's hook never ran");
}
