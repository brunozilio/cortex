//! How an agent working unattended (a routine's firing, a background job)
//! says where it stands, so its report can be read without its tools'
//! output: a closing `result:`, `needs input:` or `failed:` line. Pure.

use serde::{Deserialize, Serialize};

pub const CONVENTIONS: &str = "\
This session is a background job. The user may be watching or away; answer naturally either way. Only your \
message text reaches the job's report (not tool output, sub-agent reports or the user's replies), so these \
conventions always apply.

**Narrate.** One line on your approach before acting. After each chunk of work: what happened, what is next.

**Restate.** Say results in your own words even when a tool already printed them: the report cannot see tool \
output. When the user replies, let your first sentence carry what they asked or said as part of your answer, \
since the report cannot see their message, rather than as a separate recap.

For noisy investigation (sweeps of searches, trawls through logs, broad searches), start a sub-agent when you \
have the Agent tool and keep only its findings here.

**Completed.** First run a sanity check (a test, a build, rereading the request) and say what you checked. Then \
write `result:` at the start of its own line, followed by a self-contained one-line headline that someone who \
never saw the request can read. That line is the only sign of completion: prose like \"done\" or \"finished\" \
is not detected. `result:` means the request is delivered; pushing or launching something that still has to \
settle is narration, not a result. Leave it out only for greetings and clarifying questions; an answer to a \
question is a deliverable.

**Needs input.** Only when one action of a person unblocks you (signing in, a decision, access you cannot grant \
yourself) and guessing would cost more than the round trip. If a reasonable guess exists, take it, note the \
assumption and keep working. When truly stuck, write `needs input:` at the start of its own line, stating \
exactly what you need.

**Failed.** The task is impossible as framed (the wrong repository, a missing program, a false premise). Write \
`failed:` at the start of its own line with the reason.

Everything else: keep working.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobSignal {
    Result(String),
    NeedsInput(String),
    Failed(String),
}

impl JobSignal {
    pub fn find(text: &str) -> Option<Self> {
        text.lines().rev().find_map(|line| {
            let line = line.trim().trim_start_matches(['*', '_', '`']).trim_start();
            let after = |marker: &str| {
                let head = line.get(..marker.len())?;
                head.eq_ignore_ascii_case(marker)
                    .then(|| line[marker.len()..].trim_matches(['*', '_', '`', ' ']).to_owned())
                    .filter(|rest| !rest.is_empty())
            };
            after("result:")
                .map(Self::Result)
                .or_else(|| after("needs input:").map(Self::NeedsInput))
                .or_else(|| after("failed:").map(Self::Failed))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_signal_line_counts_and_prose_does_not() {
        let text = "Checked: tests pass.\nresult: Login page moved to the new router\n";
        assert_eq!(JobSignal::find(text), Some(JobSignal::Result("Login page moved to the new router".into())));
        assert_eq!(
            JobSignal::find("Tried twice.\n**Needs input:** the Sentry token"),
            Some(JobSignal::NeedsInput("the Sentry token".into()))
        );
        assert_eq!(JobSignal::find("failed: no such repository"), Some(JobSignal::Failed("no such repository".into())));
        assert_eq!(JobSignal::find("All done and finished."), None);
        assert_eq!(JobSignal::find("result:"), None, "a marker needs its words");
        assert!(!CONVENTIONS.contains("Claude"));
    }
}
