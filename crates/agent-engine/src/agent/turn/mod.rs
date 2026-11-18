mod cutoffs;
mod finishing;
mod interception;
mod model_call;
mod reminders;
mod this_prompt;
mod tools;
mod upkeep;

use domain::fallback::Fallback;
use domain::interception::Rejections;
use serde_json::Value;

use self::cutoffs::Cutoffs;
use self::reminders::Reminders;
use self::this_prompt::ThisPrompt;
use super::run::ThisRun;
use super::subagents::background::Spawn;
use super::{Agent, Ending, MAX_TURNS};
use crate::model::{Reply, TurnRequest};
use crate::transcript::{Message, Usage};
use crate::upkeep::Counted;

pub(in crate::agent) type Call = (String, String, Value);

pub(in crate::agent) struct TurnState<'h> {
    history: &'h mut Vec<Message>,
    asked: usize,
    usage: Usage,
    prompt: ThisPrompt,
    counted: Counted,
    compacted_at: Option<usize>,
    recoveries: u8,
    reminders: Reminders,
    fallback: Fallback,
    fell_back: Option<String>,
    rejections: Rejections,
    cutoffs: Cutoffs,
    structured: bool,
    stop_hook_active: bool,
    review_attempts: u8,
}

pub(in crate::agent) struct Exchange {
    request: TurnRequest,
    reply: Reply,
}

pub(in crate::agent) enum Next {
    AskModel,
    Replied(Box<Exchange>),
    RunTools(Vec<Call>),
    Finishing { said: bool },
    Done(Result<Usage, Ending>),
}

impl<'h> TurnState<'h> {
    fn starting(history: &'h mut Vec<Message>, fallbacks: Vec<String>) -> Self {
        Self {
            prompt: ThisPrompt::last_of(history),
            history,
            asked: 0,
            usage: Usage::default(),
            counted: Counted::default(),
            compacted_at: None,
            recoveries: 0,
            reminders: Reminders::default(),
            fallback: Fallback::new(fallbacks),
            fell_back: None,
            rejections: Rejections::default(),
            cutoffs: Cutoffs::default(),
            structured: false,
            stop_hook_active: false,
            review_attempts: 0,
        }
    }
}

impl Agent<'_> {
    pub(in crate::agent) fn turns<'env>(
        &'env self,
        history: &mut Vec<Message>,
        run: &ThisRun,
        spawn: Spawn<'_, 'env>,
    ) -> Result<Usage, Ending> {
        let limit = self.turn_limit.min(MAX_TURNS);
        let began = history.len().saturating_sub(1);
        let mut observed = false;
        let mut turn = TurnState::starting(history, self.workspace.models.fallbacks.clone());
        let mut next = Next::AskModel;
        loop {
            let kept_before_the_call = matches!(next, Next::AskModel);
            let step = match next {
                Next::Done(outcome) if observed => return outcome,
                Next::Done(outcome) => {
                    observed = true;
                    let reports = self.observe(turn.history.get(began..).unwrap_or_default(), &outcome, &run.watching);
                    match self.heard(reports, outcome.is_ok()) {
                        Some(said) => {
                            crate::transcript::told(turn.history, said);
                            Ok(Next::AskModel)
                        }
                        None => return outcome,
                    }
                }
                Next::AskModel if turn.asked == limit => Err(Ending::TooManyTurns),
                Next::AskModel => {
                    turn.asked += 1;
                    self.ask_model(&mut turn, run)
                }
                Next::Replied(exchange) => self.take_reply(&mut turn, *exchange),
                Next::RunTools(calls) => {
                    let ran = self.run_tools(&mut turn, &calls, run, spawn);
                    self.observe_meanwhile(turn.history.get(began..).unwrap_or_default(), &run.watching, spawn);
                    ran
                }
                Next::Finishing { said } => self.finish(&mut turn, run, said),
            };
            next = step.unwrap_or_else(|ending| Next::Done(Err(ending)));
            if !kept_before_the_call {
                run.transition(turn.history);
            }
        }
    }
}

#[cfg(test)]
mod tests;
