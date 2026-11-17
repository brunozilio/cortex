use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::ApiSource;
use super::stop::Stop;
use crate::RunGate;
use crate::control::Control;
use crate::model::http::Http;
use crate::sessions::Sessions;
use crate::tools::Workspace;
use domain::PromptRequest;

pub(super) struct Job {
    pub(super) source: Arc<dyn ApiSource>,
    pub(super) sessions: Arc<dyn Sessions>,
    pub(super) mcp: Arc<dyn Http>,
    pub(super) lsp: Arc<dyn crate::lsp::LanguageServers>,
    pub(super) request: PromptRequest,
    pub(super) control: Arc<Control>,
    pub(super) gate: RunGate,
    pub(super) recorder: Option<Arc<super::record::Recorder>>,
    pub(super) held: Arc<Mutex<Option<crate::sessions::Hold>>>,
    pub(super) live: super::live::Live,
}

impl Job {
    pub(super) fn run(mut self, workspace: Workspace) {
        let started = Instant::now();
        let stop = self.phases(workspace).unwrap_or_else(|stop| stop);
        self.live.leave();
        self.end(stop, started);
    }

    pub(super) fn phases(&mut self, workspace: Workspace) -> Result<Stop, Stop> {
        let session = self.load_session()?;
        let brief = self.brief()?;
        let equipped = self.equip(&brief, workspace)?;
        let governed = self.apply_policy(equipped, &brief, &session.id)?;
        let runtime = self.prepare_runtime(governed, &brief.settings, &session)?;
        let mut opened = self.open(runtime, &brief, session)?;
        let outcome = self.drive(&mut opened, &brief.agents);
        Ok(self.close(opened, outcome))
    }
}
