mod check;
mod mcp;
mod settings;

use std::sync::OnceLock;

use serde_json::{Value, json};

pub use self::check::{Checked, logged_once};

pub const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
pub const SETTINGS_FILE: &str = "schemas/settings.schema.json";
pub const MCP_FILE: &str = "schemas/mcp.schema.json";

pub fn settings() -> Value {
    settings::schema()
}

pub fn mcp() -> Value {
    mcp::schema()
}

pub fn published() -> [(&'static str, Value); 2] {
    [(SETTINGS_FILE, settings()), (MCP_FILE, mcp())]
}

pub fn checked_settings(value: Value) -> Checked {
    static SETTINGS: OnceLock<Option<jsonschema::Validator>> = OnceLock::new();
    check::checked(validator(&SETTINGS, settings), check::Whole::ListItem, value)
}

pub fn checked_mcp(value: Value) -> Checked {
    static MCP: OnceLock<Option<jsonschema::Validator>> = OnceLock::new();
    check::checked(validator(&MCP, mcp), check::Whole::Entry { under: "mcpServers" }, value)
}

fn validator(
    made: &'static OnceLock<Option<jsonschema::Validator>>,
    schema: fn() -> Value,
) -> Option<&'static jsonschema::Validator> {
    made.get_or_init(|| {
        jsonschema::validator_for(&schema())
            .inspect_err(|error| tracing::error!(%error, "a published schema does not compile; files go unchecked"))
            .ok()
    })
    .as_ref()
}

fn reference() -> Value {
    json!({ "type": "string", "description": "The JSON Schema this file follows, for editors; the app ignores it." })
}

fn reference_to(name: &str) -> Value {
    json!({ "$ref": format!("#/$defs/{name}") })
}

fn list_of(items: Value) -> Value {
    json!({ "type": "array", "items": items })
}

fn map_of(values: Value, about: &str) -> Value {
    json!({ "type": "object", "additionalProperties": values, "description": about })
}

#[cfg(test)]
mod tests;
