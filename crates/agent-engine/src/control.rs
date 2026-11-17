use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};

use domain::{AskId, Decision, PermissionMode};
use platform::sync::Locked;

use crate::permission::Remembered;

#[derive(Debug)]
pub struct Control {
    stop: Arc<AtomicBool>,
    mode: Mutex<PermissionMode>,
    ceiling: Option<PermissionMode>,
    no_prompts: bool,
    remembered: Mutex<Remembered>,
    asks: Mutex<HashMap<AskId, Sender<Decision>>>,
    steered: Arc<Mutex<Vec<String>>>,
    moving: Mutex<Option<crate::agent_runtime::Relocation>>,
    spawned: AtomicUsize,
    children: Mutex<Vec<Weak<Control>>>,
}

impl Control {
    pub fn new(mode: PermissionMode, stop: Arc<AtomicBool>) -> Self {
        Self {
            stop,
            mode: Mutex::new(mode),
            ceiling: None,
            no_prompts: false,
            remembered: Mutex::default(),
            asks: Mutex::default(),
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

    pub fn no_prompts(&self) -> bool {
        self.no_prompts
    }

    pub fn shared_stop(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    pub fn next_agent(&self) -> usize {
        self.spawned.fetch_add(1, Ordering::Relaxed)
    }

    pub fn stop_flag(&self) -> &AtomicBool {
        &self.stop
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Relaxed);
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

    pub fn open(&self, id: AskId) -> Receiver<Decision> {
        let (send, receive) = channel();
        if self.is_stopped() {
            return receive;
        }
        let mut asks = self.asks.locked();
        if self.is_stopped() {
            return receive;
        }
        asks.insert(id, send);
        receive
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_reach_the_open_ask_once() {
        let control = Control::new(PermissionMode::Ask, Arc::default());
        let id = AskId("a".into());
        let waiting = control.open(id.clone());
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
        let waiting = control.open(id.clone());
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
        let waiting = control.open(AskId("a".into()));
        control.cancel();
        assert!(control.is_stopped());
        assert!(waiting.recv().is_err());
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
        let waiting = run.open(id.clone());
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
