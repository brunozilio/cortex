use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use rquickjs::context::EvalOptions;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, Exception, Function, Runtime, Value as JsValue};
use serde_json::Value;

const PRELUDE: &str = include_str!("session_prelude.js");
const POLL: Duration = Duration::from_millis(25);
const MEMORY: usize = 512 * 1024 * 1024;
const STACK: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub running: Duration,
    pub total: Duration,
    pub calls: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { running: Duration::from_secs(30), total: Duration::from_secs(600), calls: 1000 }
    }
}

#[derive(Debug)]
pub struct Evaluate {
    pub code: String,
    pub functions: Vec<String>,
    pub limits: Limits,
    pub stop: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum Message {
    Call(HostCall),
    Done(Evaluation),
}

#[derive(Debug)]
pub struct HostCall {
    pub function: String,
    pub input: Value,
    pub reply: Reply,
}

#[derive(Debug)]
pub struct Reply {
    evaluation: u64,
    id: u32,
    to: mpsc::Sender<Settled>,
}

impl Reply {
    pub fn send(self, result: Result<Value, String>) {
        let _ = self.to.send(Settled { evaluation: self.evaluation, id: self.id, result });
    }
}

#[derive(Debug)]
struct Settled {
    evaluation: u64,
    id: u32,
    result: Result<Value, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub logs: Vec<String>,
    pub outcome: Result<String, String>,
    pub calls: usize,
}

struct Job {
    request: Evaluate,
    messages: mpsc::Sender<Message>,
}

#[derive(Debug)]
pub struct Session {
    jobs: Mutex<mpsc::Sender<Job>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::start()
    }
}

impl Session {
    pub fn start() -> Self {
        Self::with_helpers(String::new())
    }

    pub fn with_helpers(helpers: String) -> Self {
        let (jobs, waiting) = mpsc::channel::<Job>();
        std::thread::spawn(move || serve(&waiting, &helpers));
        Self { jobs: Mutex::new(jobs) }
    }

    pub fn evaluate(&self, request: Evaluate) -> mpsc::Receiver<Message> {
        let (messages, received) = mpsc::channel();
        let gone = |messages: mpsc::Sender<Message>| {
            let outcome = Err("the JavaScript context is gone; start a new session".to_owned());
            let _ = messages.send(Message::Done(Evaluation { logs: Vec::new(), outcome, calls: 0 }));
        };
        match self.jobs.lock() {
            Ok(jobs) => {
                if let Err(mpsc::SendError(job)) = jobs.send(Job { request, messages }) {
                    gone(job.messages);
                }
            }
            Err(_) => gone(messages),
        }
        received
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trip {
    Running,
    Total,
    Stopped,
}

#[derive(Default)]
struct Guard {
    deadline: Option<(Instant, Trip)>,
    stop: Option<Arc<AtomicBool>>,
    tripped: Option<Trip>,
}

struct Active {
    evaluation: u64,
    messages: Option<mpsc::Sender<Message>>,
    settled: mpsc::Sender<Settled>,
    next: u32,
    made: usize,
    cap: usize,
    outstanding: usize,
}

fn serve(jobs: &mpsc::Receiver<Job>, helpers: &str) {
    let started = Runtime::new().and_then(|runtime| Context::full(&runtime).map(|context| (runtime, context)));
    let Ok((runtime, context)) = started else {
        for job in jobs {
            let outcome = Err("the JavaScript engine could not start".to_owned());
            let _ = job.messages.send(Message::Done(Evaluation { logs: Vec::new(), outcome, calls: 0 }));
        }
        return;
    };
    runtime.set_memory_limit(MEMORY);
    runtime.set_max_stack_size(STACK);
    let guard = Arc::new(Mutex::new(Guard::default()));
    let watching = Arc::clone(&guard);
    runtime.set_interrupt_handler(Some(Box::new(move || interrupted(&watching))));
    let (settle, settled) = mpsc::channel::<Settled>();
    let active = Rc::new(RefCell::new(Active {
        evaluation: 0,
        messages: None,
        settled: settle,
        next: 0,
        made: 0,
        cap: 0,
        outstanding: 0,
    }));
    let ready = context.with(|ctx| {
        install(&ctx, &active, helpers).map_err(|error| format!("the helpers failed: {}", failure(&ctx, error)))
    });
    for (number, job) in (1..).zip(jobs) {
        let evaluation = match &ready {
            Ok(()) => {
                let run = Run { guard: &guard, active: &active, settled: &settled, number };
                context.with(|ctx| run.evaluate(&ctx, job.request, job.messages.clone()))
            }
            Err(error) => Evaluation { logs: Vec::new(), outcome: Err(error.clone()), calls: 0 },
        };
        let _ = job.messages.send(Message::Done(evaluation));
    }
}

fn interrupted(guard: &Mutex<Guard>) -> bool {
    let Ok(mut guard) = guard.lock() else { return true };
    if guard.tripped.is_some() {
        return true;
    }
    if guard.stop.as_ref().is_some_and(|stop| stop.load(Ordering::Relaxed)) {
        guard.tripped = Some(Trip::Stopped);
    } else if let Some((deadline, trip)) = guard.deadline
        && Instant::now() > deadline
    {
        guard.tripped = Some(trip);
    }
    guard.tripped.is_some()
}

fn install<'js>(ctx: &Ctx<'js>, active: &Rc<RefCell<Active>>, helpers: &str) -> rquickjs::Result<()> {
    let state = Rc::clone(active);
    let begin = Function::new(ctx.clone(), move |ctx: Ctx<'js>, function: String, input: String| {
        let mut active = state.borrow_mut();
        let Some(messages) = active.messages.clone() else {
            return Err(Exception::throw_message(
                &ctx,
                &format!("{function} did not run: the script that made this call has already ended"),
            ));
        };
        if active.made >= active.cap {
            return Err(Exception::throw_message(
                &ctx,
                &format!("{function} did not run: the script reached its limit of {} tool calls", active.cap),
            ));
        }
        active.next = active.next.wrapping_add(1);
        active.made += 1;
        active.outstanding += 1;
        let reply = Reply { evaluation: active.evaluation, id: active.next, to: active.settled.clone() };
        let input = serde_json::from_str(&input).unwrap_or(Value::Null);
        let _ = messages.send(Message::Call(HostCall { function, input, reply }));
        Ok(active.next)
    })?;
    ctx.globals().set("__begin", begin)?;
    ctx.eval::<(), _>(PRELUDE)?;
    let mut options = EvalOptions::default();
    options.global = true;
    options.strict = true;
    options.filename = Some("helpers".to_owned());
    ctx.eval_with_options::<(), _>(helpers, options)
}

