#![no_main]

use std::path::Path;

use agent_engine::config_schema::{Checked, checked_mcp, checked_settings};
use agent_engine::mcp::parse_config;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fn settled(check: fn(Value) -> Checked, value: Value, file: &str) {
    let first = check(value.clone());
    assert_eq!(first.located(Path::new(file)).len(), first.problems.len());
    if first.problems.is_empty() {
        assert_eq!(first.value, value, "a {file} without problems was changed");
    }
    let again = check(first.value.clone());
    assert_eq!(again.value, first.value, "checking a checked {file} changed it again");
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let _ = parse_config(text);
    let Ok(value) = serde_json::from_str::<Value>(text) else { return };
    settled(checked_settings, value.clone(), "settings.json");
    settled(checked_mcp, value, ".mcp.json");
});
