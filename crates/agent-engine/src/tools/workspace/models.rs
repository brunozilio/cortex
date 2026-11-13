#[derive(Debug, Clone)]
pub struct Models {
    pub model: domain::ModelOption,
    pub fallbacks: Vec<String>,
    pub small_model: Option<String>,
    pub advisor: Option<String>,
}

impl Models {
    pub fn new(model: domain::ModelOption) -> Self {
        Self { model, fallbacks: Vec::new(), small_model: None, advisor: None }
    }
}

impl Default for Models {
    fn default() -> Self {
        Self::new(domain::ModelOption::new("unknown", "Unknown"))
    }
}
