use std::collections::HashMap;
use std::io::Read as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use platform::Child;
use platform::child::{GRACE, Leftovers, Spawn};
use platform::sync::Locked;

use crate::tools::Workspace;

mod output;
mod tools;
mod wait;
mod watch;
use output::{KEPT, Unread, capped};
pub use tools::{BashOutput, KillShell};
pub(crate) use wait::{Waited, wait};
pub(crate) use watch::{Watch, ended_source, watch};

const FORGOTTEN_AFTER: Duration = Duration::from_secs(60 * 60);

struct Shell {
    client: u64,
    command: String,
    child: Child,
    output: Arc<Mutex<Vec<u8>>>,
    dropped: Arc<AtomicUsize>,
    read: usize,
    started: Instant,
    exit: Option<String>,
    ended: Option<Instant>,
    watched: bool,
    told: bool,
    readers: Vec<std::thread::JoinHandle<()>>,
}

impl Shell {
    fn unread(&self) -> Unread {
        Unread::of(&self.output.locked(), self.dropped.load(Ordering::Relaxed), self.read)
    }

    fn take_unread(&mut self) -> Unread {
        let mut kept = self.output.locked();
        let dropped = self.dropped.load(Ordering::Relaxed);
        let unread = Unread::of(&kept, dropped, self.read);
        self.read = dropped + kept.len();
        if kept.len() > KEPT {
            let read = kept.len() - KEPT;
            kept.drain(..read);
            self.dropped.fetch_add(read, Ordering::Relaxed);
        }
        unread
    }

    fn mark_read(&mut self) {
        self.read = self.written();
    }

    fn written(&self) -> usize {
        self.dropped.load(Ordering::Relaxed) + self.output.locked().len()
    }

    fn collected(&self) -> bool {
        self.exit.is_some()
            && self.readers.iter().all(std::thread::JoinHandle::is_finished)
            && self.read >= self.written()
            && (self.told || !self.watched)
    }

    fn stop(self) {
        self.child.terminate(GRACE);
    }
}

fn shells() -> &'static Mutex<HashMap<String, Shell>> {
    static SHELLS: OnceLock<Mutex<HashMap<String, Shell>>> = OnceLock::new();
    SHELLS.get_or_init(Mutex::default)
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, Shell>> {
    shells().locked()
}

pub(crate) fn start(
    workspace: &Workspace,
    summary: &str,
    command: &mut std::process::Command,
) -> std::io::Result<String> {
    let client = workspace.credentials.client;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    forget_finished(Instant::now());
    let (reader, writer) = std::io::pipe()?;
    command.stdout(writer.try_clone()?).stderr(writer);
    let spawned = Child::spawn_with(command, Spawn { leftovers: Leftovers::Kept, ..Spawn::default() });
    command.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let child = spawned?;
    let output = Arc::new(Mutex::new(Vec::new()));
    let pipe = boxed(reader);
    let dropped = Arc::new(AtomicUsize::new(0));
    let readers = vec![{
        let (output, dropped) = (Arc::clone(&output), Arc::clone(&dropped));
        platform::named::spawn("shell-output", move || collect(pipe, &output, &dropped))
    }];
    let id = format!("bash_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let shell = Shell {
        client,
        command: summary.to_owned(),
        child,
        output,
        dropped,
        read: 0,
        started: Instant::now(),
        exit: None,
        ended: None,
        watched: false,
        told: false,
        readers,
    };
    lock().insert(id.clone(), shell);
    workspace.tracking.started_shell(&id);
    Ok(id)
}

fn boxed(pipe: impl std::io::Read + Send + 'static) -> Box<dyn std::io::Read + Send> {
    Box::new(pipe)
}

fn collect(mut pipe: Box<dyn std::io::Read + Send>, output: &Mutex<Vec<u8>>, dropped: &AtomicUsize) {
    let mut chunk = [0_u8; 8192];
    while let Ok(read) = pipe.read(&mut chunk) {
        if read == 0 {
            return;
        }
        let mut kept = output.locked();
        kept.extend_from_slice(&chunk[..read]);
        dropped.fetch_add(capped(&mut kept), Ordering::Relaxed);
    }
}

fn taken(client: u64, id: &str) -> Option<Shell> {
    let mut shells = lock();
    if !shells.get(id).is_some_and(|shell| shell.client == client) {
        return None;
    }
    shells.remove(id)
}

pub fn kill_client(client: u64) {
    let leaving: Vec<Shell> = {
        let mut shells = lock();
        let ids: Vec<String> =
            shells.iter().filter(|(_, shell)| shell.client == client).map(|(id, _)| id.clone()).collect();
        ids.iter().filter_map(|id| shells.remove(id)).collect()
    };
    leaving.into_iter().for_each(Shell::stop);
}

pub(crate) fn started_by(client: u64, id: &str) -> bool {
    let shell = lock().get(id).is_some_and(|shell| shell.client == client);
    shell || crate::tools::watch::list(client).iter().any(|(watched, _)| watched == id)
}

pub fn kill(client: u64, id: &str) -> bool {
    stopped(client, id).is_some()
}

fn stopped(client: u64, id: &str) -> Option<String> {
    let shell = taken(client, id)?;
    let command = shell.command.clone();
    shell.stop();
    Some(command)
}

pub fn list(client: u64) -> Vec<(String, String, bool)> {
    forget_finished(Instant::now());
    let mut shells = lock();
    let mut listed: Vec<_> = shells
        .iter_mut()
        .filter(|(_, shell)| shell.client == client)
        .map(|(id, shell)| (id.clone(), shell.command.clone(), refresh(shell).is_none()))
        .collect();
    listed.sort();
    listed
}

pub fn tail(client: u64, id: &str, most: usize) -> Option<String> {
    let shells = lock();
    let shell = shells.get(id).filter(|shell| shell.client == client)?;
    let kept = shell.output.locked();
    let from = kept.len().saturating_sub(most);
    Some(String::from_utf8_lossy(&kept[from..]).trim().to_owned())
}

fn refresh(shell: &mut Shell) -> Option<String> {
    if shell.exit.is_none()
        && let Some(status) = shell.child.try_wait()
    {
        shell.exit = Some(status.code().map_or_else(|| "killed by a signal".to_owned(), |c| format!("exit code {c}")));
        shell.ended = Some(Instant::now());
    }
    shell.exit.clone()
}

fn forget_finished(now: Instant) {
    forget_finished_of(now, None);
}

fn forget_finished_of(now: Instant, client: Option<u64>) {
    let mut shells = lock();
    for shell in shells.values_mut() {
        refresh(shell);
    }
    shells.retain(|_, shell| {
        let theirs = client.is_none_or(|client| shell.client == client);
        !(theirs && shell.ended.is_some_and(|at| now.saturating_duration_since(at) >= FORGOTTEN_AFTER))
    });
}

#[cfg(all(test, unix))]
mod tests;
