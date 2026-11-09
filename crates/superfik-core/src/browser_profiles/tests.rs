use super::*;

#[test]
fn saved_profiles_keep_their_seed_and_conversation_assignment() {
    let mut book = ProfileBook::default();
    let profile = BrowserProfile::new("Laboratório".into());
    let id = profile.id;
    let seed = profile.settings.seed;
    book.save(profile).expect("save");
    book.assign("conversation-a".into(), Some(id)).expect("assign");
    let mut restored = ProfileBook::parse(&serde_json::to_string(&book).expect("JSON")).expect("restore");
    assert_eq!(restored.for_conversation("conversation-a").map(|p| p.settings.seed), Some(seed));
    assert!(restored.for_conversation("conversation-b").is_none());
    assert!(restored.delete(id).is_err(), "assigned profiles require an explicit unassignment");
    book.assign("conversation-a".into(), None).expect("use real values");
    assert!(
        book.uses_actual_values("conversation-a"),
        "explicit real values override an older laboratory identity file"
    );
    book.delete(id).expect("unassigned profile can be removed");
}
