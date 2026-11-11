use domain::{Choice, Question};
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub const ASK_TOOL: &str = "AskUserQuestion";

const MAX_QUESTIONS: usize = 4;
const MAX_OPTIONS: usize = 4;

#[derive(Debug, Clone, Copy, Default)]
pub struct AskUserQuestion;

impl Tool for AskUserQuestion {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ASK_TOOL.to_owned(),
            description: "Ask the user one to four questions, each with two to four options, when you cannot go on \
                          without a decision that is genuinely theirs. The user can also answer in their own words. \
                          If you recommend an option, put it first and end its label with \"(Recommended)\". \
                          Ask only when the answer changes what you do next: not for a choice with a conventional \
                          default nor for a fact you can check in the code yourself; then pick the obvious option, \
                          say so and go on. The user may see only the question, so make each one stand on its own. \
                          In planning mode, ask to clarify requirements or choose between approaches, never \
                          whether the plan is ready or whether to proceed."
                .to_owned(),
            schema: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array", "minItems": 1, "maxItems": MAX_QUESTIONS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": { "type": "string", "description": "The whole question, ending with a question mark." },
                                "header": { "type": "string", "description": "A tag of at most 12 characters." },
                                "multiSelect": { "type": "boolean", "default": false },
                                "options": {
                                    "type": "array", "minItems": 2, "maxItems": MAX_OPTIONS,
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": { "type": "string" },
                                            "description": { "type": "string" },
                                        },
                                        "required": ["label", "description"],
                                    },
                                },
                            },
                            "required": ["question", "header", "options", "multiSelect"],
                        },
                    },
                },
                "required": ["questions"],
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        input["questions"][0]["question"].as_str().unwrap_or_default().chars().take(80).collect()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Questions can only be asked by the agent loop.")
    }
}

pub fn questions(input: &Value) -> Result<Vec<Question>, String> {
    let asked = input["questions"].as_array().filter(|q| !q.is_empty()).ok_or("`questions` must list at least one")?;
    asked
        .iter()
        .take(MAX_QUESTIONS)
        .map(|question| {
            let options: Vec<Choice> = question["options"]
                .as_array()
                .into_iter()
                .flatten()
                .take(MAX_OPTIONS)
                .filter_map(|option| {
                    Some(Choice {
                        label: option["label"].as_str()?.to_owned(),
                        description: option["description"].as_str().unwrap_or_default().to_owned(),
                    })
                })
                .collect();
            let text = question["question"].as_str().ok_or("every question needs its `question`")?;
            if options.len() < 2 {
                return Err(format!("\"{text}\" needs at least two options"));
            }
            Ok(Question {
                header: question["header"].as_str().unwrap_or_default().to_owned(),
                text: text.to_owned(),
                options,
                multi: question["multiSelect"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

pub fn answers(questions: &[Question], picks: &[Vec<usize>]) -> String {
    let lines: Vec<String> = questions
        .iter()
        .enumerate()
        .map(|(at, question)| {
            let chosen: Vec<&str> = picks
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|&pick| question.options.get(pick).map(|choice| choice.label.as_str()))
                .collect();
            let answer = if chosen.is_empty() { "(no answer)".to_owned() } else { chosen.join(", ") };
            format!("\"{}\" = \"{answer}\"", question.text)
        })
        .collect();
    format!("The user answered: {}. Go on with these answers in mind.", lines.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Value {
        json!({ "questions": [{
            "question": "Which database?", "header": "Database", "multiSelect": false,
            "options": [
                { "label": "SQLite (Recommended)", "description": "One file." },
                { "label": "Postgres", "description": "A server." },
            ],
        }] })
    }

    #[test]
    fn reads_questions_for_the_card_and_answers_for_the_model() {
        let asked = questions(&input()).expect("questions");
        assert_eq!(asked[0].header, "Database");
        assert_eq!(asked[0].options[1].label, "Postgres");
        assert!(!asked[0].multi);
        assert_eq!(
            answers(&asked, &[vec![1]]),
            "The user answered: \"Which database?\" = \"Postgres\". Go on with these answers in mind."
        );
        assert!(answers(&asked, &[]).contains("(no answer)"));
    }

    #[test]
    fn refuses_a_question_without_two_options() {
        assert!(questions(&json!({ "questions": [] })).is_err());
        let one = json!({ "questions": [{ "question": "Sure?", "header": "x", "options": [{ "label": "Yes" }] }] });
        assert!(questions(&one).expect_err("one option").contains("two options"));
    }
}
