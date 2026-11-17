use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub mod session;

use rquickjs::{Context, Ctx, Function, Module, Runtime, Value as JsValue};
use serde_json::Value;

pub trait Host: Send + Sync {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String>;
}

const MOST_MEMORY: usize = 64 << 20;
const MOST_STACK: usize = 1 << 20;
pub(crate) const BEYOND_THE_LIMIT: usize = 4 << 20;

fn on_its_own_stack<T>(work: impl FnOnce() -> T) -> T {
    stacker::grow(MOST_STACK + BEYOND_THE_LIMIT, work)
}

pub(crate) fn runtime() -> rquickjs::Result<Runtime> {
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(MOST_MEMORY);
    runtime.set_max_stack_size(MOST_STACK);
    Ok(runtime)
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

pub const MOST_BUSY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Wall(Duration),
    Busy(Duration),
}

impl Limit {
    fn ended(self) -> String {
        match self {
            Self::Wall(limit) => format!("the script timed out after {limit:?}"),
            Self::Busy(limit) => format!(
                "the script used up its {limit:?} of running time (time waiting on the app does not count); it was stopped"
            ),
        }
    }
}

struct Timed {
    host: Arc<dyn Host>,
    waited: Arc<AtomicU64>,
}

impl Host for Timed {
    fn call(&self, function: &str, arguments: Value) -> Result<Value, String> {
        let started = Instant::now();
        let answer = self.host.call(function, arguments);
        let spent = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.waited.fetch_add(spent, Ordering::Relaxed);
        answer
    }
}

pub fn run(
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
    stop: Arc<AtomicBool>,
) -> Result<Value, String> {
    run_limited(name, source, export, arguments, data, host, stop, Limit::Busy(MOST_BUSY))
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
    limit: Option<Duration>,
) -> Result<Value, String> {
    let limit = limit.map_or(Limit::Busy(MOST_BUSY), Limit::Wall);
    run_limited(name, source, export, arguments, data, host, stop, limit)
}

#[allow(clippy::too_many_arguments)]
pub fn run_limited(
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
    stop: Arc<AtomicBool>,
    limit: Limit,
) -> Result<Value, String> {
    on_its_own_stack(|| run_here(name, source, export, arguments, data, host, stop, limit))
}

#[allow(clippy::too_many_arguments)]
fn run_here(
    name: &str,
    source: &str,
    export: &str,
    arguments: &[Value],
    data: &Value,
    host: Arc<dyn Host>,
    stop: Arc<AtomicBool>,
    limit: Limit,
) -> Result<Value, String> {
    let runtime = runtime().map_err(|error| error.to_string())?;
    let flag = Arc::clone(&stop);
    let timed_out = Arc::new(AtomicBool::new(false));
    let late = Arc::clone(&timed_out);
    let waited = Arc::new(AtomicU64::new(0));
    let host: Arc<dyn Host> = Arc::new(Timed { host, waited: Arc::clone(&waited) });
    let started = Instant::now();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let past = match limit {
            Limit::Wall(limit) => started.elapsed() > limit,
            Limit::Busy(limit) => {
                let waited = Duration::from_nanos(waited.load(Ordering::Relaxed));
                started.elapsed().saturating_sub(waited) > limit
            }
        };
        if past {
            late.store(true, Ordering::Relaxed);
        }
        late.load(Ordering::Relaxed) || flag.load(Ordering::Relaxed)
    })));
    let context = Context::full(&runtime).map_err(|error| error.to_string())?;
    context.with(|ctx| {
        let result = evaluate(&ctx, name, source, export, arguments, data, host);
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".to_owned());
        }
        if timed_out.load(Ordering::Relaxed) {
            return Err(limit.ended());
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
    let mut path = export.split('.');
    let mut value: JsValue = module.get(path.next().unwrap_or(export))?;
    for key in path {
        let object = value.as_object().ok_or_else(|| rquickjs::Error::new_from_js("value", "object"))?.clone();
        value = object.get(key)?;
    }
    let Some(function) = value.as_function().cloned() else {
        return js_to_json(ctx, value);
    };
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
    on_its_own_stack(|| exported_here(name, source, export))
}

fn exported_here(name: &str, source: &str, export: &str) -> Result<Value, String> {
    let runtime = runtime().map_err(|error| error.to_string())?;
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

const EMPTY_FUNCTION: &str = r"(value) => typeof value === 'function'
  && /^(async\s*)?(function\s*[\w$]*\s*)?\([^)]*\)\s*(=>\s*)?\{\s*\}$/.test(value.toString().trim())";

pub fn empty_function(name: &str, source: &str, export: &str) -> bool {
    on_its_own_stack(|| empty_function_here(name, source, export))
}

fn empty_function_here(name: &str, source: &str, export: &str) -> bool {
    let Ok(runtime) = runtime() else { return false };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    runtime.set_interrupt_handler(Some(Box::new(move || std::time::Instant::now() > deadline)));
    let Ok(context) = Context::full(&runtime) else { return false };
    context.with(|ctx| {
        let found = (|| {
            let (module, done) = Module::declare(ctx.clone(), name, source)?.eval()?;
            done.finish::<()>()?;
            let value: JsValue = module.get(export)?;
            let check: Function = ctx.eval(EMPTY_FUNCTION)?;
            check.call::<_, bool>((value,))
        })();
        found.unwrap_or(false)
    })
}

pub fn exports(name: &str, source: &str, export: &str) -> bool {
    on_its_own_stack(|| exports_here(name, source, export))
}

fn exports_here(name: &str, source: &str, export: &str) -> bool {
    let Ok(runtime) = runtime() else { return false };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    runtime.set_interrupt_handler(Some(Box::new(move || std::time::Instant::now() > deadline)));
    let Ok(context) = Context::full(&runtime) else { return false };
    context.with(|ctx| {
        let found = (|| {
            let (module, done) = Module::declare(ctx.clone(), name, source)?.eval()?;
            done.finish::<()>()?;
            let value: JsValue = module.get(export)?;
            Ok::<bool, rquickjs::Error>(!value.is_undefined())
        })();
        found.unwrap_or(false)
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

pub(crate) fn describe(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
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
