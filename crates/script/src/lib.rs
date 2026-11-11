use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rquickjs::{Context, Ctx, Function, Module, Runtime, Value as JsValue};
use serde_json::Value;

pub trait Host: Send + Sync {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String>;
}

const PRELUDE: &str = r#"
const call = (name, value) => {
  const answer = JSON.parse(globalThis.__host(name, JSON.stringify(value === undefined ? null : value)));
  if (answer.error !== undefined) throw new Error(answer.error);
  return answer.ok;
};
globalThis.__ctx = (data) => new Proxy({
  ...data,
  call,
  text: (path) => call("text", path),
  include: (path, extra) => call("include", { path, data: { ...data, ...(extra || {}) } }),
  llm: (request) => call("llm", typeof request === "string" ? { prompt: request } : request),
  tool: (name, input) => call("tool", { name, input: input || {} }),
  log: (text) => call("log", String(text)),
  readFile: (path) => call("readFile", path),
  writeFile: (path, text) => call("writeFile", { path, text }),
  exec: (command, options) => call("exec", { command, ...(options || {}) }),
  fetch: (url, options) => call("fetch", { url, ...(options || {}) }),
}, { get: (target, key) => (key in target ? target[key] : (...args) => call(String(key), args.length > 1 ? args : args[0])) });
"#;

pub fn run(
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
    stop: Arc<AtomicBool>,
) -> Result<Value, String> {
    run_within(name, source, export, arguments, data, host, stop, None)
}

#[allow(clippy::too_many_arguments)]
pub fn run_within(
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
    stop: Arc<AtomicBool>,
    limit: Option<std::time::Duration>,
) -> Result<Value, String> {
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    let flag = Arc::clone(&stop);
    let deadline = limit.map(|limit| std::time::Instant::now() + limit);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let late = deadline.is_some_and(|deadline| std::time::Instant::now() > deadline);
        if late {
            flag.store(true, Ordering::Relaxed);
        }
        flag.load(Ordering::Relaxed)
    })));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    context.with(|ctx| {
        let result = evaluate(&ctx, name, source, export, arguments, data, host);
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".to_owned());
        }
        result.map_err(|error| describe(&ctx, error))
    })
}

fn evaluate<'js>(
    ctx: &Ctx<'js>,
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
) -> rquickjs::Result<Value> {
    let bridge = Function::new(ctx.clone(), move |function: String, arguments: String| -> String {
        let arguments = serde_json::from_str(&arguments).unwrap_or(Value::Null);
        let answer = match host.call(&function, arguments) {
            Ok(value) => serde_json::json!({ "ok": value }),
            Err(error) => serde_json::json!({ "error": error }),
        };
        answer.to_string()
    })?;
    ctx.globals().set("__host", bridge)?;
    ctx.eval::<(), _>(PRELUDE)?;
    let (module, done) = Module::declare(ctx.clone(), name, source)?.eval()?;
    done.finish::<()>()?;
    let function: Function = module.get(export)?;
    let make: Function = ctx.globals().get("__ctx")?;
    let context: JsValue = make.call((json_to_js(ctx, data)?,))?;
    let mut list = rquickjs::function::Args::new(ctx.clone(), arguments.len() + 1);
    for argument in arguments {
        list.push_arg(json_to_js(ctx, argument)?)?;
    }
    list.push_arg(context)?;
    let mut value: JsValue = function.call_arg(list)?;
    if let Some(promise) = value.as_promise() {
        value = promise.clone().finish::<JsValue>()?;
    }
    js_to_json(ctx, value)
}

pub fn exported(name: &str, source: &str, export: &str) -> Result<Value, String> {
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    runtime.set_interrupt_handler(Some(Box::new(move || std::time::Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    context.with(|ctx| {
        let evaluated = (|| {
            let (module, done) = Module::declare(ctx.clone(), name, source)?.eval()?;
            done.finish::<()>()?;
            let value: JsValue = module.get(export)?;
            js_to_json(&ctx, value)
        })();
        evaluated.map_err(|error| describe(&ctx, error))
    })
}

fn json_to_js<'js>(ctx: &Ctx<'js>, value: &Value) -> rquickjs::Result<JsValue<'js>> {
    ctx.json_parse(value.to_string())
}

fn js_to_json<'js>(ctx: &Ctx<'js>, value: JsValue<'js>) -> rquickjs::Result<Value> {
    if value.is_undefined() {
        return Ok(Value::Null);
    }
    let text = ctx.json_stringify(value)?.map(|text| text.to_string()).transpose()?.unwrap_or_default();
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn describe(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    if matches!(error, rquickjs::Error::Exception) {
        let caught = ctx.catch();
        if let Some(exception) = caught.as_exception() {
            return format!(
                "{}{}",
                exception.message().unwrap_or_default(),
                exception.stack().map(|stack| format!("\n{stack}")).unwrap_or_default()
            );
        }
        return format!("{caught:?}");
    }
    error.to_string()
}

#[cfg(test)]
mod tests;
