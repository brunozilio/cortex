use agent_engine::tools::builtin::shell::shells;
use domain::Principal;
use serde_json::{Value, json};

pub fn handle(client: u64, _principal: &Principal, input: Value) -> Result<Value, String> {
    if let Some(id) = input["kill"].as_str() {
        shells::kill(client, id);
    }
    Ok(json!({ "shells": shells::list(client) }))
}

pub fn forget(client: u64) {
    shells::kill_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_sees_only_its_own_commands() {
        let listed = handle(78_001, &Principal::LocalOwner, json!({ "kill": "bash_none" })).expect("listed");
        assert_eq!(listed, json!({ "shells": [] }));
        forget(78_001);
    }
}
