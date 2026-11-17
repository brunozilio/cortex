use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{Backend, BackendError, ClientCommand, Envelope, Reply, RequestId, ServerEvent};
use crossbeam_channel::{Receiver, Sender};
use platform::sync::Locked;

use crate::client::dispatch::Dispatcher;

pub(crate) type Command = Envelope<ClientCommand>;
pub(crate) type Asked = Arc<Mutex<Vec<Command>>>;

#[derive(Clone)]
pub(crate) struct Outlet {
    events: Sender<ServerEvent>,
    wake: Sender<()>,
}

impl Outlet {
    pub(crate) fn emit(&self, event: ServerEvent) {
        let _ = self.events.send(event);
        let _ = self.wake.try_send(());
    }

    pub(crate) fn reply(&self, id: RequestId, reply: Reply) {
        self.emit(ServerEvent::Reply { id, reply });
    }
}

type Script = dyn Fn(&Command, &Outlet) + Send;

struct Scripted {
    on: Box<Script>,
    asked: Asked,
    outlet: Outlet,
    events: Receiver<ServerEvent>,
}

impl Backend for Scripted {
    fn send(&self, command: Command) -> Result<(), BackendError> {
        (self.on)(&command, &self.outlet);
        self.asked.locked().push(command);
        Ok(())
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

pub(crate) fn scripted(on: impl Fn(&Command, &Outlet) + Send + 'static) -> (Arc<Dispatcher>, Outlet, Asked) {
    let (events_tx, events) = crossbeam_channel::unbounded();
    let (wake, woken) = crossbeam_channel::bounded(1);
    let outlet = Outlet { events: events_tx, wake };
    let asked = Asked::default();
    let backend = Scripted { on: Box::new(on), asked: Arc::clone(&asked), outlet: outlet.clone(), events };
    (Arc::new(Dispatcher::new(Box::new(backend), woken)), outlet, asked)
}

pub(crate) struct Quiet;

impl Backend for Quiet {
    fn send(&self, _command: Command) -> Result<(), BackendError> {
        Ok(())
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        None
    }
}

pub(crate) fn until<T>(mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "waited five seconds for it");
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub(crate) fn within_five_seconds<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, finished) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = done.send(work());
    });
    finished.recv_timeout(Duration::from_secs(5)).expect("it hung")
}
