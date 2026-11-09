//! Clicks on the Notes node, and what a click elsewhere does to the note
//! being written.

use superfik_core::note::Paper;

use crate::state::{Action, NoteAction, Session};
use crate::view::target::{NoteTarget, Target};

pub fn click(target: NoteTarget) -> Vec<Action> {
    let action = match target {
        NoteTarget::Card(id) => NoteAction::Open(Some(id)),
        NoteTarget::Check(id, item) => NoteAction::Toggle { id, item },
        NoteTarget::Unlink(id) => NoteAction::Unlink(id),
        NoteTarget::Link(id) => NoteAction::Link(id),
        NoteTarget::New => NoteAction::Open(None),
        NoteTarget::Paper(i) => match Paper::ALL.get(i) {
            Some(paper) => NoteAction::Recolor(*paper),
            None => return Vec::new(),
        },
        NoteTarget::Draft => return Vec::new(),
    };
    vec![Action::Note(action)]
}

pub fn leave_draft(session: &Session, target: Option<Target>) -> Option<Action> {
    let keeps = matches!(
        target,
        Some(Target::Note(NoteTarget::Draft | NoteTarget::Paper(_) | NoteTarget::Card(_) | NoteTarget::New))
    );
    (session.notes.editing.is_some() && !keeps).then_some(Action::Note(NoteAction::Close))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::note::NoteId;

    use super::*;

    #[test]
    fn a_click_elsewhere_saves_the_draft_but_a_click_in_it_does_not() {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        assert!(leave_draft(&session, Some(Target::Surface)).is_none(), "nothing is being written");
        session.apply(Action::Note(NoteAction::Open(None)), 0.0);
        assert!(leave_draft(&session, Some(Target::Note(NoteTarget::Draft))).is_none());
        assert!(leave_draft(&session, Some(Target::Note(NoteTarget::Paper(2)))).is_none(), "a swatch keeps writing");
        assert!(matches!(leave_draft(&session, None), Some(Action::Note(NoteAction::Close))));
        assert!(matches!(
            leave_draft(&session, Some(Target::Note(NoteTarget::Check(NoteId(0), 0)))),
            Some(Action::Note(NoteAction::Close))
        ));
    }

    #[test]
    fn a_box_ticks_and_a_card_opens() {
        let id = NoteId(4);
        assert!(matches!(
            click(NoteTarget::Check(id, 1)).as_slice(),
            [Action::Note(NoteAction::Toggle { id: NoteId(4), item: 1 })]
        ));
        assert!(matches!(click(NoteTarget::Card(id)).as_slice(), [Action::Note(NoteAction::Open(Some(NoteId(4))))]));
        assert!(matches!(click(NoteTarget::Paper(2)).as_slice(), [Action::Note(NoteAction::Recolor(Paper::Blue))]));
        assert!(click(NoteTarget::Paper(9)).is_empty());
    }
}
