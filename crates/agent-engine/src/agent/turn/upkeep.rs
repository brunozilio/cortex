use domain::EngineEvent;

use super::TurnState;
use crate::agent::{Agent, Ending};
use crate::model::ModelError;
use crate::upkeep::{clear_old_results, compact_hooked, measure};

pub(super) struct Budget {
    pub(super) used: u64,
    pub(super) threshold: u64,
}

impl Agent<'_> {
    pub(super) fn upkeep(&self, turn: &mut TurnState<'_>) -> Result<Budget, Ending> {
        let model = &self.workspace.models.model;
        let measured = measure(model, &self.request.system, &self.request.tools, turn.history);
        let threshold = model
            .capabilities
            .as_ref()
            .and_then(|c| c.auto_compact_token_limit)
            .unwrap_or_else(|| measured.window.saturating_sub(measured.reserved));
        let mut used = turn.counted.used(measured.used, turn.history);
        if used >= threshold && clear_old_results(turn.history) > 0 {
            turn.counted.forget();
            self.workspace.forget_seen_files();
            used = measure(model, &self.request.system, &self.request.tools, turn.history).used;
        }
        if turn.history.len() > 2 && used >= threshold && turn.compacted_at != Some(turn.history.len()) {
            self.compact_ahead(turn)?;
        }
        Ok(Budget { used, threshold })
    }

    fn compact_ahead(&self, turn: &mut TurnState<'_>) -> Result<(), Ending> {
        let template = self.turn_template(turn.fell_back.as_ref());
        let compacted = compact_hooked(self.api, &template, turn.history, self.workspace, "auto");
        turn.prompt.compacted(turn.history);
        match compacted {
            Ok(compaction) => self.compacted(turn, compaction),
            Err(ModelError::Cancelled) => return Err(Ending::Cancelled),
            Err(error) => tracing::warn!(%error, "automatic compaction failed; the turn goes on without it"),
        }
        Ok(())
    }

    pub(super) fn compact_to_fit(&self, turn: &mut TurnState<'_>) -> Result<(), Ending> {
        let template = self.turn_template(turn.fell_back.as_ref());
        let compaction =
            compact_hooked(self.api, &template, turn.history, self.workspace, "auto").map_err(Ending::Failed)?;
        turn.prompt.compacted(turn.history);
        self.compacted(turn, compaction);
        Ok(())
    }

    fn compacted(&self, turn: &mut TurnState<'_>, compaction: domain::Compaction) {
        turn.counted.forget();
        turn.reminders.resend();
        (self.emit)(EngineEvent::Compacted(compaction));
        turn.compacted_at = Some(turn.history.len());
    }
}
