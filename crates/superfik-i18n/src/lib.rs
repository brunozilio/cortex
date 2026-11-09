//! Interface languages. Every string the user reads goes through
//! [`Strings`]: a [`Text`] key (with typed parameters) or a domain value
//! from `superfik-core` (activity, day, effort, agent, step, engine
//! problem) in, the sentence in the chosen [`Language`] out.
//!
//! Each language is one file implementing [`Catalog`]; a missing
//! translation does not compile. Adding a language is a [`Language`]
//! variant plus its file. Text sent to the model (agent briefs, skill
//! instructions) and brand names (Claude Code, Ultracode) stay as they are.

mod catalog;
mod days;
mod en;
mod language;
mod pt_br;
mod strings;
mod text;

pub use catalog::Catalog;
pub use language::Language;
pub use strings::Strings;
pub use text::Text;
