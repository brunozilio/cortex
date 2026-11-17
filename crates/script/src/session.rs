use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use rquickjs::context::EvalOptions;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, Exception, Function, Promise, Runtime, Value as JsValue};
use serde_json::Value;

const PRELUDE: &str = include_str!("session_prelude.js");
const POLL: Duration = Duration::from_millis(25);
const MEMORY: usize = 512 * 1024 * 1024;
const STACK: usize = 4 * 1024 * 1024;
const THREAD_STACK: usize = STACK + crate::BEYOND_THE_LIMIT;
const STOPPED: &str = "Stopped: the user interrupted the script";
const STUCK: &str = "The script is waiting for a promise that nothing will settle";

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
    id: u32,
    to: mpsc::Sender<Settled>,
}

impl Reply {
    pub fn send(self, result: Result<Value, String>) {
        let _ = self.to.send(Settled { id: self.id, result });
    }
}

#[derive(Debug)]
struct Settled {
    id: u32,
    result: Result<Value, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub number: u64,
    pub logs: Vec<String>,
    pub outcome: Result<String, String>,
    pub calls: usize,
}

struct Job {
    number: u64,
    request: Evaluate,
    messages: mpsc::Sender<Message>,
}

#[derive(Debug)]
pub struct Submitted {
    pub number: u64,
    pub messages: mpsc::Receiver<Message>,
}

