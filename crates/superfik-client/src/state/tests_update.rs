use super::*;
use crate::state::Action;

fn apply(session: &mut Session, action: UpdateAction) -> Vec<UpdateCall> {
    session
        .apply(Action::Update(action), 0.0)
        .into_iter()
        .filter_map(|effect| if let Effect::Update(call) = effect { Some(call) } else { None })
        .collect()
}

fn offer(required: bool) -> UpdateOffer {
    UpdateOffer { version: "0.15.0".into(), required }
}

#[test]
fn up_to_date_nothing_shows() {
    let mut session = Session::default();
    assert!(apply(&mut session, UpdateAction::Offered(None)).is_empty());
    assert!(!session.update.covers() && !session.update.offered());
}

#[test]
fn an_optional_update_downloads_in_the_background_and_waits_for_a_restart() {
    let mut session = Session::default();
    assert_eq!(apply(&mut session, UpdateAction::Offered(Some(offer(false)))), [UpdateCall::Prepare]);
    assert!(!session.update.covers(), "the agents keep working");
    assert!(session.update.offered(), "the sidebar offers it");
    apply(&mut session, UpdateAction::Stage(UpdateStageReport::Downloading { per_mille: 500 }));
    assert_eq!(session.update.phase, UpdatePhase::Preparing(UpdateStage::Downloading(0.5)));
    apply(&mut session, UpdateAction::Ready);
    apply(&mut session, UpdateAction::Open);
    assert!(session.update.covers());
    assert_eq!(apply(&mut session, UpdateAction::Primary), [UpdateCall::Restart]);
    apply(&mut session, UpdateAction::Close);
    assert!(!session.update.covers() && session.update.ready());
}

#[test]
fn the_same_offer_again_does_not_start_over() {
    let mut session = Session::default();
    apply(&mut session, UpdateAction::Offered(Some(offer(false))));
    assert!(apply(&mut session, UpdateAction::Offered(Some(offer(false)))).is_empty(), "already preparing");
    apply(&mut session, UpdateAction::Ready);
    assert!(apply(&mut session, UpdateAction::Offered(Some(offer(false)))).is_empty());
    assert!(session.update.ready(), "a staged update stays staged");
}

#[test]
fn a_required_update_covers_the_window_until_its_button_starts_it() {
    let mut session = Session::default();
    assert!(apply(&mut session, UpdateAction::Offered(Some(offer(true)))).is_empty());
    assert!(session.update.covers());
    apply(&mut session, UpdateAction::Close);
    assert!(session.update.covers(), "it cannot be put aside");
    assert_eq!(apply(&mut session, UpdateAction::Primary), [UpdateCall::Prepare]);
    assert!(apply(&mut session, UpdateAction::Primary).is_empty(), "already preparing");
    apply(&mut session, UpdateAction::Failed("network".into()));
    assert_eq!(session.update.phase, UpdatePhase::Failed("network".into()));
    assert_eq!(apply(&mut session, UpdateAction::Primary), [UpdateCall::Prepare], "try again");
}

#[test]
fn stages_reported_after_a_failure_are_dropped() {
    let mut session = Session::default();
    apply(&mut session, UpdateAction::Offered(Some(offer(true))));
    apply(&mut session, UpdateAction::Stage(UpdateStageReport::Installing));
    assert_eq!(session.update.phase, UpdatePhase::Available);
}

#[test]
fn while_the_update_covers_the_window_keys_reach_nothing_and_enter_presses_its_button() {
    let mut session = Session::default();
    apply(&mut session, UpdateAction::Offered(Some(offer(true))));
    session.apply(Action::Edit(crate::state::EditKey::Insert("hi".into())), 0.0);
    assert!(session.conversation().composer.text().is_empty());
    let effects = session.apply(Action::Submit, 0.0);
    assert!(effects.iter().any(|e| matches!(e, Effect::Update(UpdateCall::Prepare))));
}
