#[derive(Debug, Clone)]
pub struct Models {
    pub model: domain::ModelOption,
    pub fallbacks: Vec<String>,
    pub small_model: Option<String>,
    pub advisor: Option<String>,
    pub escalation: Option<String>,
    pub roles: domain::model_roles::ModelRoles,
}

impl Models {
    pub fn new(model: domain::ModelOption) -> Self {
        Self {
            model,
            fallbacks: Vec::new(),
            small_model: None,
            advisor: None,
            escalation: None,
            roles: Default::default(),
        }
    }

    pub fn switch_to(&mut self, id: String) {
        self.model.id = id;
        self.model.capabilities = None;
        self.model.window = None;
    }
}

impl Default for Models {
    fn default() -> Self {
        Self::new(domain::ModelOption::new("unknown", "Unknown"))
    }
}