#[derive(Debug)]
pub struct Session {
    jobs: Mutex<(mpsc::Sender<Job>, u64)>,
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
        let thread = std::thread::Builder::new().name("script-session".into()).stack_size(THREAD_STACK);
        let _unanswered_jobs_say_the_context_is_gone = thread.spawn(move || serve(&waiting, &helpers));
        Self { jobs: Mutex::new((jobs, 0)) }
    }

    pub fn evaluate(&self, request: Evaluate) -> mpsc::Receiver<Message> {
        self.submit(request).messages
    }

    pub fn submit(&self, request: Evaluate) -> Submitted {
        let (messages, received) = mpsc::channel();
        let gone = |messages: mpsc::Sender<Message>, number: u64| {
            let outcome = Err("the JavaScript context is gone; start a new session".to_owned());
            let _ = messages.send(Message::Done(Evaluation { number, logs: Vec::new(), outcome, calls: 0 }));
        };
        let Ok(mut jobs) = self.jobs.lock() else {
            gone(messages, 0);
            return Submitted { number: 0, messages: received };
        };
        jobs.1 += 1;
        let number = jobs.1;
        if let Err(mpsc::SendError(job)) = jobs.0.send(Job { number, request, messages }) {
            gone(job.messages, number);
        }
        Submitted { number, messages: received }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trip {
    Running,
    Stopped,
}

#[derive(Default)]
struct Guard {
    deadline: Option<Instant>,
    stops: Vec<Arc<AtomicBool>>,
    tripped: Option<Trip>,
}

struct Account {
    messages: mpsc::Sender<Message>,
    logs: Vec<String>,
    made: usize,
    cap: usize,
}

struct Accounts {
    scripts: BTreeMap<u64, Account>,
    calls: HashMap<u32, u64>,
    next: u32,
    settle: mpsc::Sender<Settled>,
}

impl Accounts {
    fn owner(&self, candidates: &str) -> Option<u64> {
        let named: Vec<u64> = serde_json::from_str(candidates).unwrap_or_default();
        named
            .into_iter()
            .find(|number| self.scripts.contains_key(number))
            .or_else(|| self.scripts.keys().next_back().copied())
    }
}

fn serve(jobs: &mpsc::Receiver<Job>, helpers: &str) {
    let started = Runtime::new().and_then(|runtime| Context::full(&runtime).map(|context| (runtime, context)));
    let Ok((runtime, context)) = started else {
        for job in jobs {
            let outcome = Err("the JavaScript engine could not start".to_owned());
            let _ = job.messages.send(Message::Done(Evaluation { number: 0, logs: Vec::new(), outcome, calls: 0 }));
        }
        return;
    };
    runtime.set_memory_limit(MEMORY);
    runtime.set_max_stack_size(STACK);
    let guard = Arc::new(Mutex::new(Guard::default()));
    let watching = Arc::clone(&guard);
    runtime.set_interrupt_handler(Some(Box::new(move || interrupted(&watching))));
    let (settle, settled) = mpsc::channel::<Settled>();
    let accounts = Rc::new(RefCell::new(Accounts { scripts: BTreeMap::new(), calls: HashMap::new(), next: 0, settle }));
    context.with(|ctx| {
        let ready =
            install(&ctx, &accounts, helpers).map_err(|error| format!("the helpers failed: {}", failure(&ctx, error)));
        let mut driver =
            Driver { ctx: ctx.clone(), guard, accounts, scripts: BTreeMap::new(), ready, timed_out: false };
        driver.run(jobs, &settled);
    });
}

fn interrupted(guard: &Mutex<Guard>) -> bool {
    let Ok(mut guard) = guard.lock() else { return true };
    if guard.tripped.is_some() {
        return true;
    }
    if guard.stops.iter().any(|stop| stop.load(Ordering::Relaxed)) {
        guard.tripped = Some(Trip::Stopped);
    } else if guard.deadline.is_some_and(|deadline| Instant::now() > deadline) {
        guard.tripped = Some(Trip::Running);
    }
    guard.tripped.is_some()
}

fn install<'js>(ctx: &Ctx<'js>, accounts: &Rc<RefCell<Accounts>>, helpers: &str) -> rquickjs::Result<()> {
    let state = Rc::clone(accounts);
    let begin = Function::new(ctx.clone(), move |ctx: Ctx<'js>, function: String, input: String, owners: String| {
        let mut accounts = state.borrow_mut();
        let refused = |message: String| Err(Exception::throw_message(&ctx, &message));
        let Some(owner) = accounts.owner(&owners) else {
            return refused(format!("{function} did not run: the script that made this call has already ended"));
        };
        accounts.next = accounts.next.wrapping_add(1);
        let id = accounts.next;
        let settle = accounts.settle.clone();
        let Some(account) = accounts.scripts.get_mut(&owner) else {
            return refused(format!("{function} did not run: the script that made this call has already ended"));
        };
        if account.made >= account.cap {
            return refused(format!(
                "{function} did not run: the script reached its limit of {} tool calls",
                account.cap
            ));
        }
        account.made += 1;
        let input = serde_json::from_str(&input).unwrap_or(Value::Null);
        let _ = account.messages.send(Message::Call(HostCall { function, input, reply: Reply { id, to: settle } }));
        accounts.calls.insert(id, owner);
        Ok(id)
    })?;
    let state = Rc::clone(accounts);
    let log = Function::new(ctx.clone(), move |owners: String, text: String| {
        let mut accounts = state.borrow_mut();
        if let Some(owner) = accounts.owner(&owners)
            && let Some(account) = accounts.scripts.get_mut(&owner)
        {
            account.logs.push(text);
        }
    })?;
    ctx.globals().set("__begin", begin)?;
    ctx.globals().set("__log", log)?;
    ctx.eval::<(), _>(PRELUDE)?;
    let mut options = EvalOptions::default();
    options.global = true;
    options.strict = true;
    options.filename = Some("helpers".to_owned());
    ctx.eval_with_options::<(), _>(helpers, options)
}

enum Phase<'js> {
    Main(Promise<'js>),
    Finish(Promise<'js>),
}

struct Script<'js> {
    phase: Phase<'js>,
    limits: Limits,
    started: Instant,
    stop: Arc<AtomicBool>,
}

struct Driver<'js> {
    ctx: Ctx<'js>,
    guard: Arc<Mutex<Guard>>,
    accounts: Rc<RefCell<Accounts>>,
    scripts: BTreeMap<u64, Script<'js>>,
    ready: Result<(), String>,
    timed_out: bool,
}

