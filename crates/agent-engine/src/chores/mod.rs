mod clean;
mod heads_up;

use std::sync::atomic::AtomicBool;

use domain::{ChoreAnswer, SessionCommand};

use self::clean::field;
pub use self::clean::{clean_suggestion, clean_title, conversation_text, kebab, spoken};
use crate::model::{ModelApi, ModelError, TurnRequest};
use crate::transcript::Message;






pub fn answer(
    command: SessionCommand,
    api: &dyn ModelApi,
    template: &TurnRequest,
    prompt: &str,
    history: &[Message],
    stop: &AtomicBool,
) -> Result<Option<ChoreAnswer>, ModelError> {
    Ok(match command {
        SessionCommand::Title => title(api, template, prompt, stop)?.map(ChoreAnswer::Title),
        SessionCommand::Suggest => suggestion(api, template, history, stop)?.map(ChoreAnswer::Suggestion),
        SessionCommand::Recap => recap(api, template, history, stop)?.map(ChoreAnswer::Recap),
        SessionCommand::Side => Some(ChoreAnswer::Side(side_answer(api, template, history, prompt, stop)?)),
        SessionCommand::HeadsUp => heads_up::look(api, template, history, stop)?.map(ChoreAnswer::HeadsUp),
        SessionCommand::Rename => {
            branch_name(api, template, history, stop)?.map(|name| ChoreAnswer::Title(spoken(&name)))
        }
        SessionCommand::MeasureContext | SessionCommand::Compact | SessionCommand::Rewind(_) => None,
    })
}

const BRANCH_TAIL: usize = 1000;
const RECAP_TAIL: usize = 12_000;

fn ask(
    api: &dyn ModelApi,
    template: &TurnRequest,
    system: String,
    messages: Vec<Message>,
    max_tokens: u32,
    stop: &AtomicBool,
) -> Result<String, ModelError> {
    let request = TurnRequest {
        system,
        messages,
        tools: Vec::new(),
        max_tokens: template.max_tokens.min(max_tokens),
        ..template.clone()
    };
    Ok(api.turn(&request, &mut |_| {}, stop)?.message.text())
}

pub fn title(
    api: &dyn ModelApi,
    template: &TurnRequest,
    prompt: &str,
    stop: &AtomicBool,
) -> Result<Option<String>, ModelError> {
    let message = Message::user(format!("<session>\n{}\n</session>", prompt.trim()));
    let answer = ask(api, template, crate::prompt::text("chore-title"), vec![message], 200, stop)?;
    Ok(clean_title(&field(&answer, "title")))
}

pub fn branch_name(
    api: &dyn ModelApi,
    template: &TurnRequest,
    history: &[Message],
    stop: &AtomicBool,
) -> Result<Option<String>, ModelError> {
    let message =
        Message::user(format!("<conversation>\n{}\n</conversation>", conversation_text(history, BRANCH_TAIL)));
    let answer = ask(api, template, crate::prompt::text("chore-branch"), vec![message], 100, stop)?;
    Ok(kebab(&field(&answer, "name")))
}

pub fn recap(
    api: &dyn ModelApi,
    template: &TurnRequest,
    history: &[Message],
    stop: &AtomicBool,
) -> Result<Option<String>, ModelError> {
    let message = Message::user(format!("<conversation>\n{}\n</conversation>", conversation_text(history, RECAP_TAIL)));
    let answer = ask(api, template, crate::prompt::text("chore-recap"), vec![message], 300, stop)?;
    let answer = answer.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok((!answer.is_empty()).then_some(answer))
}

pub fn suggestion(
    api: &dyn ModelApi,
    template: &TurnRequest,
    history: &[Message],
    stop: &AtomicBool,
) -> Result<Option<String>, ModelError> {
    let mut messages = history.to_vec();
    messages.push(Message::user(crate::prompt::text("chore-suggest")));
    let answer = ask(api, template, template.system.clone(), messages, 60, stop)?;
    Ok(clean_suggestion(&answer))
}

pub fn side_answer(
    api: &dyn ModelApi,
    template: &TurnRequest,
    history: &[Message],
    question: &str,
    stop: &AtomicBool,
) -> Result<String, ModelError> {
    let mut messages = history.to_vec();
    while messages.last().is_some_and(|message| message.tool_uses().next().is_some()) {
        messages.pop();
    }
    messages.push(Message::user(format!("{}\n{}", crate::prompt::text("chore-side"), question.trim())));
    let answer = ask(api, template, template.system.clone(), messages, 4096, stop)?;
    if answer.trim().is_empty() {
        return Err(ModelError::Provider("the model gave no answer".into()));
    }
    Ok(answer.trim().to_owned())
}

#[cfg(test)]
mod tests;
