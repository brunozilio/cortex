//! Agent operations over native CDP; separate from the vault's command ids.
mod actions;
mod ax;
mod reply;

use ax::{Node, References};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use superfik_core::BrowserAnswer;

#[derive(Clone)]
pub(super) enum Operation {
    Snapshot(usize),
    Click(u32),
    Type(u32, String, bool),
    Evaluate(String),
    Upload(u32, Vec<String>),
    Dialog(bool, Option<String>),
}

enum State {
    Idle,
    Setup,
    Frames,
    Ax,
    Scroll(Node),
    Resolve(Node),
    Check(Node, String),
    Quads(Node),
    Insert(Node, bool),
    Release,
    Evaluate,
    Upload,
    Intercept(Node),
    Chooser,
    Dialog,
}

pub(super) struct Automation {
    next: u64,
    expected: u64,
    state: State,
    operation: Operation,
    refs: References,
    sessions: HashMap<String, (String, String)>,
    frames: VecDeque<(String, String)>,
    session: String,
    frame: String,
    chooser: Option<Node>,
    lines: Vec<String>,
    pub outbox: Vec<String>,
    pub answer: Option<Result<BrowserAnswer, String>>,
    initialized: bool,
    dialog: Option<Value>,
}

impl Default for Automation {
    fn default() -> Self {
        Self {
            next: 1_000_000_000,
            expected: 0,
            state: State::Idle,
            operation: Operation::Snapshot(0),
            refs: References::default(),
            sessions: HashMap::new(),
            frames: VecDeque::new(),
            session: String::new(),
            frame: String::new(),
            chooser: None,
            lines: Vec::new(),
            outbox: Vec::new(),
            answer: None,
            initialized: false,
            dialog: None,
        }
    }
}

impl Automation {
    pub fn begin(&mut self, operation: Operation) -> Result<(), String> {
        self.answer = None;
        if self.has_dialog() && !matches!(operation, Operation::Dialog(..)) {
            self.finish(Ok(self.dialog_summary()));
            return Ok(());
        }
        if !self.initialized {
            self.send("Page.enable", json!({}), "", State::Idle);
            self.send("DOM.enable", json!({}), "", State::Idle);
            self.initialized = true;
        }
        self.operation = operation.clone();
        match operation {
            Operation::Dialog(accept, prompt) => {
                if !self.has_dialog() {
                    return Err("No JavaScript dialog is open.".into());
                }
                self.send(
                    "Page.handleJavaScriptDialog",
                    json!({"accept":accept,"promptText":prompt.unwrap_or_default()}),
                    "",
                    State::Dialog,
                );
            }
            Operation::Snapshot(offset) if offset > 0 => {
                if self.lines.is_empty() {
                    return Err("Take a fresh browser_snapshot before requesting another page.".into());
                }
                self.finish(Ok(BrowserAnswer::text(ax::window(&self.lines, offset))));
            }
            Operation::Snapshot(_) => {
                self.refs.begin();
                self.lines.clear();
                self.send(
                    "Target.setAutoAttach",
                    json!({"autoAttach":true,"waitForDebuggerOnStart":false,"flatten":true}),
                    "",
                    State::Setup,
                );
            }
            Operation::Click(reference) | Operation::Type(reference, ..) | Operation::Upload(reference, ..) => {
                let node = self.refs.node(reference)?;
                self.send(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({"backendNodeId":node.backend}),
                    &node.session.clone(),
                    State::Scroll(node),
                );
            }
            Operation::Evaluate(expression) => self.send(
                "Runtime.evaluate",
                json!({"expression":expression,"awaitPromise":true,"returnByValue":true,"userGesture":true}),
                "",
                State::Evaluate,
            ),
        }
        Ok(())
    }

    fn send(&mut self, method: &str, params: Value, session: &str, state: State) {
        self.next += 1;
        self.expected = self.next;
        self.state = state;
        let mut command = json!({"id":self.next,"method":method,"params":params});
        if !session.is_empty() {
            command["sessionId"] = session.into();
        }
        self.outbox.push(command.to_string());
    }

