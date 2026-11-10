//! What the user can do to a run while it works: cancel it, answer its
//! asks, change its permission mode. Shared between the run's thread and
//! its [`RunHandle`](superfik_engine::RunHandle).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};

use superfik_core::{AskId, Decision, PermissionMode};

use crate::permission::Remembered;

#[derive(Debug)]
pub struct Control {
    stop: Arc<AtomicBool>,
    mode: Mutex<PermissionMode>,
    ceiling: Option<PermissionMode>,
    no_prompts: bool,
    remembered: Mutex<Remembered>,
    asks: Mutex<HashMap<AskId, Sender<Decision>>>,
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
            spawned: AtomicUsize::new(0),
            children: Mutex::default(),
        }
    }

    pub fn child(&self) -> Arc<Self> {
        self.child_configured(None, false)
    }
    pub fn child_configured(&self, ceiling: Option<PermissionMode>, no_prompts: bool) -> Arc<Self> {
        let mut children = self.children.lock().expect("child controls");
        let mut child = Self::new(self.mode(), Arc::default());
        child.ceiling = ceiling;
        child.no_prompts = self.no_prompts || no_prompts;
        *child.mode.get_mut().expect("new mode") = self.mode().capped(ceiling);
        *child.remembered.get_mut().expect("new permissions") = self.remembered();
        let child = Arc::new(child);
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
        for child in self.children.lock().expect("children").iter().filter_map(Weak::upgrade) {
            child.cancel();
        }
        if let Ok(mut asks) = self.asks.lock() {
            asks.clear();
        }
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode.lock().map(|mode| *mode).unwrap_or_default()
    }

    pub fn set_mode(&self, mode: PermissionMode) {
        let children = self.children.lock().expect("child controls");
        let mode = mode.capped(self.ceiling);
        if let Ok(mut current) = self.mode.lock() {
            *current = mode;
        }
        for child in children.iter().filter_map(Weak::upgrade) {
            child.set_mode(mode);
        }
    }

    pub fn remembered(&self) -> Remembered {
        self.remembered.lock().map(|remembered| remembered.clone()).unwrap_or_default()
    }

    pub fn remember(&self, apply: impl FnOnce(&mut Remembered)) {
        if let Ok(mut remembered) = self.remembered.lock() {
            apply(&mut remembered);
        }
    }

    pub fn open(&self, id: AskId) -> Receiver<Decision> {
        let (send, receive) = channel();
        if self.is_stopped() {
            return receive;
        }
        if let Ok(mut asks) = self.asks.lock() {
            if self.is_stopped() {
                return receive;
            }
            asks.insert(id, send);
        }
        receive
    }

    pub fn answer(&self, id: &AskId, decision: Decision) {
        let sender = self.asks.lock().ok().and_then(|mut asks| asks.remove(id));
        for child in self.children.lock().expect("children").iter().filter_map(Weak::upgrade) {
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
}
