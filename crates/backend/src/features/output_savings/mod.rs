use agent_engine::tools::builtin::shell::savings;
use serde_json::Value;

use crate::feature::Context;

pub fn handle(context: &Context<'_>, _input: Value) -> Result<Value, String> {
    serde_json::to_value(savings::list(context.client)).map_err(|error| error.to_string())
}

pub fn forget(client: u64) {
    savings::forget(client);
}

#[cfg(test)]
mod tests {
    use domain::output::Saving;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_client_reads_only_its_own_savings() {
        let saving = Saving { program: "cargo".into(), before: 900, after: 120 };
        savings::record(78_101, saving.clone());
        savings::record(78_102, Saving { program: "git".into(), before: 5, after: 5 });

        let listed = crate::feature::testing::local(78_101, |context| handle(context, json!({}))).expect("listed");

        assert_eq!(listed, json!([saving]));
        forget(78_101);
        forget(78_102);
        assert_eq!(savings::list(78_101), []);
    }
}
