use std::sync::Mutex;

use serde_json::json;

use super::*;

#[derive(Default)]
struct Recorder(Mutex<Vec<(String, Value)>>);

impl Host for Recorder {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        self.0.lock().expect("lock").push((function.to_owned(), arguments.clone()));
        match function {
            "text" => Ok(json!(format!("text of {}", arguments.as_str().unwrap_or_default()))),
            "llm" => Ok(json!(format!("answer to {}", arguments["prompt"].as_str().unwrap_or_default()))),
            "fail" => Err("it failed".into()),
            _ => Ok(Value::Null),
        }
    }
}

fn go(source: &str, export: &str, arguments: &[Value], data: Value) -> (Result<Value, String>, Arc<Recorder>) {
    let host = Arc::new(Recorder::default());
    let result = run("test.js", source, export, arguments, &data, host.clone(), Arc::new(AtomicBool::new(false)));
    (result, host)
}

#[test]
fn recursion_without_end_is_an_error_whatever_stack_the_caller_has_left() {
    let endless = "export default function f() { return f() + 1; }";
    let small = std::thread::Builder::new().stack_size(256 * 1024);
    let ran = small
        .spawn(move || {
            let (result, _) = go(endless, "default", &[], json!({}));
            let spec = exported("spec.js", "function f() { return f() + 1; }\nexport default f();", "default");
            let found = exports("spec.js", "function f() { return f() + 1; }\nf();\nexport const run = 1;", "run");
            let empty = empty_function("spec.js", "function f() { return f() + 1; }\nf();", "default");
            (result, spec, found, empty)
        })
        .expect("thread")
        .join()
        .expect("a runaway script never takes the caller's thread down");
    let refused = ran.0.expect_err("the stack runs out");
    assert!(refused.to_lowercase().contains("stack"), "{refused}");
    assert!(ran.1.is_err());
    assert!(!ran.2 && !ran.3);
}

#[test]
fn a_script_that_keeps_allocating_fails_instead_of_taking_the_machine_s_memory() {
    let hog = r#"
      export default function () {
        const kept = [];
        for (let i = 0; i < 256; i++) kept.push("x".repeat(1 << 20) + i);
        return kept.length;
      }"#;
    let (result, _) = go(hog, "default", &[], json!({}));
    let refused = result.expect_err("256 MiB is past what a script may hold");
    assert!(refused.to_lowercase().contains("memory"), "{refused}");
    let (after, _) = go("export default () => 1 + 1;", "default", &[], json!({}));
    assert_eq!(after, Ok(json!(2)), "the next script starts fresh");
}

#[test]
fn a_script_that_recurses_without_end_fails_instead_of_overflowing_the_stack() {
    let (result, _) =
        go("const deeper = () => deeper() + 1;\nexport default () => deeper();", "default", &[], json!({}));
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn a_prompt_is_a_function_of_its_context() {
    let source = r##"
      export default function (ctx) {
        const lines = [ctx.text("prompts/role.md"), "# Environment", `- Working directory: ${ctx.cwd}`];
        if (ctx.tools.includes("Advisor")) lines.push("advisor on");
        return lines.join("\n");
      }"##;
    let (result, host) = go(source, "default", &[], json!({ "cwd": "/repo", "tools": ["Read"] }));
    assert_eq!(result, Ok(json!("text of prompts/role.md\n# Environment\n- Working directory: /repo")));
    assert_eq!(host.0.lock().expect("lock")[0].0, "text");
}

#[test]
fn a_tool_gets_its_input_and_calls_back_through_its_context() {
    let source = r#"
      export const spec = { name: "Echo" };
      export function run(input, ctx) {
        ctx.log("working");
        return { said: ctx.llm(`say ${input.word}`), custom: ctx.anything({ x: 1 }) };
      }"#;
    let (result, host) = go(source, "run", &[json!({ "word": "hi" })], json!({}));
    assert_eq!(result, Ok(json!({ "said": "answer to say hi", "custom": null })));
    let calls: Vec<String> = host.0.lock().expect("lock").iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(calls, ["log", "llm", "anything"]);
}

#[test]
fn a_host_error_is_a_javascript_exception_and_a_script_error_is_reported() {
    let caught = r#"export function run(_, ctx) { try { ctx.call("fail") } catch (e) { return e.message } }"#;
    assert_eq!(go(caught, "run", &[json!(null)], json!({})).0, Ok(json!("it failed")));
    let broken = r#"export function run() { return missing.value }"#;
    let error = go(broken, "run", &[json!(null)], json!({})).0.expect_err("broken");
    assert!(error.contains("missing"), "{error}");
}

#[test]
fn an_async_function_is_awaited() {
    let source = r#"export async function run(input) { return input * 2 }"#;
    assert_eq!(go(source, "run", &[json!(21)], json!({})).0, Ok(json!(42)));
}

#[test]
fn stopping_interrupts_a_script_that_never_ends() {
    let host = Arc::new(Recorder::default());
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        flag.store(true, Ordering::Relaxed);
    });
    let started = std::time::Instant::now();
    let result = run("loop.js", "export function run() { for (;;) {} }", "run", &[], &json!({}), host, stop);
    assert_eq!(result, Err("cancelled".to_owned()));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn an_exported_value_is_read_without_running_anything_else() {
    let source = r#"export const spec = { name: "Echo", input: { type: "object" } }; export function run() {}"#;
    assert_eq!(exported("echo.js", source, "spec"), Ok(json!({ "name": "Echo", "input": { "type": "object" } })));
    assert!(exported("bad.js", "export const spec = ", "spec").is_err());
}

