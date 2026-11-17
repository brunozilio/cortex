use crate::api::features::output_savings::OutputSavingsReply;
use agent_engine::tools::builtin::shell::savings;

use crate::features::context::Context;

pub fn handle(context: &Context<'_>) -> Result<OutputSavingsReply, String> {
    Ok(OutputSavingsReply { savings: savings::list(context.client) })
}

pub fn forget(client: u64) {
    savings::forget(client);
}

#[cfg(test)]
mod tests {
    use domain::output::Saving;

    use super::*;

    #[test]
    fn a_client_reads_only_its_own_savings() {
        let saving = Saving { program: "cargo".into(), before: 900, after: 120 };
        savings::record(78_101, saving.clone());
        savings::record(78_102, Saving { program: "git".into(), before: 5, after: 5 });

        let listed = crate::features::context::testing::local(78_101, handle).expect("listed");

        assert_eq!(listed.savings, [saving]);
        forget(78_101);
        forget(78_102);
        assert_eq!(savings::list(78_101), []);
        assert_eq!(
            crate::features::context::testing::local(78_101, handle),
            Ok(OutputSavingsReply { savings: Vec::new() })
        );
    }
}
