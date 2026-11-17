use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};
use crate::transcript::{Block, Message, StopReason, Usage};

#[derive(Debug, Default)]
pub struct ScriptedModel {
    replies: Mutex<VecDeque<Result<Reply, ModelError>>>,
    asked: Mutex<Vec<TurnRequest>>,
}

impl ScriptedModel {
    pub fn new(replies: impl IntoIterator<Item = Result<Reply, ModelError>>) -> Self {
        Self { replies: Mutex::new(replies.into_iter().collect()), asked: Mutex::default() }
    }

    pub fn reply(blocks: Vec<Block>) -> Result<Reply, ModelError> {
        let message = Message::assistant(blocks);
        let stop = if message.tool_uses().next().is_some() { StopReason::ToolUse } else { StopReason::EndTurn };
        Ok(Reply { message, stop, usage: Usage { input: 10, output: 5, ..Usage::default() } })
    }

    pub fn asked(&self) -> Vec<TurnRequest> {
        self.asked.lock().map(|asked| asked.clone()).unwrap_or_default()
    }
}

impl ModelApi for ScriptedModel {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), _stop: &AtomicBool) -> Result<Reply, ModelError> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(request.clone());
        }
        let reply = self
            .replies
            .lock()
            .ok()
            .and_then(|mut replies| replies.pop_front())
            .unwrap_or_else(|| Err(ModelError::Provider("the script ran out".into())))?;
        for block in &reply.message.blocks {
            if let Block::Text(text) = block {
                on(Delta::Text(text.clone()));
            }
        }
        Ok(reply)
    }
}
