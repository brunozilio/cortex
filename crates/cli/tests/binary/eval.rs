use crate::Sandbox;

#[test]
fn a_project_without_evals_says_where_they_go() {
    let sandbox = Sandbox::fresh();
    let ran = sandbox.run(&["eval"]);
    let folder = sandbox.project_dir().join("evals");
    ran.failed_with(1).quiet_on_stdout().complained(&format!("No evals in {}", folder.display()));
}

#[test]
fn a_case_without_checks_scores_zero_and_the_json_says_why() {
    let sandbox = Sandbox::fresh();
    sandbox.write(&sandbox.project_dir().join("evals/greets/case.json"), r#"{"prompt":"say hi"}"#);
    let ran = sandbox.run(&["eval", "--json"]);
    ran.failed_with(1).complained("Running greets");
    let scores = ran.json();
    assert_eq!(scores[0]["name"], "greets");
    assert_eq!(scores[0]["passed"], 0);
    assert_eq!(scores[0]["notes"][0], "case.json needs checks or a rubric");
}
