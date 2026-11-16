use super::Flag;

pub const MEMORY_ANCHORS: &str = "experimental-memory-anchors";
pub const STRUCTURED_LEARNINGS: &str = "experimental-structured-learnings";
pub const LEARNING_CHECK: &str = "experimental-learning-check";
pub const BLAST_RADIUS: &str = "experimental-blast-radius";
pub const CODE_MAP: &str = "experimental-code-map";
pub const MEMORY_SEED: &str = "experimental-memory-seed";
pub const MEMORY_SCORECARD: &str = "experimental-memory-scorecard";

pub const KNOWN: &[Flag] = &[
    Flag {
        name: MEMORY_ANCHORS,
        about: "Memories name the files they are about, and the agent sees them when it reads or edits those files",
    },
    Flag {
        name: STRUCTURED_LEARNINGS,
        about: "Decisions are kept as what was decided, what was tried and ruled out, and what to watch out for",
    },
    Flag {
        name: LEARNING_CHECK,
        about: "/check has the agent compare the branch's changes with the memories about the files they touch",
    },
    Flag {
        name: BLAST_RADIUS,
        about: "After an edit, tell the agent which other files call the symbols it changed (language servers)",
    },
    Flag {
        name: CODE_MAP,
        about: "Index the project's symbols and callers with tree-sitter and point each prompt at the matching code",
    },
    Flag {
        name: MEMORY_SEED,
        about: "/memory seed offers what you asked for in two or more past sessions as memories to keep",
    },
    Flag {
        name: MEMORY_SCORECARD,
        about: "One request in 20 runs without recalled memory, so /memory score can show whether memory saves corrections and cost",
    },
];

#[cfg(test)]
mod tests {
    use super::super::{Features, known};
    use super::*;

    #[test]
    fn the_context_features_are_experimental_known_and_on_until_turned_off() {
        let listed = Features::default().listed();
        for flag in KNOWN {
            assert!(flag.name.starts_with("experimental-"), "{}", flag.name);
            assert_eq!(known(flag.name), Some(*flag));
            let shown = listed.iter().find(|listed| listed.name == flag.name).expect("listed");
            assert!(shown.on && shown.experimental && !shown.security);
        }
    }
}
