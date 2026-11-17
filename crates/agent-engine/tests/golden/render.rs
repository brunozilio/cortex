use agent_engine::sessions::Entry;
use agent_engine::transcript::{Block, Message, Role};
use domain::{AgentStatus, EngineEvent};

const SHOWN: usize = 180;

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

pub fn clipped(line: &str) -> String {
    if line.chars().count() <= SHOWN {
        return line.to_owned();
    }
    let head: String = line.chars().take(SHOWN).collect();
    format!("{head}…")
}

fn merged(events: &[EngineEvent]) -> Vec<EngineEvent> {
    let mut joined: Vec<EngineEvent> = Vec::new();
    for event in events {
        if let Some(last) = joined.last_mut()
            && join(last, event)
        {
            continue;
        }
        joined.push(event.clone());
    }
    joined
}

fn join(last: &mut EngineEvent, next: &EngineEvent) -> bool {
    match (last, next) {
        (EngineEvent::TextDelta(said), EngineEvent::TextDelta(more))
        | (EngineEvent::ThinkingDelta(said), EngineEvent::ThinkingDelta(more)) => {
            said.push_str(more);
            true
        }
        (EngineEvent::AgentEvent { index, event }, EngineEvent::AgentEvent { index: other, event: more })
            if index == other =>
        {
            join(event, more)
        }
        _ => false,
    }
}

pub fn events(events: &[EngineEvent]) -> Vec<String> {
    merged(events).iter().map(event).collect()
}

pub fn lanes(events: &[EngineEvent]) -> Vec<String> {
    let mut main: Vec<Vec<EngineEvent>> = vec![Vec::new()];
    let mut usage = Vec::new();
    let mut agents: std::collections::BTreeMap<usize, Vec<EngineEvent>> = std::collections::BTreeMap::new();
    for event in events {
        match event {
            EngineEvent::Usage(_) => {
                usage.push(event.clone());
                main.push(Vec::new());
            }
            EngineEvent::AgentEvent { index, event } => agents.entry(*index).or_default().push((**event).clone()),
            EngineEvent::AgentSpawned { index, .. } | EngineEvent::AgentUpdate { index, .. } => {
                agents.entry(*index).or_default().push(event.clone());
            }
            other => main.last_mut().into_iter().for_each(|said| said.push(other.clone())),
        }
    }
    let mut lines = vec!["[main lane]".to_owned()];
    lines.extend(main.iter().flat_map(|said| self::events(said)));
    for (index, lane) in agents {
        lines.push(format!("[agent #{index} lane]"));
        lines.extend(self::events(&lane));
    }
    lines.push("[main lane usage]".to_owned());
    lines.extend(self::events(&usage));
    lines
}

pub fn calls_grouped(events: &[EngineEvent]) -> Vec<String> {
    let mut ordered: Vec<EngineEvent> = Vec::new();
    let mut batch: Vec<(String, EngineEvent)> = Vec::new();
    for event in events {
        match call_of(event) {
            Some(id) => batch.push((id, event.clone())),
            None => {
                flush(&mut batch, &mut ordered);
                ordered.push(event.clone());
            }
        }
    }
    flush(&mut batch, &mut ordered);
    self::events(&ordered)
}

fn flush(batch: &mut Vec<(String, EngineEvent)>, ordered: &mut Vec<EngineEvent>) {
    batch.sort_by(|(a, _), (b, _)| a.cmp(b));
    ordered.extend(batch.drain(..).map(|(_, event)| event));
}

fn call_of(event: &EngineEvent) -> Option<String> {
    match event {
        EngineEvent::ToolStarted(call) => Some(call.id.clone()),
        EngineEvent::ToolFinished { id }
        | EngineEvent::ToolResult { id, .. }
        | EngineEvent::ToolProgress { id, .. }
        | EngineEvent::ToolLog { id, .. } => Some(id.clone()),
        _ => None,
    }
}