#[test]
fn an_export_that_is_not_a_function_is_its_own_value() {
    let source = r#"export const spec = { name: "Echo", description: "Says it back." };"#;
    assert_eq!(go(source, "spec.description", &[], json!({})).0, Ok(json!("Says it back.")));
}

#[test]
fn whether_a_module_exports_a_name_is_known_without_running_it() {
    let source = r#"export const spec = {}; export function execute(input) { return input }"#;
    assert!(exports("echo.js", source, "execute"));
    assert!(!exports("echo.js", source, "run"));
    assert!(!exports("bad.js", "export const spec = ", "spec"));
}

#[test]
fn a_function_with_an_empty_body_is_known_as_empty() {
    let empty = |source: &str| empty_function("hook.js", source, "default");
    assert!(empty("export default (input, ctx) => {};"));
    assert!(empty("export default function (input) {}"));
    assert!(empty("export default async function named(input, ctx) {\n}"));
    assert!(!empty("export default (input) => ({ decision: 'block' });"));
    assert!(!empty("export default (input) => { return input };"));
    assert!(!empty("export const event = 'Stop';"));
}

struct Slow;

impl Host for Slow {
    fn call(&self, _function: &str, _arguments: Value) -> Result<Value, String> {
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok(Value::Null)
    }
}

#[test]
fn a_script_that_never_ends_is_stopped_by_its_running_time_even_with_nobody_to_stop_it() {
    let stop = Arc::new(AtomicBool::new(false));
    let busy = Limit::Busy(std::time::Duration::from_millis(100));
    let source = "export function run() { for (;;) {} }";
    let started = std::time::Instant::now();
    let host = Arc::new(Recorder::default());
    let result = run_limited("loop.js", source, "run", &[], &json!({}), host, Arc::clone(&stop), busy);
    let error = result.expect_err("the budget ends it");
    assert!(error.contains("running time"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(!stop.load(Ordering::Relaxed));
}

#[test]
fn waiting_on_the_host_does_not_count_as_running_time() {
    let stop = Arc::new(AtomicBool::new(false));
    let busy = Limit::Busy(std::time::Duration::from_millis(200));
    let source = r#"export function run(_, ctx) { ctx.log("a"); ctx.log("b"); ctx.log("c"); return "done" }"#;
    let result = run_limited("slow.js", source, "run", &[json!(null)], &json!({}), Arc::new(Slow), stop, busy);
    assert_eq!(result, Ok(json!("done")), "450 ms waiting on the host, little of it running");
}

#[test]
fn every_run_has_a_running_time_budget() {
    assert!(MOST_BUSY >= std::time::Duration::from_secs(10) && MOST_BUSY <= std::time::Duration::from_secs(300));
}

#[test]
fn a_script_past_its_time_limit_fails_without_stopping_its_caller() {
    let host = Arc::new(Recorder::default());
    let stop = Arc::new(AtomicBool::new(false));
    let limit = Some(std::time::Duration::from_millis(100));
    let source = "export function run() { for (;;) {} }";
    let result = run_within("loop.js", source, "run", &[], &json!({}), host, Arc::clone(&stop), limit);
    let error = result.expect_err("the limit ends it");
    assert!(error.contains("timed out"), "{error}");
    assert!(!stop.load(Ordering::Relaxed), "the caller's stop flag is the caller's to set");
}
