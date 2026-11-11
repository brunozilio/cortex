//! Watching a background command for its client's inbox: a monitor's new
//! lines as they come (batched every half second), and every command's end.
//! A command stopped with `KillShell` (or by its client leaving) says
//! nothing more.

use std::time::Duration;

use superfik_core::inbox::Notice;

use super::{lock, refresh};
use crate::tools::inbox;

/// How often a watched command is looked at.
const EVERY: Duration = Duration::from_millis(500);
/// What one batch of a monitor's lines carries at most.
const BATCH: usize = 4_000;

/// What to report of a background command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Watch {
    /// Only that it ended (a command left running).
    End,
    /// Each new line, and that it ended (`Monitor`).
    Lines,
}

/// Watches background command `id` of `client` on a thread of its own.
pub(crate) fn watch(id: String, client: u64, what: Watch) {
    std::thread::spawn(move || {
        let mut seen: usize = 0;
        let mut partial = String::new();
        loop {
            std::thread::sleep(EVERY);
            let (new, exit, command) = {
                let mut shells = lock();
                let Some(shell) = shells.get_mut(&id) else { return };
                let exit = refresh(shell);
                let kept = shell.output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                // `seen` counts every byte ever printed; the buffer lost `dropped` of them
                // from its front (unread output, or `BashOutput` reading it).
                let dropped = shell.dropped.load(std::sync::atomic::Ordering::Relaxed);
                let from = seen.saturating_sub(dropped).min(kept.len());
                let new = String::from_utf8_lossy(&kept[from..]).into_owned();
                seen = dropped + kept.len();
                (new, exit, shell.command.clone())
            };
            let source = match what {
                Watch::Lines => format!("monitor {id}"),
                Watch::End => format!("background {id}"),
            };
            if what == Watch::Lines {
                partial.push_str(&new);
                let complete = partial.rfind('\n').map(|at| partial.drain(..=at).collect::<String>());
                let text = match (complete, exit.is_some()) {
                    (Some(lines), false) => lines,
                    (Some(lines), true) => format!("{lines}{}", std::mem::take(&mut partial)),
                    (None, true) => std::mem::take(&mut partial),
                    (None, false) => String::new(),
                };
                if !text.trim().is_empty() {
                    inbox::push(client, Notice::new(&source, crate::tools::clip(text.trim_end(), BATCH)));
                }
            }
            if let Some(exit) = exit {
                inbox::push(client, Notice::new(source, format!("`{command}` finished ({exit}).")));
                return;
            }
        }
    });
}
