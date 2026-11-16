use std::cell::RefCell;

use super::*;

struct Scripted {
    writes: Vec<(&'static str, &'static str)>,
    answers: RefCell<Vec<&'static str>>,
    asked: RefCell<Vec<(String, Option<String>)>>,
}

impl Asker for Scripted {
    fn ask(&self, dir: &Path, prompt: &str, mode: Option<&str>) -> Result<Answer, String> {
        if self.asked.borrow().is_empty() {
            for (path, text) in &self.writes {
                std::fs::write(dir.join(path), text).map_err(|e| e.to_string())?;
            }
        }
        self.asked.borrow_mut().push((prompt.to_owned(), mode.map(str::to_owned)));
        let text = self.answers.borrow_mut().remove(0);
        Ok(Answer { text: text.into(), cost_usd: 0.1 })
    }
}

fn case(project: &Path, name: &str, case: &str, fixture: &[(&str, &str)]) -> PathBuf {
    let dir = folder(project).join(name);
    std::fs::create_dir_all(dir.join(FIXTURE)).expect("fixture");
    std::fs::write(dir.join(CASE_FILE), case).expect("case");
    for (path, text) in fixture {
        std::fs::write(dir.join(FIXTURE).join(path), text).expect("file");
    }
    dir
}

#[test]
fn a_case_runs_in_a_copy_of_its_fixture_and_is_scored_by_its_checks_and_rubric() {
    let project = tempfile::tempdir().expect("project");
    let dir = case(
        project.path(),
        "rename",
        r#"{"prompt": "Rename greet", "mode": "bypass", "rubric": "Only lib.rs changed.", "checks": [
            {"file_contains": {"path": "lib.rs", "text": "fn welcome"}},
            {"answer_contains": "welcome"},
            {"command": "test -f lib.rs"}
        ]}"#,
        &[("lib.rs", "fn greet() {}")],
    );
    let agent = Scripted {
        writes: vec![("lib.rs", "fn welcome() {}")],
        answers: RefCell::new(vec!["Renamed to welcome.", r#"{"passed": true, "why": "ok"}"#]),
        asked: RefCell::default(),
    };

    let outcome = run_case(&agent, "rename", &dir);

    assert_eq!((outcome.passed, outcome.total), (4, 4), "{:?}", outcome.notes);
    assert!((outcome.cost_usd - 0.2).abs() < 1e-9);
    let asked = agent.asked.borrow();
    assert_eq!(asked[0], ("Rename greet".to_owned(), Some("bypass".to_owned())));
    assert!(asked[1].0.contains("M lib.rs") && asked[1].1.as_deref() == Some("plan"), "{:?}", asked[1]);
    let fixture = std::fs::read_to_string(dir.join(FIXTURE).join("lib.rs")).expect("fixture");
    assert_eq!(fixture, "fn greet() {}", "the fixture itself is never touched");
}

#[test]
fn failed_checks_are_named_and_cases_are_found_by_name() {
    let project = tempfile::tempdir().expect("project");
    let dir = case(
        project.path(),
        "fix",
        r#"{"prompt": "Fix it", "checks": [{"file_contains": {"path": "a.txt", "text": "fixed"}}, {"command": "false"}]}"#,
        &[("a.txt", "broken")],
    );
    case(project.path(), "other", r#"{"prompt": "x", "checks": [{"answer_contains": "x"}]}"#, &[]);
    let agent = Scripted { writes: vec![], answers: RefCell::new(vec!["Done."]), asked: RefCell::default() };

    let outcome = run_case(&agent, "fix", &dir);
    assert_eq!((outcome.passed, outcome.total), (0, 2));
    assert!(outcome.notes.iter().any(|note| note.contains("a.txt contains \"fixed\"")), "{:?}", outcome.notes);

    let all = cases(project.path(), &[]).expect("cases");
    assert_eq!(all.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["fix", "other"]);
    assert_eq!(cases(project.path(), &["other".into()]).expect("one").len(), 1);
    assert!(cases(project.path(), &["missing".into()]).is_err());
}
