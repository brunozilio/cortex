# 0002 The agent loop as an explicit state machine

Status: Implemented, 2026-11-17 (accepted 2026-11-16): the run's phases are in `crates/agent-engine/src/engine/phases/`, `TurnState` and `Next` in `crates/agent-engine/src/agent/turn/mod.rs`, the golden tests in `crates/agent-engine/tests/golden.rs`

## Context

One run is driven by a handful of very long functions spread over several files:

- `Job::run` (`crates/agent-engine/src/engine/job.rs`) and `Job::converse` (`crates/agent-engine/src/engine/converse.rs`) are in effect one function of about 500 lines that loads the session, builds the Brief, equips tools, applies policy, opens the model and drives the loop.
- `Agent::run_loop` (`crates/agent-engine/src/agent/mod.rs`) is about 190 lines with the end-of-turn conditions written as an ordered chain of checks.
- `Agent::call` (`crates/agent-engine/src/agent/dispatch.rs`) and `permitted_as` (`crates/agent-engine/src/agent/permission.rs`) are 200 and 116 lines.

The 300-line file limit was met by spreading `impl Agent` over 19 files, so following one tool call means opening five of them. The audit found bugs that live in the gaps between these steps: an index fixed before the loop and used after a compaction shortened the history, and loops (structured output, a blocking `Stop` hook) with no cap of their own. The conversation is saved only at the end of the run because there is no earlier point that is clearly "between steps".

## Decision

A run becomes a sequence of typed phases, and the loop becomes a state machine.

- Run phases, each a function with its own input and output type: `load_session`, `brief`, `equip`, `apply_policy`, `prepare_runtime`, `open`, `drive`, `close`.
- A `TurnState` with named transitions replaces `Agent::run_loop`: awaiting the model, awaiting a permission answer, running tools, finishing.
- Each transition is the point where the conversation is persisted (`0006-one-persistence-pattern.md`).
- Before any of this moves, transcript replay ("golden") tests are added, driven through the engine's public start call with a scripted model: a turn with a tool, compaction, fallback, refusal, a blocking `Stop` hook, structured output, worktree.
- `impl Agent` is regrouped from 19 files into about five cohesive modules (`0009-function-size-lint.md`).

## Consequences

- Each transition can be tested alone, and the golden tests fix the observable behaviour while the code moves.
- "Save after every transition" has an obvious place.
- Caps and invariants (for example, every tool use is followed by its result) can be checked at a transition instead of being implicit.

What gets worse:

- More types and more plumbing: state that was a local variable becomes a field that must be carried from phase to phase.
- This rewrites the most central and most tested code in the repository. The work is long, and until it is done the two shapes coexist.
- The golden tests pin today's behaviour, including behaviour nobody chose; changing it later means updating recorded transcripts.
- A state machine makes illegal orders unrepresentable only if the states are kept honest; a catch-all state would bring the old function back under a new name.
