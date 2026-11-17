use std::collections::VecDeque;
use std::io::{self, Read};
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crossbeam_channel::{Receiver, Sender, unbounded};
use std::time::{Duration, Instant};

use crate::sync::Locked;
use crate::{Tree, lost};

pub const GRACE: Duration = Duration::from_secs(2);
pub const STDERR_KEPT: usize = 64 * 1024;

const POLL: Duration = Duration::from_millis(10);
const TAIL_WAIT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Stderr {
    #[default]
    AsConfigured,
    Tail,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Leftovers {
    #[default]
    Ended,
    Kept,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spawn {
    pub stderr: Stderr,
    pub leftovers: Leftovers,
}

enum Life {
    Running(Tree),
    Ended(ExitStatus),
}

pub struct Child {
    id: u32,
    life: Mutex<Life>,
    leftovers: Leftovers,
    stderr: Option<Tail>,
}

impl std::fmt::Debug for Child {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Child").field("id", &self.id).finish_non_exhaustive()
    }
}

impl Child {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with(command, Spawn::default())
    }

    pub fn spawn_with(command: &mut Command, spawn: Spawn) -> io::Result<Self> {
        if spawn.stderr == Stderr::Tail {
            command.stderr(Stdio::piped());
        }
        let mut tree = Tree::spawn(command)?;
        let stderr = match spawn.stderr {
            Stderr::Tail => tree.child().stderr.take().map(Tail::following),
            Stderr::AsConfigured => None,
        };
        Ok(Self { id: tree.id(), life: Mutex::new(Life::Running(tree)), leftovers: spawn.leftovers, stderr })
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn take_stdin(&self) -> Option<ChildStdin> {
        self.running(|tree| tree.child().stdin.take())
    }

    pub fn take_stdout(&self) -> Option<ChildStdout> {
        self.running(|tree| tree.child().stdout.take())
    }

    pub fn take_stderr(&self) -> Option<ChildStderr> {
        self.running(|tree| tree.child().stderr.take())
    }

    fn running<T>(&self, take: impl FnOnce(&mut Tree) -> Option<T>) -> Option<T> {
        match &mut *self.life.locked() {
            Life::Running(tree) => take(tree),
            Life::Ended(_) => None,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.try_wait().is_none()
    }

    pub fn try_wait(&self) -> Option<ExitStatus> {
        let mut life = self.life.locked();
        let tree = match &mut *life {
            Life::Ended(status) => return Some(*status),
            Life::Running(tree) => tree,
        };
        if !tree.has_exited() {
            return None;
        }
        let status = reaped(tree, self.leftovers == Leftovers::Ended);
        *life = Life::Ended(status);
        Some(status)
    }

    pub fn wait_until(&self, deadline: Instant) -> Option<ExitStatus> {
        loop {
            match self.try_wait() {
                Some(status) => return Some(status),
                None if Instant::now() >= deadline => return None,
                None => std::thread::sleep(POLL),
            }
        }
    }

    pub fn terminate(&self, grace: Duration) -> ExitStatus {
        let mut life = self.life.locked();
        let tree = match &mut *life {
            Life::Ended(status) => return *status,
            Life::Running(tree) => tree,
        };
        if !grace.is_zero() && !tree.has_exited() && tree.ask_to_stop() {
            let until = Instant::now() + grace;
            while !tree.has_exited() && Instant::now() < until {
                std::thread::sleep(POLL);
            }
        }
        let status = reaped(tree, true);
        *life = Life::Ended(status);
        status
    }

    pub fn kill(&self) -> ExitStatus {
        self.terminate(Duration::ZERO)
    }

    pub fn stderr_tail(&self) -> String {
        let Some(tail) = &self.stderr else { return String::new() };
        if !self.is_alive() {
            tail.settle(TAIL_WAIT);
        }
        tail.text()
    }
}

fn reaped(tree: &mut Tree, ending_the_group: bool) -> ExitStatus {
    if ending_the_group {
        tree.kill();
    }
    tree.child().wait().unwrap_or_else(|_| lost())
}

impl Drop for Child {
    fn drop(&mut self) {
        let life = std::mem::replace(&mut *self.life.locked(), Life::Ended(lost()));
        let Life::Running(mut tree) = life else { return };
        if tree.has_exited() {
            reaped(&mut tree, self.leftovers == Leftovers::Ended);
            return;
        }
        if !tree.ask_to_stop() {
            tree.kill();
        }
        if let Err(mut tree) = hand_over(tree) {
            reaped(&mut tree, true);
        }
    }
}

fn hand_over(tree: Tree) -> Result<(), Tree> {
    static REAPER: OnceLock<Option<Sender<Tree>>> = OnceLock::new();
    let reaper = REAPER.get_or_init(|| {
        let (leaving, left) = unbounded();
        std::thread::Builder::new().name("child-reaper".to_owned()).spawn(move || reap(&left)).ok().map(|_| leaving)
    });
    match reaper {
        Some(reaper) => reaper.send(tree).map_err(|refused| refused.0),
        None => Err(tree),
    }
}

fn reap(left: &Receiver<Tree>) {
    let mut held: Vec<(Tree, Instant)> = Vec::new();
    loop {
        let next = if held.is_empty() { left.recv().ok() } else { left.recv_timeout(POLL).ok() };
        match next {
            Some(tree) => held.push((tree, Instant::now() + GRACE)),
            None if held.is_empty() => return,
            None => {}
        }
        held.retain_mut(|(tree, until)| {
            if !tree.has_exited() && Instant::now() < *until {
                return true;
            }
            reaped(tree, true);
            false
        });
    }
}

struct Tail {
    kept: Arc<Mutex<VecDeque<u8>>>,
    ended: Arc<AtomicBool>,
}

impl Tail {
    fn following(mut pipe: impl Read + Send + 'static) -> Self {
        let tail = Self { kept: Arc::default(), ended: Arc::default() };
        let (kept, ended) = (Arc::clone(&tail.kept), Arc::clone(&tail.ended));
        let follow = move || {
            let mut chunk = [0_u8; 4096];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => keep(&kept, &chunk[..read], STDERR_KEPT),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            ended.store(true, Ordering::Relaxed);
        };
        if std::thread::Builder::new().name("child-stderr".to_owned()).spawn(follow).is_err() {
            tail.ended.store(true, Ordering::Relaxed);
        }
        tail
    }

    fn settle(&self, wait: Duration) {
        let until = Instant::now() + wait;
        while !self.ended.load(Ordering::Relaxed) && Instant::now() < until {
            std::thread::sleep(POLL);
        }
    }

    fn text(&self) -> String {
        let bytes: Vec<u8> = self.kept.locked().iter().copied().collect();
        String::from_utf8_lossy(&bytes).trim().to_owned()
    }
}

pub fn last_lines(text: &str, most: usize) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|line| !line.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(most)..].join("\n")
}

fn keep(kept: &Mutex<VecDeque<u8>>, new: &[u8], most: usize) {
    let mut kept = kept.locked();
    kept.extend(new);
    let over = kept.len().saturating_sub(most);
    kept.drain(..over);
}

#[cfg(test)]
#[cfg(unix)]
#[path = "child_tests.rs"]
mod tests;
