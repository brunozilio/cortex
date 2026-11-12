use agent_engine::tools::builtin::shell::shells;
use agent_engine::tools::watch;
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    if let Some(id) = input["kill"].as_str()
        && watch::stop(context.client, id).is_none()
    {
        shells::kill(context.client, id);
    }
    let watches = watch::list(context.client).into_iter().map(|(id, explained)| (id, explained, true));
    let listed: Vec<_> = shells::list(context.client).into_iter().chain(watches).collect();
    Ok(json!({ "shells": listed }))
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
