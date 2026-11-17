use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct Services {
    pub network: Option<Network>,
    pub lsp: Option<crate::lsp::Lsp>,
    pub files: Option<Arc<crate::checkpoints::FileHistory>>,
    pub deferred: Option<Arc<crate::tools::tool_search::Deferred>>,
    pub launch: super::Launch,
    pub judge: Option<Arc<crate::tools::host_judge::HostJudge>>,
}

#[derive(Clone)]
pub struct Network(pub Arc<dyn crate::model::http::Http>);

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Network")
    }
}