pub fn event(event: &EngineEvent) -> String {
    match event {
        EngineEvent::SessionStarted(id) => format!("SessionStarted {}", id.0),
        EngineEvent::Usage(usage) => format!(
            "Usage model={} input={} output={} cost={}",
            usage.model,
            usage.input,
            usage.output,
            if usage.estimated_cost_usd.is_some() { "<usd>" } else { "none" }
        ),
        EngineEvent::StructuredOutput(value) => format!("StructuredOutput {value}"),
        EngineEvent::TextDelta(text) => format!("Text {}", quoted(text)),
        EngineEvent::ThinkingDelta(text) => format!("Thinking {}", quoted(text)),
        EngineEvent::ToolStarted(call) => format!(
            "ToolStarted {} {} {} input={}",
            call.id,
            call.name,
            quoted(&call.summary),
            call.input.as_ref().map_or("-".to_owned(), |input| quoted(&input.to_string()))
        ),
        EngineEvent::ToolProgress { id, line } => format!("ToolProgress {id} {}", quoted(line)),
        EngineEvent::ToolFinished { id } => format!("ToolFinished {id}"),
        EngineEvent::ToolResult { id, output, failed } => {
            format!("ToolResult {id} {} {}", if *failed { "failed" } else { "ok" }, quoted(output))
        }
        EngineEvent::AgentSpawned { index, kind, summary, prompt } => {
            format!("AgentSpawned #{index} kind={kind} summary={summary:?} prompt={}", quoted(prompt))
        }
        EngineEvent::AgentEvent { index, event } => format!("Agent #{index} | {}", self::event(event)),
        EngineEvent::AgentUpdate { index, status, summary } => {
            let status = match status {
                AgentStatus::Running { progress } => format!("Running({progress})"),
                AgentStatus::Done { .. } => "Done(<seconds>)".to_owned(),
                AgentStatus::Failed => "Failed".to_owned(),
                AgentStatus::Cancelled => "Cancelled".to_owned(),
            };
            format!("AgentUpdate #{index} {status} summary={summary:?}")
        }
        EngineEvent::Compacted(compaction) => format!(
            "Compacted before=<tokens> after=<tokens> shrank={} summary={}",
            compaction.after < compaction.before,
            quoted(&compaction.summary)
        ),
        EngineEvent::Asked(ask) => format!("Asked {} {:?}", ask.id.0, ask.kind),
        EngineEvent::Answered { ask, decision } => format!("Answered {} {decision:?}", ask.0),
        EngineEvent::AskWithdrawn(ask) => format!("AskWithdrawn {}", ask.0),
        EngineEvent::Completed { cost_usd, .. } => {
            format!("Completed duration=<ms> cost={}", if cost_usd.is_some() { "<usd>" } else { "none" })
        }
        other => format!("{other:?}"),
    }
}

fn kind(event: &EngineEvent) -> String {
    match event {
        EngineEvent::AgentEvent { index, event } => format!("Agent #{index} | {}", kind(event)),
        other => {
            let shown = format!("{other:?}");
            let name = shown.split(['(', '{', ' ']).next().unwrap_or_default().to_owned();
            call_of(other).map_or(name.clone(), |id| format!("{name} {id}"))
        }
    }
}

pub fn conversation(history: &[Message]) -> Vec<String> {
    let mut lines = Vec::new();
    for message in history {
        lines.push(match message.role {
            Role::User => "user:".to_owned(),
            Role::Assistant => "assistant:".to_owned(),
        });
        lines.extend(message.blocks.iter().map(|block| format!("  {}", self::block(block))));
    }
    lines
}

