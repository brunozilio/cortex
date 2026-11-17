use domain::EngineEvent;

use super::open::Opened;
use crate::agent::Ending;
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::transcript::Usage;

impl Job {
    pub(in crate::engine) fn close(&self, opened: Opened, outcome: Result<Usage, Ending>) -> Stop {
        let Opened { workspace, mut worktree, api, session, history, checkpoint, .. } = opened;
        let ended = serde_json::json!({"session_id":session});
        let _ = workspace.hooks.run(crate::hooks::HookEvent::RunEnd, "", ended, &worktree.cleanup);
        match worktree.lease.finish() {
            Ok(Some(path)) => {
                self.gate.emit(EngineEvent::Notice(format!("Changes retained in worktree: {}", path.display())));
            }
            Err(error) => {
                self.gate.emit(EngineEvent::Notice(format!("Worktree cleanup failed; files retained: {error}")));
            }
            _ => {}
        }
        let history = crate::sessions::repaired(history);
        if let Err(error) = self.save(&session, &history) {
            return Stop::failed(format!("Could not save the conversation: {error}"));
        }
        self.keep_checkpoint(&session, checkpoint, &workspace);
        match outcome {
            Ok(_) | Err(Ending::Moved) => Stop::completed(api.ledger.cost()),
            Err(Ending::Cancelled) => Stop::Cancelled,
            Err(Ending::Failed(error)) => Stop::Model(error),
            Err(Ending::Refused) => Stop::failed("The model declined to continue with this request."),
            Err(Ending::TooManyTurns) => Stop::failed("The agent stopped after too many steps without finishing."),
        }
    }
}