impl<'js> Driver<'js> {
    fn run(&mut self, jobs: &mpsc::Receiver<Job>, settled: &mpsc::Receiver<Settled>) {
        loop {
            let mut moved = false;
            if self.scripts.is_empty() {
                match jobs.recv() {
                    Ok(job) => self.start(job),
                    Err(_) => return,
                }
                moved = true;
            }
            while let Ok(job) = jobs.try_recv() {
                self.start(job);
                moved = true;
            }
            while let Ok(reply) = settled.try_recv() {
                moved |= self.deliver(reply);
            }
            moved |= self.run_jobs();
            moved |= self.check();
            if moved || self.scripts.is_empty() {
                continue;
            }
            if self.accounts.borrow().calls.is_empty() {
                for number in self.scripts.keys().copied().collect::<Vec<_>>() {
                    self.end(number, Err(STUCK.to_owned()));
                }
                continue;
            }
            if let Ok(reply) = settled.recv_timeout(POLL) {
                self.deliver(reply);
            }
        }
    }

    fn segment<T>(&self, running: Duration, work: impl FnOnce() -> T) -> T {
        if let Ok(mut guard) = self.guard.lock() {
            guard.deadline = Some(Instant::now() + running);
        }
        let result = work();
        if let Ok(mut guard) = self.guard.lock() {
            guard.deadline = None;
        }
        result
    }

    fn running(&self) -> Duration {
        self.scripts.values().map(|script| script.limits.running).min().unwrap_or(Limits::default().running)
    }

    fn take_trip(&mut self) {
        let tripped = self.guard.lock().ok().and_then(|mut guard| guard.tripped.take());
        if tripped == Some(Trip::Running) {
            self.timed_out = true;
        }
    }

    fn refresh_stops(&self) {
        if let Ok(mut guard) = self.guard.lock() {
            guard.stops = self.scripts.values().map(|script| Arc::clone(&script.stop)).collect();
        }
    }

    fn start(&mut self, job: Job) {
        let Job { number, request, messages } = job;
        self.accounts
            .borrow_mut()
            .scripts
            .insert(number, Account { messages, logs: Vec::new(), made: 0, cap: request.limits.calls });
        if let Err(error) = &self.ready {
            let error = error.clone();
            return self.finish_now(number, Err(error));
        }
        let ctx = self.ctx.clone();
        let failed = |error: rquickjs::Error| failure(&ctx, error);
        let exposed = ctx
            .globals()
            .get::<_, Function>("__expose")
            .and_then(|expose| expose.call::<_, ()>((request.functions.clone(),)))
            .map_err(failed);
        if let Err(error) = exposed {
            return self.finish_now(number, Err(error));
        }
        let mut options = EvalOptions::default();
        options.global = true;
        options.strict = true;
        options.promise = true;
        options.filename = Some(format!("script-{number}"));
        if let Ok(mut guard) = self.guard.lock() {
            guard.stops.push(Arc::clone(&request.stop));
        }
        let evaluated = self
            .segment(request.limits.running, || ctx.eval_with_options::<JsValue, _>(request.code.as_str(), options));
        self.take_trip();
        self.refresh_stops();
        if request.stop.load(Ordering::Relaxed) {
            return self.finish_now(number, Err(STOPPED.to_owned()));
        }
        match evaluated.and_then(|value| promised(&ctx, value)).map_err(failed) {
            Ok(promise) => {
                let script = Script {
                    phase: Phase::Main(promise),
                    limits: request.limits,
                    started: Instant::now(),
                    stop: request.stop,
                };
                self.scripts.insert(number, script);
                self.refresh_stops();
            }
            Err(error) => {
                let error = if std::mem::take(&mut self.timed_out) { timed_out(&request.limits) } else { error };
                self.finish_now(number, Err(error));
            }
        }
    }

    fn deliver(&mut self, reply: Settled) -> bool {
        if self.accounts.borrow_mut().calls.remove(&reply.id).is_none() {
            return false;
        }
        let ctx = self.ctx.clone();
        let (ok, body) = match reply.result {
            Ok(value) => (true, value.to_string()),
            Err(error) => (false, error),
        };
        let delivered = ctx
            .globals()
            .get::<_, Function>("__settle")
            .and_then(|settle| self.segment(self.running(), || settle.call::<_, ()>((reply.id, ok, body))));
        self.take_trip();
        if delivered.is_err() {
            let _ = ctx.catch();
        }
        true
    }

