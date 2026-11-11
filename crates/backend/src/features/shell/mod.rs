use agent_engine::tools::builtin::shell::shells;
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    if let Some(id) = input["kill"].as_str() {
        shells::kill(context.client, id);
    }
    Ok(json!({ "shells": shells::list(context.client) }))
}

pub fn forget(client: u64) {
    shells::kill_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_sees_only_its_own_commands() {
        let listed = crate::feature::testing::local(78_001, |context| handle(context, json!({ "kill": "bash_none" })))
            .expect("listed");
        assert_eq!(listed, json!({ "shells": [] }));
        forget(78_001);
    }
}
