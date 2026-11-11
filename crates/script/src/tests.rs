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