    fn run_jobs(&mut self) -> bool {
        let mut ran = false;
        loop {
            let running = self.running();
            let ctx = self.ctx.clone();
            if !self.segment(running, || ctx.execute_pending_job()) {
                break;
            }
            ran = true;
            self.take_trip();
        }
        self.take_trip();
        ran
    }

    fn check(&mut self) -> bool {
        let mut moved = false;
        for number in self.scripts.keys().copied().collect::<Vec<_>>() {
            moved |= self.check_one(number);
        }
        self.timed_out = false;
        moved
    }

    fn check_one(&mut self, number: u64) -> bool {
        let Some(script) = self.scripts.get(&number) else { return false };
        if script.stop.load(Ordering::Relaxed) {
            self.end(number, Err(STOPPED.to_owned()));
            return true;
        }
        if script.started.elapsed() > script.limits.total {
            let message = format!("Timed out: the script was stopped {:?} after it started", script.limits.total);
            self.end(number, Err(message));
            return true;
        }
        let limits = script.limits;
        let (promise, finishing) = match &script.phase {
            Phase::Main(promise) => (promise.clone(), false),
            Phase::Finish(promise) => (promise.clone(), true),
        };
        match promise.state() {
            PromiseState::Pending => false,
            PromiseState::Rejected => {
                let reason = match promise.result::<JsValue>() {
                    Some(Ok(reason)) => shown_failure(&self.ctx, reason),
                    Some(Err(error)) => failure(&self.ctx, error),
                    None => String::new(),
                };
                let reason = if self.timed_out { timed_out(&limits) } else { reason };
                self.end(number, Err(reason));
                true
            }
            PromiseState::Resolved if finishing => {
                let shown = promise
                    .result::<JsValue>()
                    .unwrap_or_else(|| Ok(JsValue::new_undefined(self.ctx.clone())))
                    .and_then(|value| {
                        self.ctx
                            .globals()
                            .get::<_, Function>("__show")
                            .and_then(|show| show.call::<_, String>((value,)))
                    })
                    .map_err(|error| failure(&self.ctx, error));
                self.end(number, shown);
                true
            }
            PromiseState::Resolved => {
                let ctx = self.ctx.clone();
                let finished = promise
                    .result::<JsValue>()
                    .unwrap_or_else(|| Ok(JsValue::new_undefined(ctx.clone())))
                    .and_then(|evaluated| {
                        let value: JsValue = evaluated
                            .as_object()
                            .map_or(Ok(JsValue::new_undefined(ctx.clone())), |object| object.get("value"))?;
                        let finish: Function = ctx.globals().get("__finish")?;
                        let finished = self.segment(limits.running, || finish.call::<_, JsValue>((value,)))?;
                        promised(&ctx, finished)
                    });
                self.take_trip();
                match finished {
                    Ok(promise) => {
                        if let Some(script) = self.scripts.get_mut(&number) {
                            script.phase = Phase::Finish(promise);
                        }
                    }
                    Err(error) => {
                        let error = failure(&ctx, error);
                        self.end(number, Err(error));
                    }
                }
                true
            }
        }
    }

    fn end(&mut self, number: u64, outcome: Result<String, String>) {
        self.scripts.remove(&number);
        self.refresh_stops();
        self.finish_now(number, outcome);
    }

    fn finish_now(&mut self, number: u64, outcome: Result<String, String>) {
        let mut accounts = self.accounts.borrow_mut();
        accounts.calls.retain(|_, owner| *owner != number);
        if let Some(account) = accounts.scripts.remove(&number) {
            let evaluation = Evaluation { number, logs: account.logs, outcome, calls: account.made };
            let _ = account.messages.send(Message::Done(evaluation));
        }
    }
}

fn promised<'js>(ctx: &Ctx<'js>, value: JsValue<'js>) -> rquickjs::Result<Promise<'js>> {
    if let Some(promise) = value.as_promise() {
        return Ok(promise.clone());
    }
    let (promise, resolve, _) = Promise::new(ctx)?;
    resolve.call::<_, ()>((value,))?;
    Ok(promise)
}

fn timed_out(limits: &Limits) -> String {
    format!(
        "Timed out: the script's own JavaScript ran for {:?} without a break, not counting time spent in tool calls",
        limits.running
    )
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
