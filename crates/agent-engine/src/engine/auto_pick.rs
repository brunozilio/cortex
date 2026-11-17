use std::sync::Arc;

use domain::EngineEvent;
use domain::auto_pick::{Pick, Tier, settled};

use super::job::Job;
use crate::meter::Ledger;
use crate::model::{ModelApi, TurnRequest};
use crate::tools::Models;
use crate::transcript::Message;

const PICK_TOKENS: u32 = 100;

impl Job {
    pub(super) fn auto_pick(
        &mut self,
        api: &Arc<dyn ModelApi>,
        ledger: &Arc<Ledger>,
        template: &mut TurnRequest,
        models: &mut Models,
    ) {
        let small = models.small_model.clone();
        let pick = settled(&self.request.prompt.text)
            .or_else(|| self.judged(api, ledger, template, small.as_deref()))
            .unwrap_or(Pick::FALLBACK);
        if let (Tier::Small, Some(small)) = (pick.tier, small)
            && small != self.request.session.model.id
        {
            models.switch_to(small.clone());
            self.request.session.model.id.clone_from(&small);
            template.model = small;
        }
        self.request.session.effort = pick.effort;
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
        let reader = crate::tools::builtin::web::guarded::reader(small, template, self.request.session.model.provider);
        let mut model = self.request.session.model.clone();
        model.id.clone_from(&reader.model);
        let metered = self.metered(Arc::clone(api), ledger, model);
        let request = TurnRequest {
            system: crate::prompt::text("auto-pick"),
            messages: vec![Message::user(format!("<request>\n{}\n</request>", self.request.prompt.text))],
            effort: domain::Effort::Low,
            max_tokens: reader.max_tokens.min(PICK_TOKENS),
            ..reader
        };
        match crate::model::BlockingTurn::turn_blocking(&metered, &request, &mut |_| {}, self.control.token()) {
            Ok(reply) => Pick::parse(&reply.message.text()),
            Err(error) => {
                tracing::warn!(%error, "the automatic model pick could not run");
                None
            }
        }
    }
}
