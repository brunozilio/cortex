use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crossbeam_channel::RecvError;

use tokio::sync::oneshot;

use domain::{AskId, Decision, PermissionMode};
use platform::Stop;
use platform::sync::Locked;

use crate::model::CancellationToken;

use domain::permission::Remembered;

type Asks = Arc<Mutex<HashMap<AskId, oneshot::Sender<Decision>>>>;

#[derive(Debug, Clone)]
pub struct Answerer {
    asks: Asks,
    id: AskId,
}

impl Answerer {
    pub fn answer(&self, decision: Decision) -> bool {
        let sender = self.asks.locked().remove(&self.id);
        sender.is_some_and(|sender| sender.send(decision).is_ok())
    }
}

#[derive(Debug)]
pub struct Control {
    stop: Arc<Stop>,
    mode: Mutex<PermissionMode>,
    ceiling: Option<PermissionMode>,
    no_prompts: bool,
    remembered: Mutex<Remembered>,
    asks: Asks,
    steered: Arc<Mutex<Vec<String>>>,
    moving: Mutex<Option<crate::agent_runtime::Relocation>>,
    spawned: AtomicUsize,
    children: Mutex<Vec<Weak<Control>>>,
}

impl Control {
    pub fn new(mode: PermissionMode, stop: Arc<Stop>) -> Self {
        Self {
            stop,
            mode: Mutex::new(mode),
            ceiling: None,
            no_prompts: false,
            remembered: Mutex::default(),
            asks: Arc::default(),
            steered: Arc::default(),
            moving: Mutex::default(),
            spawned: AtomicUsize::new(0),
            children: Mutex::default(),
        }
    }

    pub fn child(&self) -> Arc<Self> {
        self.child_configured(None, false)
    }
    pub fn child_configured(&self, ceiling: Option<PermissionMode>, no_prompts: bool) -> Arc<Self> {
        self.descendant(ceiling, no_prompts, Arc::default())
    }

    pub fn reconfigured(&self, ceiling: Option<PermissionMode>, no_prompts: bool) -> Arc<Self> {
        self.descendant(ceiling, no_prompts, Arc::clone(&self.steered))
    }

    fn descendant(
        &self,
        ceiling: Option<PermissionMode>,
        no_prompts: bool,
        steered: Arc<Mutex<Vec<String>>>,
    ) -> Arc<Self> {
        let mut children = self.children.locked();
        let child = Arc::new(Self {
            ceiling,
            no_prompts: self.no_prompts || no_prompts,
            remembered: Mutex::new(self.remembered()),
            steered,
            ..Self::new(self.mode().capped(ceiling), Arc::default())
        });
        children.push(Arc::downgrade(&child));
        if self.is_stopped() {
            child.cancel();
        }
        child
    }

    pub fn ceiling(&self) -> Option<PermissionMode> {
        self.ceiling
    }

    pub fn no_prompts(&self) -> bool {
        self.no_prompts
    }

    pub fn shared_stop(&self) -> Arc<Stop> {
        self.stop.clone()
    }

    pub fn next_agent(&self) -> usize {
        self.spawned.fetch_add(1, Ordering::Relaxed)
    }

    pub fn stop_flag(&self) -> &Stop {
        &self.stop
    }

    pub fn token(&self) -> &CancellationToken {
        self.stop.token()
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.is_stopped()
    }

    pub fn cancel(&self) {
        self.stop.cancel();
        for child in self.children.locked().iter().filter_map(Weak::upgrade) {
            child.cancel();
        }
        self.asks.locked().clear();
    }

    pub fn mode(&self) -> PermissionMode {
        *self.mode.locked()
    }

    pub fn set_mode(&self, mode: PermissionMode) {
        let children = self.children.locked();
        let mode = mode.capped(self.ceiling);
        *self.mode.locked() = mode;
        for child in children.iter().filter_map(Weak::upgrade) {
            child.set_mode(mode);
        }
    }

    pub fn remembered(&self) -> Remembered {
        self.remembered.locked().clone()
    }

    pub fn remember(&self, apply: impl FnOnce(&mut Remembered)) {
        apply(&mut self.remembered.locked());
    }

    pub fn steer(&self, text: String) {
        self.steered.locked().push(text);
    }

    pub fn move_to(&self, setup: crate::agent_runtime::Relocation) {
        *self.moving.locked() = Some(setup);
    }

    pub fn is_moving(&self) -> bool {
        self.moving.locked().is_some()
    }

    pub fn take_move(&self) -> Option<crate::agent_runtime::Relocation> {
        self.moving.locked().take()
    }

    pub fn take_steered(&self) -> Vec<String> {
        std::mem::take(&mut *self.steered.locked())
    }

    pub fn open(&self, id: AskId) -> Awaited {
        let (send, receive) = oneshot::channel();
        let awaited = Awaited { answer: Some(receive), cancel: self.token().clone() };
        if self.is_stopped() {
            return awaited;
        }
        let mut asks = self.asks.locked();
        if self.is_stopped() {
            return awaited;
        }
        asks.insert(id, send);
        awaited
    }

    pub fn answerer(&self, id: &AskId) -> Answerer {
        Answerer { asks: Arc::clone(&self.asks), id: id.clone() }
    }

    pub fn answer(&self, id: &AskId, decision: Decision) {
        let sender = self.asks.locked().remove(id);
        for child in self.children.locked().iter().filter_map(Weak::upgrade) {
            child.answer(id, decision.clone());
        }
        if let Some(sender) = sender {
            let _ = sender.send(decision);
        }
    }
}

