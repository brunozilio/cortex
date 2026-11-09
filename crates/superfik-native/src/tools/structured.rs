//! Validated structured results, independent of a provider's wire dialect.
use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;
use serde_json::{Value, json};
#[derive(Debug)]
pub struct Structured {
    schema: Value,
    validator: jsonschema::Validator,
}
impl Structured {
    pub fn new(schema: Value) -> Result<Self, String> {
        let validator = jsonschema::validator_for(&schema).map_err(|e| format!("invalid output JSON schema: {e}"))?;
        Ok(Self { schema, validator })
    }
}
impl Tool for Structured {
    fn spec(&self) -> ToolSpec {
        ToolSpec {name:"StructuredOutput".into(),description:"Deliver the final structured result. All fields must satisfy this schema. Call after doing the requested work and before completing the response. Validation errors must be corrected.".into(),schema:json!({"type":"object","properties":{"result":self.schema},"required":["result"],"additionalProperties":false})}
    }
    fn access(&self) -> Access {
        Access::Reads
    }
    fn summary(&self, _: &Value) -> String {
        "Structured result".into()
    }
    fn run(&self, input: &Value, _: &Workspace) -> Outcome {
        if input.as_object().is_none_or(|o| o.len() != 1 || !o.contains_key("result")) {
            return Outcome::error("Pass exactly one field named result.");
        }
        match self.validator.validate(&input["result"]) {
            Ok(()) => Outcome::ok("Structured result accepted."),
            Err(e) => Outcome::error(format!("Invalid structured result: {e}")),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn validates_nested_result_and_rejects_missing_and_extra_fields() {
        let tool=Structured::new(json!({"type":"object","properties":{"score":{"type":"number","minimum":0,"maximum":1}},"required":["score"],"additionalProperties":false})).expect("valid test fixture");
        let workspace = Workspace::new(".");
        assert!(!tool.run(&json!({"result":{"score":1}}), &workspace).is_error);
        for invalid in [json!({}), json!({"score":2}), json!({"score":"1"}), json!({"score":1,"extra":true})] {
            assert!(tool.run(&json!({"result":invalid}), &workspace).is_error);
        }
    }
}
