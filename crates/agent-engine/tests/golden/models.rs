use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender, unbounded};

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use agent_engine::transcript::{Block, Message, StopReason, Usage};
use serde_json::Value;

use crate::stage::PATIENCE;

pub type Scripted = Result<Reply, ModelError>;

pub fn says(text: &str) -> Scripted {
    ScriptedModel::reply(vec![Block::Text(text.into())])
}

pub fn no_findings() -> Scripted {
    says("{\"findings\": []}")
}

pub fn calls(uses: impl IntoIterator<Item = (&'static str, &'static str, Value)>) -> Scripted {
    ScriptedModel::reply(
        uses.into_iter().map(|(id, name, input)| Block::ToolUse { id: id.into(), name: name.into(), input }).collect(),
    )
}

pub fn call(id: &'static str, name: &'static str, input: Value) -> Scripted {
    calls([(id, name, input)])
}

pub fn stopping(blocks: Vec<Block>, stop: StopReason) -> Scripted {
    Ok(Reply {
        message: Message::assistant(blocks),
        stop,
        usage: Usage { input: 10, output: 5, ..Usage::default() },
        served: None,
    })
}

pub fn measuring(blocks: Vec<Block>, prompt_tokens: u64) -> Scripted {
    let reply = ScriptedModel::reply(blocks)?;
    Ok(Reply { usage: Usage { input: prompt_tokens, output: 50, ..Usage::default() }, ..reply })
}

pub struct Gated {
    partial: &'static str,
    reached: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

pub struct Gate {
    pub reached: Receiver<()>,
    pub release: Sender<()>,
}

impl Gated {
    pub fn streaming(partial: &'static str) -> (Arc<Self>, Gate) {
        let (reached, at_gate) = unbounded();
        let (release, released) = unbounded();
        let model = Self { partial, reached: Mutex::new(reached), release: Mutex::new(released) };
        (Arc::new(model), Gate { reached: at_gate, release })
    }
}

impl ModelApi for Gated {
    fn turn<'a>(
        &'a self,
        _request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            on(Delta::Text(self.partial.into()));
            self.reached.lock().expect("reached").send(()).expect("the test waits at the gate");
            self.release.lock().expect("release").recv_timeout(PATIENCE).expect("the test opens the gate");
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            ScriptedModel::reply(vec![Block::Text(self.partial.into())])
        })
    }
}

pub struct Held {
    script: ScriptedModel,
    turn: usize,
    asked: AtomicUsize,
    reached: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl Held {
    pub fn before_turn(turn: usize, replies: impl IntoIterator<Item = Scripted>) -> (Arc<Self>, Gate) {
        let (reached, at_gate) = unbounded();
        let (release, released) = unbounded();
        let model = Self {
            script: ScriptedModel::new(replies),
            turn,
            asked: AtomicUsize::new(0),
            reached: Mutex::new(reached),
            release: Mutex::new(released),
        };
        (Arc::new(model), Gate { reached: at_gate, release })
    }
}

impl ModelApi for Held {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            if self.asked.fetch_add(1, Ordering::SeqCst) == self.turn {
                self.reached.lock().expect("reached").send(()).expect("the test waits at the gate");
                self.release.lock().expect("release").recv_timeout(PATIENCE).expect("the test opens the gate");
            }
            self.script.answer(request, on)
        })
    }
}

pub struct ByTask {
    main: ScriptedModel,
    tasks: Vec<(&'static str, ScriptedModel)>,
}

impl ByTask {
    pub fn new(main: impl IntoIterator<Item = Scripted>) -> Self {
        Self { main: ScriptedModel::new(main), tasks: Vec::new() }
    }

    pub fn task(mut self, opening: &'static str, replies: impl IntoIterator<Item = Scripted>) -> Self {
        self.tasks.push((opening, ScriptedModel::new(replies)));
        self
    }
}

impl ModelApi for ByTask {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        let opening = request.messages.first().map(Message::text).unwrap_or_default();
        let lane = self.tasks.iter().find(|(task, _)| opening == *task).map_or(&self.main, |(_, lane)| lane);
        Box::pin(async move { lane.answer(request, on) })
    }
}