fn block(block: &Block) -> String {
    let outcome = |is_error: &bool| if *is_error { "failed" } else { "ok" };
    match block {
        Block::Text(text) => format!("text {}", quoted(text)),
        Block::Image(_) => "image".to_owned(),
        Block::Thinking { text, .. } => format!("thinking {}", quoted(text)),
        Block::ToolUse { id, name, input } => format!("tool_use {id} {name} {}", quoted(&input.to_string())),
        Block::ToolResult { id, content, is_error } => {
            format!("tool_result {id} {} {}", outcome(is_error), quoted(content))
        }
        Block::ToolResultRich { id, content, images, is_error } => {
            format!("tool_result_rich {id} {} images={} {}", outcome(is_error), images.len(), quoted(content))
        }
        Block::Reminder(text) => format!("reminder {}", quoted(text)),
        Block::Native { .. } => "native".to_owned(),
    }
}

fn entry(entry: &Entry) -> String {
    match entry {
        Entry::Message(message) => format!("message {:?}", message.role).to_lowercase(),
        Entry::Truncated { keep } => format!("truncated keep={keep}"),
        Entry::Amended { keep } => format!("amended keep={keep}"),
        Entry::Restarted => "restarted".to_owned(),
        Entry::Event { event, .. } => format!("event {}", kind(event)),
        Entry::Prompted { prompt, .. } => format!("prompted {}", quoted(prompt)),
        Entry::Shown(_) => "shown".to_owned(),
        Entry::Called { agent, call, .. } => format!(
            "called model={} stop={} messages={}{}",
            call.model,
            call.stop,
            call.messages,
            agent.map_or(String::new(), |index| format!(" agent=#{index}"))
        ),
        Entry::Opened { .. } => "opened".to_owned(),
    }
}

pub fn line(line: &str) -> String {
    let kept: serde_json::Value = serde_json::from_str(line).expect("a whole line of JSON");
    let kind = kept["kind"].as_str().unwrap_or("?");
    let numbered = if kept["seq"].is_u64() { "numbered" } else { "unnumbered" };
    let what = match kind {
        "message" => kept["role"].as_str().unwrap_or("?").to_lowercase(),
        "event" => match &kept["event"] {
            serde_json::Value::Object(named) => named.keys().next().cloned().unwrap_or_default(),
            other => other.as_str().unwrap_or("?").to_owned(),
        },
        _ => String::new(),
    };
    format!("{kind} {what}").trim_end().to_owned() + " (" + numbered + ")"
}

pub fn log(entries: &[Entry]) -> Vec<String> {
    entries.iter().map(entry).collect()
}

pub fn log_calls_grouped(entries: &[Entry]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut batch: Vec<(String, String)> = Vec::new();
    for kept in entries {
        let call = match kept {
            Entry::Event { event, .. } => call_of(event),
            _ => None,
        };
        match call {
            Some(id) => batch.push((id, entry(kept))),
            None => {
                batch.sort_by(|(a, _), (b, _)| a.cmp(b));
                lines.extend(batch.drain(..).map(|(_, line)| line));
                lines.push(entry(kept));
            }
        }
    }
    batch.sort_by(|(a, _), (b, _)| a.cmp(b));
    lines.extend(batch.into_iter().map(|(_, line)| line));
    lines
}

pub fn log_lanes(entries: &[Entry]) -> Vec<String> {
    let mut main = vec!["[main lane]".to_owned()];
    let mut agents: std::collections::BTreeMap<usize, Vec<String>> = std::collections::BTreeMap::new();
    for kept in entries {
        let lane = match kept {
            Entry::Event {
                event:
                    EngineEvent::AgentEvent { index, .. }
                    | EngineEvent::AgentSpawned { index, .. }
                    | EngineEvent::AgentUpdate { index, .. },
                ..
            } => Some(*index),
            Entry::Called { agent, .. } => *agent,
            _ => None,
        };
        match lane {
            Some(index) => agents.entry(index).or_default().push(entry(kept)),
            None => main.push(entry(kept)),
        }
    }
    for (index, lane) in agents {
        main.push(format!("[agent #{index} lane]"));
        main.extend(lane);
    }
    main
}

pub fn dedented(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let indent = lines.iter().map(|line| line.len() - line.trim_start().len()).min().unwrap_or(0);
    lines.iter().map(|line| line[indent..].trim_end().to_owned()).collect()
}
