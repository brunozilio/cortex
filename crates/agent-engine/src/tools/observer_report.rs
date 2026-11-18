use std::sync::{Arc, Mutex};

use domain::inbox::Notice;
use platform::sync::Locked;
use serde_json::Value;

use super::{Effect, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub use domain::observer::REPORT_TOOL;

pub type Reports = Arc<Mutex<Vec<Notice>>>;

#[derive(Debug)]
pub struct ObserverReport {
    pub observer: String,
    pub reports: Reports,
}

impl Tool for ObserverReport {
    fn spec(&self) -> ToolSpec {
        super::declared(REPORT_TOOL, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        Vec::new()
    }

    fn summary(&self, input: &Value) -> String {
        input["message"].as_str().unwrap_or_default().lines().next().unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
        let Some(message) = input["message"].as_str().map(str::trim).filter(|message| !message.is_empty()) else {
            return Outcome::error("Missing required parameter `message`.");
        };
        self.reports.locked().push(domain::observer::report(&self.observer, message));
        Outcome::ok("Sent to the agent you watch.")
    }
}
