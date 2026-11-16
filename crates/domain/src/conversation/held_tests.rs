use std::path::{Path, PathBuf};

use super::*;
use crate::EngineEvent;

#[test]
fn a_refused_or_repeated_tool_call_is_part_of_the_conversation() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::Refused { tool: "Bash".into(), reason: "rm -rf is denied".into() });
    transcript.apply(&EngineEvent::Repeated { tool: "Read".into() });
    assert_eq!(
        transcript.entries,
        [
            Entry::Refused { tool: "Bash".into(), reason: "rm -rf is denied".into() },
            Entry::Repeated { tool: "Read".into() },
        ]
    );
}

#[test]
fn a_session_that_moved_knows_where_it_works_now() {
    let mut transcript = Transcript::default();
    assert_eq!(transcript.moved_to(), None);
    transcript.begin();
    transcript.apply(&EngineEvent::Moved(PathBuf::from("/work/tree-a")));
    assert_eq!(transcript.moved_to(), Some(Path::new("/work/tree-a")));
}
