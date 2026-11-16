use serde_json::json;

use super::*;

fn found(name: &str, description: &str) -> SkillEntry {
    SkillEntry { name: name.into(), description: description.into(), model_invocation: true, body: None }
}

#[test]
fn a_hook_keeps_changes_adds_and_drops_skills_by_what_it_returns() {
    let discovered = [found("review", "Review a diff"), found("verify", "Run checks"), found("init", "Start")];
    let returned = json!([
        { "name": "verify" },
        { "name": "review", "description": "Review with our rules", "body": "Use the ACME checklist." },
        { "name": "deploy", "description": "Ship it", "body": "Run make deploy.", "disableModelInvocation": true },
    ]);

    let edited = edit(&discovered, &returned).expect("valid");

    assert_eq!(
        edited,
        [
            Edited { kept: Some(1), skill: found("verify", "Run checks") },
            Edited {
                kept: Some(0),
                skill: SkillEntry {
                    name: "review".into(),
                    description: "Review with our rules".into(),
                    model_invocation: true,
                    body: Some("Use the ACME checklist.".into()),
                },
            },
            Edited {
                kept: None,
                skill: SkillEntry {
                    name: "deploy".into(),
                    description: "Ship it".into(),
                    model_invocation: false,
                    body: Some("Run make deploy.".into()),
                },
            },
        ],
        "init is left out, so it is removed"
    );
}

#[test]
fn a_new_skill_needs_a_body_and_a_description() {
    let error = edit(&[], &json!([{ "name": "deploy", "description": "Ship it" }])).expect_err("no body");
    assert_eq!(error, "the hook added the skill \"deploy\" without a body");
    let error = edit(&[], &json!([{ "name": "deploy", "body": "Run it." }])).expect_err("no description");
    assert_eq!(error, "the hook added the skill \"deploy\" without a description");
}

#[test]
fn every_skill_has_one_plain_name_and_the_list_is_a_list() {
    let discovered = [found("review", "Review a diff")];
    assert_eq!(
        edit(&discovered, &json!([{ "name": "review" }, { "name": "review" }])).expect_err("twice"),
        "the hook listed the skill \"review\" twice"
    );
    assert_eq!(
        edit(&discovered, &json!([{ "name": " " }])).expect_err("blank"),
        "every skill the hook returns needs a name"
    );
    assert_eq!(
        edit(&discovered, &json!([{ "name": "a b", "body": "x", "description": "y" }])).expect_err("space"),
        "the skill name \"a b\" has spaces or a leading slash"
    );
    assert_eq!(edit(&discovered, &json!({ "name": "review" })).expect_err("object"), "skills must be a list");
}