    pub fn event(&mut self, method: &str, params: &Value) {
        if method == "Page.javascriptDialogOpening" {
            self.dialog = Some(
                json!({"type":params["type"],"message":params["message"],"default_prompt":params["defaultPrompt"]}),
            );
            if self.expected != 0 && !matches!(self.operation, Operation::Dialog(..)) {
                self.finish(Ok(self.dialog_summary()));
            }
        }
        if method == "Page.javascriptDialogClosed" {
            self.dialog = None;
        }
        if method == "Page.fileChooserOpened"
            && let (Some(node), Operation::Upload(_, paths)) = (&self.chooser, &self.operation)
            && params["frameId"].as_str() == Some(&node.frame)
            && let Some(backend) = params["backendNodeId"].as_u64()
        {
            let session = node.session.clone();
            self.send("DOM.setFileInputFiles", json!({"backendNodeId":backend,"files":paths}), &session, State::Upload);
        }
        if method == "Target.attachedToTarget"
            && params["targetInfo"]["type"] == "iframe"
            && let (Some(frame), Some(session), Some(url)) = (
                params["targetInfo"]["targetId"].as_str(),
                params["sessionId"].as_str(),
                params["targetInfo"]["url"].as_str(),
            )
        {
            self.sessions.insert(frame.into(), (session.into(), url.into()));
        }
        if method == "Page.frameNavigated" && params["frame"].get("parentId").is_none() {
            self.refs.invalidate();
            self.lines.clear();
        }
        if method == "Target.detachedFromTarget" {
            self.sessions.retain(|_, (session, _)| params["sessionId"].as_str() != Some(session));
        }
    }

    fn mouse(&mut self, x: f64, y: f64, session: &str) {
        for (kind, button, buttons, count) in
            [("mouseMoved", "none", 0, 0), ("mousePressed", "left", 1, 1), ("mouseReleased", "left", 0, 1)]
        {
            self.send(
                "Input.dispatchMouseEvent",
                json!({"type":kind,"x":x,"y":y,"button":button,"buttons":buttons,"clickCount":count}),
                session,
                State::Release,
            );
        }
    }
    fn next_frame(&mut self) {
        if let Some((frame, url)) = self.frames.pop_front() {
            self.frame = frame.clone();
            self.lines.push(format!("Frame: {url}"));
            self.session = self.sessions.get(&frame).map(|(session, _)| session.clone()).unwrap_or_default();
            self.send("Accessibility.getFullAXTree", json!({"frameId":frame}), &self.session.clone(), State::Ax);
        } else {
            self.refs.finish();
            let offset = match self.operation {
                Operation::Snapshot(offset) => offset,
                _ => 0,
            };
            self.finish(Ok(BrowserAnswer::text(ax::window(&self.lines, offset))));
        }
    }
    fn finish(&mut self, answer: Result<BrowserAnswer, String>) {
        self.restore_chooser();
        self.state = State::Idle;
        self.expected = 0;
        self.answer = Some(answer);
    }
    pub fn cancel(&mut self) {
        self.state = State::Idle;
        self.expected = 0;
        self.answer = None;
        self.outbox.clear();
        self.restore_chooser();
    }
    pub fn contains(&self, text: &str) -> bool {
        self.lines.iter().any(|line| line.contains(text))
    }
    pub fn visible(&self, reference: u32) -> bool {
        self.refs.visible(reference)
    }
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }
    pub fn dialog_summary(&self) -> BrowserAnswer {
        BrowserAnswer::text(json!({"dialog":self.dialog,"next":"Use browser_dialog with an explicit accept/dismiss decision when a dialog is open."}).to_string())
    }
    fn restore_chooser(&mut self) {
        if let Some(node) = self.chooser.take() {
            self.next += 1;
            let mut command =
                json!({"id":self.next,"method":"Page.setInterceptFileChooserDialog","params":{"enabled":false}});
            if !node.session.is_empty() {
                command["sessionId"] = node.session.into();
            }
            self.outbox.push(command.to_string());
        }
    }
}
