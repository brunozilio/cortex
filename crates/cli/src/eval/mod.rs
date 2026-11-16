mod files;
mod process;

use std::path::{Path, PathBuf};

use domain::evals::{Case, Check, Found, Outcome, judge_prompt, judged};

pub use process::Process;

pub const CASE_FILE: &str = "case.json";
pub const FIXTURE: &str = "fixture";
const JUDGE_MODE: &str = "plan";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub text: String,
    pub cost_usd: f64,
}

pub trait Asker {
    fn ask(&self, dir: &Path, prompt: &str, mode: Option<&str>) -> Result<Answer, String>;
}

pub fn folder(project: &Path) -> PathBuf {
    project.join(brand::DIR).join("evals")
}

pub fn cases(project: &Path, wanted: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let folder = folder(project);
    let entries = std::fs::read_dir(&folder).map_err(|error| {
        format!("No evals in {}: add <name>/{CASE_FILE} and an optional <name>/{FIXTURE}/ ({error})", folder.display())
    })?;
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.path().join(CASE_FILE).is_file())
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .filter(|(name, _)| wanted.is_empty() || wanted.contains(name))
        .collect();
    found.sort();
    if found.is_empty() {
        return Err(format!("No eval case matched in {}", folder.display()));
    }
    Ok(found)
}

pub fn run_case(asker: &dyn Asker, name: &str, case_dir: &Path) -> Outcome {
    let failed =
        |note: String| Outcome { name: name.to_owned(), passed: 0, total: 1, cost_usd: 0.0, notes: vec![note] };
    let case = match std::fs::read_to_string(case_dir.join(CASE_FILE))
        .map_err(|e| e.to_string())
        .and_then(|t| Case::parse(&t))
    {
        Ok(case) => case,
        Err(error) => return failed(error),
    };
    let work = match tempfile::tempdir() {
        Ok(work) => work,
        Err(error) => return failed(format!("no scratch folder: {error}")),
    };
    let fixture = case_dir.join(FIXTURE);
    if let Err(error) = files::copy(&fixture, work.path()) {
        return failed(format!("could not copy the fixture: {error}"));
    }
    let answer = match asker.ask(work.path(), &case.prompt, case.mode.as_deref()) {
        Ok(answer) => answer,
        Err(error) => return failed(format!("the agent failed: {error}")),
    };
    let found = Found {
        answer: answer.text.clone(),
        files: case
            .checks
            .iter()
            .filter_map(Check::file)
            .map(|path| (path.to_owned(), std::fs::read_to_string(work.path().join(path)).ok()))
            .collect(),
    };
    let mut outcome =
        Outcome { name: name.to_owned(), passed: 0, total: 0, cost_usd: answer.cost_usd, notes: Vec::new() };
    for check in &case.checks {
        let passed = check.judge(&found).unwrap_or_else(|| match check {
            Check::Command(command) => files::succeeds(work.path(), command),
            _ => false,
        });
        outcome.total += 1;
        if passed {
            outcome.passed += 1;
        } else {
            outcome.notes.push(format!("failed: {}", check.describe()));
        }
    }
    if let Some(rubric) = &case.rubric {
        let changes = files::changed(&fixture, work.path()).join("\n");
        let prompt = judge_prompt(rubric, &case.prompt, &answer.text, &changes);
        graded(asker, work.path(), &prompt, &mut outcome);
    }
    outcome
}

fn graded(asker: &dyn Asker, dir: &Path, prompt: &str, outcome: &mut Outcome) {
    outcome.total += 1;
    match asker.ask(dir, prompt, Some(JUDGE_MODE)) {
        Ok(reply) => {
            outcome.cost_usd += reply.cost_usd;
            match judged(&reply.text) {
                Some((true, _)) => outcome.passed += 1,
                Some((false, why)) => outcome.notes.push(format!("rubric: {why}")),
                None => outcome.notes.push("rubric: the judge answered out of form".into()),
            }
        }
        Err(error) => outcome.notes.push(format!("rubric: the judge failed: {error}")),
    }
}

#[cfg(test)]
mod tests;