#[derive(Debug)]
pub struct Awaited {
    answer: Option<oneshot::Receiver<Decision>>,
    cancel: CancellationToken,
}

impl Awaited {
    pub fn recv(&mut self) -> Result<Decision, RecvError> {
        let answer = self.answer.take().ok_or(RecvError)?;
        let cancel = self.cancel.clone();
        let waited = platform::runtime::block_on(async move {
            tokio::select! {
                biased;
                () = cancel.cancelled() => Err(RecvError),
                answered = answer => answered.map_err(|_| RecvError),
            }
        });
        waited.unwrap_or(Err(RecvError))
    }

    pub fn try_recv(&mut self) -> Result<Decision, RecvError> {
        let answer = self.answer.as_mut().ok_or(RecvError)?;
        let answered = answer.try_recv().map_err(|_| RecvError)?;
        self.answer = None;
        Ok(answered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_reach_the_open_ask_once() {
        let control = Control::new(PermissionMode::Ask, Arc::default());
        let id = AskId("a".into());
        let mut waiting = control.open(id.clone());
        control.answer(&id, Decision::Allow);
        control.answer(&id, Decision::Deny);
        assert_eq!(waiting.recv(), Ok(Decision::Allow));
        assert!(waiting.try_recv().is_err());
    }

    fn poison<T: Send>(lock: &Mutex<T>) {
        let crashed = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _held = lock.lock();
                    panic!("poison the lock");
                })
                .join()
        });
        assert!(crashed.is_err() && lock.is_poisoned());
    }

    #[test]
    fn a_crashed_thread_never_loosens_the_permission_mode() {
        let control = Control::new(PermissionMode::Plan, Arc::default());
        poison(&control.mode);
        assert_eq!(control.mode(), PermissionMode::Plan);
        control.set_mode(PermissionMode::Ask);
        assert_eq!(control.mode(), PermissionMode::Ask);
    }

    #[test]
    fn a_crashed_thread_still_lets_a_run_be_cancelled_and_answered() {
        let control = Control::new(PermissionMode::Ask, Arc::default());
        let child = control.child();
        let id = AskId("a".into());
        let mut waiting = control.open(id.clone());
        poison(&control.children);
        poison(&control.asks);
        control.answer(&id, Decision::Allow);
        assert_eq!(waiting.recv(), Ok(Decision::Allow));
        control.cancel();
        assert!(child.is_stopped());
    }

    #[test]
    fn cancelling_releases_waiting_asks() {
        let control = Control::new(PermissionMode::Ask, Arc::default());
        let mut waiting = control.open(AskId("a".into()));
        control.cancel();
        assert!(control.is_stopped());
        assert!(waiting.recv().is_err());
    }

    #[test]
    fn a_question_being_waited_on_ends_the_moment_its_run_is_cancelled() {
        let control = Arc::new(Control::new(PermissionMode::Ask, Arc::default()));
        let parent = Control::new(PermissionMode::Ask, Arc::default());
        let child = parent.child();
        let mut asked = control.open(AskId("a".into()));
        let mut asked_of_child = child.open(AskId("b".into()));
        let canceller = Arc::clone(&control);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            canceller.cancel();
            parent.cancel();
        });
        let started = std::time::Instant::now();
        assert!(asked.recv().is_err());
        assert!(asked_of_child.recv().is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    }
}
#[cfg(test)]
mod child_policy_tests {
    use super::*;
    #[test]
    fn child_permission_ceiling_survives_parent_mode_changes() {
        let parent = Control::new(PermissionMode::Bypass, Arc::default());
        let read_only = parent.child_configured(Some(PermissionMode::Plan), false);
        let inherited = parent.child();
        assert_eq!(read_only.mode(), PermissionMode::Plan);
        parent.set_mode(PermissionMode::Ask);
        assert_eq!(inherited.mode(), PermissionMode::Ask);
        assert_eq!(read_only.mode(), PermissionMode::Plan);
        parent.set_mode(PermissionMode::Bypass);
        assert_eq!(inherited.mode(), PermissionMode::Bypass);
        assert_eq!(read_only.mode(), PermissionMode::Plan);
        parent.cancel();
        assert!(read_only.is_stopped() && inherited.is_stopped());
    }

    #[test]
    fn a_reconfigured_control_hears_what_its_handle_is_told_and_a_sub_agent_does_not() {
        let handle = Control::new(PermissionMode::Ask, Arc::default());
        let run = handle.reconfigured(Some(PermissionMode::AcceptEdits), false);
        let sub_agent = run.child();
        handle.steer("also the tests".into());
        assert!(sub_agent.take_steered().is_empty());
        assert_eq!(run.take_steered(), ["also the tests"]);
        assert!(handle.take_steered().is_empty());

        let id = AskId("a".into());
        let mut waiting = run.open(id.clone());
        handle.answer(&id, Decision::Allow);
        assert_eq!(waiting.recv(), Ok(Decision::Allow));

        handle.set_mode(PermissionMode::Bypass);
        assert_eq!(run.mode(), PermissionMode::AcceptEdits);
        handle.set_mode(PermissionMode::Plan);
        assert_eq!((run.mode(), sub_agent.mode()), (PermissionMode::Plan, PermissionMode::Plan));

        handle.cancel();
        assert!(run.is_stopped() && sub_agent.is_stopped());
    }
}
