use std::sync::Arc;

use domain::EngineEvent;
use domain::auto_pick::{Pick, Tier, settled};

use super::job::Job;
use crate::meter::{Ledger, Metered};
use crate::model::{ModelApi, TurnRequest};
use crate::transcript::Message;

const PICK_TOKENS: u32 = 100;

impl Job {
    pub(super) fn auto_pick(&mut self, api: &Arc<dyn ModelApi>, ledger: &Arc<Ledger>, template: &mut TurnRequest) {
        let small = self.workspace.models.small_model.clone();
        let pick = settled(&self.request.prompt)
            .or_else(|| self.judged(api, ledger, template, small.as_deref()))
            .unwrap_or(Pick::FALLBACK);
        let model = match (pick.tier, small) {
            (Tier::Small, Some(small)) => small,
            _ => self.request.model.id.clone(),
        };
        if model != self.request.model.id
            && let Some(workspace) = Arc::get_mut(&mut self.workspace)
        {
            workspace.models.model.id = model.clone();
            workspace.models.model.capabilities = None;
            workspace.models.model.window = None;
            self.request.model.id = model.clone();
            template.model = model.clone();
        }
        self.request.effort = pick.effort;
        template.effort = pick.effort;
        self.gate.emit(EngineEvent::Picked { model: template.model.clone(), effort: pick.effort });
    }

    fn judged(
        &self,
        api: &Arc<dyn ModelApi>,
        ledger: &Arc<Ledger>,
        template: &TurnRequest,
        small: Option<&str>,
    ) -> Option<Pick> {
        let reader = crate::tools::builtin::web::guarded::reader(small, template, self.request.model.provider);
        let mut model = self.request.model.clone();
        model.id = reader.model.clone();
        let metered = Metered {
            api: Arc::clone(api),
            ledger: Arc::clone(ledger),
            provider: self.request.model.provider,
            gate: self.gate.clone(),
            model,
            calls: self.recorder.as_ref().map(|recorder| recorder.calls()),
            budget: self.request.budget,
        };
        let request = TurnRequest {
            system: crate::prompt::text("auto-pick"),
            messages: vec![Message::user(format!("<request>\n{}\n</request>", self.request.prompt))],
            effort: domain::Effort::Low,
            max_tokens: reader.max_tokens.min(PICK_TOKENS),
            ..reader
        };
        match metered.turn(&request, &mut |_| {}, self.control.stop_flag()) {
            Ok(reply) => Pick::parse(&reply.message.text()),
            Err(error) => {
                tracing::warn!(%error, "the automatic model pick could not run");
                None
            }
        }
    }
}