struct Run<'a> {
    guard: &'a Arc<Mutex<Guard>>,
    active: &'a Rc<RefCell<Active>>,
    settled: &'a mpsc::Receiver<Settled>,
    number: u64,
}

struct Clock {
    left: Duration,
    total: Instant,
}

impl Run<'_> {
    fn evaluate(&self, ctx: &Ctx<'_>, request: Evaluate, messages: mpsc::Sender<Message>) -> Evaluation {
        {
            let mut active = self.active.borrow_mut();
            active.evaluation = self.number;
            active.messages = Some(messages);
            active.made = 0;
            active.cap = request.limits.calls;
            active.outstanding = 0;
        }
        if let Ok(mut guard) = self.guard.lock() {
            *guard = Guard { stop: Some(Arc::clone(&request.stop)), ..Guard::default() };
        }
        let mut clock = Clock { left: request.limits.running, total: Instant::now() + request.limits.total };
        let outcome = self.outcome(ctx, &request, &mut clock);
        let outcome = outcome.map_err(|failure| self.explained(failure, &request.limits));
        let logs = ctx
            .globals()
            .get::<_, Function>("__takeLogs")
            .and_then(|take| take.call::<_, Vec<String>>(()))
            .unwrap_or_default();
        if let Ok(drop) = ctx.globals().get::<_, Function>("__drop") {
            let _ = drop.call::<_, ()>(());
        }
        let mut active = self.active.borrow_mut();
        active.messages = None;
        if let Ok(mut guard) = self.guard.lock() {
            *guard = Guard::default();
        }
        Evaluation { logs, outcome, calls: active.made }
    }

    fn explained(&self, failure: String, limits: &Limits) -> String {
        let tripped = self.guard.lock().ok().and_then(|guard| guard.tripped);
        match tripped {
            Some(Trip::Stopped) => "Stopped: the user interrupted the script".to_owned(),
            Some(Trip::Running) => format!(
                "Timed out: the script's own JavaScript ran for {:?}, not counting time spent in tool calls",
                limits.running
            ),
            Some(Trip::Total) => format!("Timed out: the script was stopped {:?} after it started", limits.total),
            None => failure,
        }
    }

    fn outcome(&self, ctx: &Ctx<'_>, request: &Evaluate, clock: &mut Clock) -> Result<String, String> {
        let failed = |error: rquickjs::Error| failure(ctx, error);
        let expose: Function = ctx.globals().get("__expose").map_err(failed)?;
        expose.call::<_, ()>((request.functions.clone(),)).map_err(failed)?;
        let mut options = EvalOptions::default();
        options.global = true;
        options.strict = true;
        options.promise = true;
        options.filename = Some("script".to_owned());
        let started = self.running(clock, || ctx.eval_with_options::<JsValue, _>(request.code.as_str(), options));
        let evaluated = self.settle(ctx, clock, started.map_err(failed)?)?;
        let value = evaluated
            .as_object()
            .map_or(Ok(JsValue::new_undefined(ctx.clone())), |object| object.get("value"))
            .map_err(failed)?;
        let finish: Function = ctx.globals().get("__finish").map_err(failed)?;
        let finished = self.running(clock, || finish.call::<_, JsValue>((value,))).map_err(failed)?;
        let finished = self.settle(ctx, clock, finished)?;
        let show: Function = ctx.globals().get("__show").map_err(failed)?;
        show.call::<_, String>((finished,)).map_err(failed)
    }

    fn running<T>(&self, clock: &mut Clock, work: impl FnOnce() -> T) -> T {
        let begun = Instant::now();
        let (deadline, trip) = if begun + clock.left < clock.total {
            (begun + clock.left, Trip::Running)
        } else {
            (clock.total, Trip::Total)
        };
        if let Ok(mut guard) = self.guard.lock() {
            guard.deadline = Some((deadline, trip));
        }
        let result = work();
        if let Ok(mut guard) = self.guard.lock() {
            guard.deadline = None;
        }
        clock.left = clock.left.saturating_sub(begun.elapsed());
        result
    }

    fn tripped(&self) -> bool {
        self.guard.lock().map_or(true, |guard| guard.tripped.is_some())
    }

    fn settle<'js>(&self, ctx: &Ctx<'js>, clock: &mut Clock, value: JsValue<'js>) -> Result<JsValue<'js>, String> {
        let Some(promise) = value.as_promise().cloned() else { return Ok(value) };
        let failed = |error: rquickjs::Error| failure(ctx, error);
        loop {
            self.running(clock, || while ctx.execute_pending_job() {});
            if self.tripped() {
                return Err("interrupted".to_owned());
            }
            match promise.state() {
                PromiseState::Resolved => return promise.result::<JsValue>().unwrap_or(Ok(value)).map_err(failed),
                PromiseState::Rejected => {
                    return Err(promise.result::<JsValue>().map_or_else(String::new, |result| match result {
                        Ok(reason) => shown_failure(ctx, reason),
                        Err(error) => failure(ctx, error),
                    }));
                }
                PromiseState::Pending => self.wait(ctx, clock)?,
            }
        }
    }

    fn wait(&self, ctx: &Ctx<'_>, clock: &mut Clock) -> Result<(), String> {
        let failed = |error: rquickjs::Error| failure(ctx, error);
        loop {
            if self.active.borrow().outstanding == 0 {
                return Err("The script is waiting for a promise that nothing will settle".to_owned());
            }
            if interrupted(self.guard) {
                return Err("interrupted".to_owned());
            }
            if Instant::now() > clock.total {
                if let Ok(mut guard) = self.guard.lock() {
                    guard.tripped = Some(Trip::Total);
                }
                return Err("interrupted".to_owned());
            }
            match self.settled.recv_timeout(POLL) {
                Ok(settled) if settled.evaluation == self.number => {
                    self.active.borrow_mut().outstanding -= 1;
                    let (ok, body) = match settled.result {
                        Ok(value) => (true, value.to_string()),
                        Err(error) => (false, error),
                    };
                    let deliver: Function = ctx.globals().get("__settle").map_err(failed)?;
                    self.running(clock, || deliver.call::<_, ()>((settled.id, ok, body))).map_err(failed)?;
                    return Ok(());
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err("the agent stopped answering".to_owned()),
            }
        }
    }
}

fn failure(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    if matches!(error, rquickjs::Error::Exception) {
        return shown_failure(ctx, ctx.catch());
    }
    error.to_string()
}

fn shown_failure<'js>(ctx: &Ctx<'js>, reason: JsValue<'js>) -> String {
    ctx.globals()
        .get::<_, Function>("__failure")
        .and_then(|explain| explain.call::<_, String>((reason.clone(),)))
        .unwrap_or_else(|_| format!("{reason:?}"))
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
