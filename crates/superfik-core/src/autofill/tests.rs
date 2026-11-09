use super::*;

fn page(json: &str) -> Vec<PageField> {
    serde_json::from_str(json).expect("fixture parses")
}

fn fills(username: Option<u32>, password: Option<u32>) -> Option<FillPlan> {
    Some(FillPlan { username, password })
}

fn focused(mut fields: Vec<PageField>, reference: u32) -> Vec<PageField> {
    for field in &mut fields {
        field.focused = field.reference == reference;
    }
    fields
}

#[test]
fn a_sign_in_form_fills_its_username_and_password_not_the_search() {
    assert_eq!(plan(&page(include_str!("fixtures/github.json"))), fills(Some(2), Some(3)));
}

#[test]
fn a_first_step_with_only_the_username_fills_the_username() {
    assert_eq!(plan(&page(include_str!("fixtures/google_identifier.json"))), fills(Some(1), None));
}

#[test]
fn a_password_step_with_the_username_hidden_fills_the_password() {
    assert_eq!(plan(&page(include_str!("fixtures/google_password.json"))), fills(None, Some(2)));
}

#[test]
fn a_page_without_forms_takes_the_nearest_text_field() {
    assert_eq!(plan(&page(include_str!("fixtures/react_without_form.json"))), fills(Some(2), Some(3)));
}

#[test]
fn the_sign_in_form_comes_before_the_registration_form() {
    assert_eq!(plan(&page(include_str!("fixtures/sign_in_and_register.json"))), fills(Some(4), Some(5)));
}

#[test]
fn a_focused_field_chooses_the_form() {
    let fields = focused(page(include_str!("fixtures/sign_in_and_register.json")), 1);
    assert_eq!(plan(&fields), fills(Some(1), Some(2)));
    let fields = focused(page(include_str!("fixtures/sign_in_and_register.json")), 3);
    assert_eq!(plan(&fields), fills(Some(1), Some(3)));
}

#[test]
fn a_focused_username_without_a_password_beside_it_fills_alone() {
    let mut fields = page(include_str!("fixtures/github.json"));
    fields.retain(|f| f.kind != "password");
    assert_eq!(plan(&focused(fields, 2)), fills(Some(2), None));
}

#[test]
fn a_registration_page_alone_still_takes_the_login() {
    assert_eq!(plan(&page(include_str!("fixtures/register_only.json"))), fills(Some(2), Some(3)));
}

#[test]
fn verification_codes_are_never_the_username() {
    assert_eq!(plan(&page(include_str!("fixtures/bank_cpf.json"))), fills(Some(1), Some(3)));
}

#[test]
fn a_page_without_login_fields_fills_nothing() {
    assert_eq!(plan(&page(include_str!("fixtures/newsletter.json"))), None);
    assert_eq!(plan(&[]), None);
}

#[test]
fn a_fill_never_prints_its_values() {
    let fill = LoginFill { username: Some("bruno".into()), password: Some("hunter2".into()) };
    let shown = format!("{fill:?}");
    assert!(!shown.contains("bruno") && !shown.contains("hunter2"), "{shown}");
}

#[test]
fn disabled_and_read_only_fields_are_skipped() {
    let mut fields = page(include_str!("fixtures/github.json"));
    fields[1].readonly = true;
    fields[2].disabled = true;
    assert_eq!(plan(&fields), None);
}

#[test]
fn an_agents_sign_in_names_the_login_and_the_host() {
    assert_eq!(sign_in_subject("GitHub", "https://github.com/login?x=1"), "GitHub → sign in on github.com");
    assert_eq!(sign_in_subject("A", "not a url"), "A → sign in on not a url");
    assert!(agent_answer(FillOutcome::Filled).is_ok());
    assert!(agent_answer(FillOutcome::Absent).is_err());
    assert!(agent_answer(FillOutcome::Denied).is_err());
}
