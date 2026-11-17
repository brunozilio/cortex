pub const MARK: &str = "# Follow-through";
pub const MOST_NUDGES: usize = 2;

const ENDING_CHARS: usize = 300;
const PROMISES: [&str; 11] = [
    "i'll ",
    "i will ",
    "let me ",
    "i'm going to ",
    "i am going to ",
    "next i ",
    "vou ",
    "deixa eu ",
    "deixe-me ",
    "em seguida",
    "voy a ",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judgement {
    Done,
    Waiting,
    Continue(String),
}

impl Judgement {
    pub fn parse(reply: &str) -> Option<Self> {
        let start = reply.find('{')?;
        let end = reply.rfind('}')?;
        let value: serde_json::Value = serde_json::from_str(reply.get(start..=end)?).ok()?;
        match value["verdict"].as_str()? {
            "done" => Some(Self::Done),
            "waiting" => Some(Self::Waiting),
            "continue" => {
                let nudge = value["nudge"].as_str().map(str::trim).filter(|nudge| !nudge.is_empty())?;
                Some(Self::Continue(nudge.to_owned()))
            }
            _ => None,
        }
    }

    pub fn nudge(&self) -> Option<String> {
        let Self::Continue(nudge) = self else { return None };
        Some(format!(
            "{MARK}\nYour answer said there was more to do, but the turn was ending. {nudge}\nCarry it out now, or say plainly why it cannot be done."
        ))
    }
}

pub fn worth_judging(answer: &str, nudged: usize) -> bool {
    let answer = answer.trim();
    if nudged >= MOST_NUDGES || answer.is_empty() || answer.ends_with('?') {
        return false;
    }
    let skipped = answer.chars().count().saturating_sub(ENDING_CHARS);
    let ending: String = answer.chars().skip(skipped).collect::<String>().to_lowercase().replace(',', "");
    PROMISES.iter().any(|promise| ending.contains(promise))
}

pub fn nudges<'a>(user_texts: impl IntoIterator<Item = &'a str>) -> usize {
    user_texts.into_iter().filter(|text| text.starts_with(MARK)).count()
}

#[cfg(test)]
mod tests;
