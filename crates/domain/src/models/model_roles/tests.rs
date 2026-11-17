use super::*;
use crate::approval::PermissionMode;

fn roles(pairs: &[(ModelRole, &str)]) -> ModelRoles {
    let mut roles = ModelRoles::default();
    for (role, model) in pairs {
        roles.set(*role, Some((*model).to_owned()));
    }
    roles
}

#[test]
fn each_role_has_a_name_it_is_typed_and_saved_under() {
    let names: Vec<_> = ModelRole::ALL.iter().map(|role| role.name()).collect();
    assert_eq!(names, ["plan", "explore", "small", "review", "advisor"]);
    assert_eq!(ModelRole::parse(" Explore "), Some(ModelRole::Explore));
    assert_eq!(ModelRole::parse("implement"), None);
}

#[test]
fn planning_turns_use_the_plan_model_and_the_rest_use_the_session_model() {
    let set = roles(&[(ModelRole::Plan, "claude:opus")]);
    assert_eq!(set.for_turn("grok:grok-4", PermissionMode::Plan), "claude:opus");
    assert_eq!(set.for_turn("grok:grok-4", PermissionMode::AcceptEdits), "grok:grok-4");
    assert_eq!(ModelRoles::default().for_turn("grok:grok-4", PermissionMode::Plan), "grok:grok-4");
}

#[test]
fn the_plan_and_explore_agents_take_their_role_and_explore_falls_back_to_the_small_model() {
    let set = roles(&[(ModelRole::Plan, "p")]);
    assert_eq!(set.for_agent("Plan", Some("s")), Some("p"));
    assert_eq!(set.for_agent("Explore", Some("s")), Some("s"));
    assert_eq!(roles(&[(ModelRole::Explore, "e")]).for_agent("Explore", Some("s")), Some("e"));
    assert_eq!(set.for_agent("general-purpose", Some("s")), None);
}

#[test]
fn clearing_a_role_forgets_it_and_blank_names_are_never_kept() {
    let mut set = roles(&[(ModelRole::Review, "r")]);
    set.set(ModelRole::Review, None);
    set.set(ModelRole::Advisor, Some("  ".into()));
    assert_eq!(set, ModelRoles::default());
}

#[test]
fn roles_round_trip_as_an_object_keyed_by_role_name() {
    let set = roles(&[(ModelRole::Explore, "e"), (ModelRole::Advisor, "a")]);
    let json = serde_json::to_value(&set).expect("serialize");
    assert_eq!(json, serde_json::json!({"explore": "e", "advisor": "a"}));
    assert_eq!(serde_json::from_value::<ModelRoles>(json).expect("deserialize"), set);
}
