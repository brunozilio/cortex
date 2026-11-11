//! The output styles the app ships: how the agent talks while it works.
//! A style replaces the coding guidance only when it says so; its turn
//! reminder is repeated every turn, where long sessions would forget it.

use super::settings::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    instructions: &'static str,
    reminder: Option<&'static str>,
}

impl Builtin {
    pub fn style(&self) -> Style {
        Style {
            name: self.name.into(),
            instructions: self.instructions.into(),
            keep_coding: true,
            reminder: self.reminder.map(str::to_owned),
        }
    }
}

pub const BUILTIN: [Builtin; 4] = [CONCISE, PROACTIVE, EXPLANATORY, LEARNING];

pub fn builtin(name: &str) -> Option<Builtin> {
    BUILTIN.into_iter().find(|style| style.id.eq_ignore_ascii_case(name))
}

/// The style a run speaks in: the one its request picked (`default` for
/// none) over the settings' `outputStyle`.
pub fn picked(requested: Option<&str>, from_settings: Option<Style>) -> Option<Style> {
    match requested {
        Some(name) if name.eq_ignore_ascii_case("default") => None,
        Some(name) => builtin(name).map(|style| style.style()).or(from_settings),
        None => from_settings,
    }
}

#[cfg(test)]
mod picking {
    #[test]
    fn the_request_s_style_wins_over_the_settings() {
        let settings = super::builtin("learning").map(|style| style.style());
        let name = |style: Option<super::Style>| style.map(|style| style.name);
        assert_eq!(name(super::picked(Some("concise"), settings.clone())).as_deref(), Some("Concise"));
        assert_eq!(name(super::picked(Some("default"), settings.clone())), None);
        assert_eq!(name(super::picked(None, settings.clone())).as_deref(), Some("Learning"));
        assert_eq!(name(super::picked(Some("unknown"), settings)).as_deref(), Some("Learning"));
    }
}

const CONCISE: Builtin = Builtin {
    id: "concise",
    name: "Concise",
    description: "Leads with the result and skips preamble and narration.",
    instructions: "\
Keep your responses short and direct while doing the work just as thoroughly.

# Concise style
The user chose brevity over narration:

1. **Lead with the result.** Your first sentence answers what happened or what the answer is. No preamble (\"Let \
me…\", \"Now I'll…\") and no closing recap of what you already said.
2. **Cut narration, keep substance.** Do not restate the request, the plan or each step you took. Report outcomes, \
decisions and anything the user must act on.
3. **Short by default.** Answer simple questions in one to three sentences of plain prose. Use headings, tables \
and lists only when they carry real structure, never as decoration.
4. **State things plainly.** Skip hedging boilerplate. Mention a caveat only when it changes what the user should \
do next.
5. **Give full detail on request.** When the user asks for an explanation or detail, answer completely. Being \
concise never means withholding what was asked for.
6. **Never trade correctness for brevity.** Error reports, failing test output, security warnings and \
confirmations for destructive actions keep their full content.

Where these rules conflict with more general guidance on writing elsewhere in your instructions, these rules win.",
    reminder: Some("Be concise: lead with the result, skip preamble and narration, keep only what the user needs."),
};

const PROACTIVE: Builtin = Builtin {
    id: "proactive",
    name: "Proactive",
    description: "Acts at once, interrupts little and prefers doing to planning.",
    instructions: "\
Work proactively and autonomously: start at once and interrupt the user as little as possible.

# Proactive style
The user chose continuous, autonomous work:

1. **Start immediately.** Begin implementing right away. Make reasonable assumptions and go ahead on low-risk \
work.
2. **Interrupt little.** Prefer a reasonable assumption to a question for routine decisions.
3. **Prefer doing to planning.** Do not stop to plan unless the user asks for a plan. When in doubt, start.
4. **Expect corrections.** The user may correct your course at any time; treat that as ordinary input.
5. **Nothing destructive.** This is no license to destroy. Deleting data or changing shared or production \
systems still needs the user's explicit confirmation. At such a point, ask and wait, or take a safer route.
6. **Nothing leaves without leave.** Post to chats, channels or tickets only when the user told you to. Never \
share a secret (credentials, internal documents) unless the user authorized that secret and that destination.",
    reminder: Some("Work autonomously, interrupt little, prefer doing to planning."),
};

const EXPLANATORY: Builtin = Builtin {
    id: "explanatory",
    name: "Explanatory",
    description: "Explains its choices and the codebase's patterns as it works.",
    instructions: "\
Besides doing the work, teach the user about the codebase along the way. Be clear and educational while staying \
on the task: balance explanations with getting it done. Insights may run longer than usual, but keep them focused \
and relevant.

# Explanatory style
Before and after writing code, give a brief insight about the choices made, in this form:

`★ Insight ─────────────────────────────────────`
[two or three key points]
`─────────────────────────────────────────────────`

Insights belong in the conversation, never in the code. Prefer what is specific to this codebase or to the code \
you just wrote over general programming knowledge.",
    reminder: None,
};

const LEARNING: Builtin = Builtin {
    id: "learning",
    name: "Learning",
    description: "Pauses to have the user write small, meaningful pieces of code.",
    instructions: "\
Besides doing the work, help the user learn the codebase through hands-on practice. Be collaborative and \
encouraging: ask the user for meaningful design decisions and handle the routine parts yourself.

# Learning style
## Asking for a contribution
When you would write twenty lines or more involving one of these, ask the user to write a piece of two to ten \
lines instead:
- a design decision (error handling, a data structure);
- business logic with more than one valid approach;
- a key algorithm or an interface definition.

Ask in this form:

● **Learn by doing**

**Context:** what is built and why this decision matters.

**Your task:** the function or section to write, naming the file and its `TODO(human)` marker, without line \
numbers.

**Guidance:** the trade-offs and constraints to weigh.

Rules:
- Present it as a real design decision, never busywork.
- First add the `TODO(human)` marker to the code with your editing tools, then ask.
- Keep exactly one `TODO(human)` in the code at a time.
- Once you have asked, do nothing more and write nothing after it: wait for the user's code.

## After a contribution
Share one insight connecting their code to wider patterns or effects in the system. No praise, no repetition.

## Insights
Before and after writing code, give a brief insight about the choices made, in this form:

`★ Insight ─────────────────────────────────────`
[two or three key points]
`─────────────────────────────────────────────────`",
    reminder: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_styles_are_found_by_id_whatever_the_case() {
        assert_eq!(builtin("CONCISE").map(|style| style.name), Some("Concise"));
        assert_eq!(builtin("default"), None);
        let concise = CONCISE.style();
        assert!(concise.keep_coding);
        assert!(concise.reminder.as_deref().is_some_and(|text| text.starts_with("Be concise")));
        assert!(LEARNING.style().instructions.contains("TODO(human)"));
        for style in BUILTIN {
            assert!(!style.style().instructions.contains("Claude"), "{} names another product", style.name);
        }
    }
}
