use std::sync::{Arc, Mutex};

use domain::PromptRequest;
use platform::sync::Locked;

use crate::brief::Brief;
use crate::tools::script_tool::{self, ScriptTool};

#[derive(Debug, Clone)]
pub(in crate::engine) struct Loaded {
    pub(in crate::engine) brief: Brief,
    pub(in crate::engine) scripts: Vec<ScriptTool>,
}

pub(in crate::engine) trait Load {
    fn fingerprint(&self, request: &PromptRequest) -> Option<u64>;
    fn load(&self, request: &PromptRequest) -> Result<Loaded, String>;
}

pub(in crate::engine) struct FromDisk;

impl Load for FromDisk {
    fn fingerprint(&self, request: &PromptRequest) -> Option<u64> {
        super::sources::fingerprint(request.folder())
    }

    fn load(&self, request: &PromptRequest) -> Result<Loaded, String> {
        let brief = Brief::load(request.folder())?;
        let scripts = script_tool::load(&brief.gate(&request.session.cwd));
        Ok(Loaded { brief, scripts })
    }
}

#[derive(Default)]
pub(in crate::engine) struct SessionCache {
    kept: Mutex<Option<(u64, Arc<Loaded>)>>,
}

impl SessionCache {
    pub(in crate::engine) fn loaded(&self, request: &PromptRequest, load: &dyn Load) -> Result<Arc<Loaded>, String> {
        let fingerprint = load.fingerprint(request);
        if let (Some(now), Some((then, loaded))) = (fingerprint, &*self.kept.locked())
            && now == *then
        {
            return Ok(Arc::clone(loaded));
        }
        let loaded = Arc::new(load.load(request)?);
        *self.kept.locked() = fingerprint.map(|now| (now, Arc::clone(&loaded)));
        Ok(loaded)
    }
}
