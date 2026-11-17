# 0009 Function-size lint replaces the 300-line file limit

Status: Accepted, 2026-11-16

## Context

The repository lint (`xtask/src/lint.rs`) failed any file over 300 lines, and the working agreement asked for files of 200 to 300 lines. The rule was met mechanically: no file is over 300 lines and 21 sit between 280 and 300.

The audit measured what that bought:

- `impl Agent` is spread over 19 files, `impl App` over 14, `impl Transcript` over 11. The blocks were cut to fit, not separated by responsibility, so any function can touch any field and following one tool call means opening five files.
- Functions of 190 to 269 lines fill a whole file each: `Job::run`, `Agent::sub_agent_with`, `Job::converse`, `Agent::call`, `settings::load`, `Agent::run_loop`.
- Tests were pushed into sibling files to stay under the limit.

The lint counted lines in a file. It said nothing about cohesion or about the size of a function, which is where the complexity is.

## Decision

- The file-length check is removed. File length is not a goal.
- Function size and complexity are linted by clippy: `too_many_lines` with a threshold of 100 lines and `cognitive_complexity`, both configured in `clippy.toml` and enabled for the workspace. Since clippy runs with warnings denied, a function over the threshold fails the gate.
- The known oversized functions carry an explicit `#[allow]`. That list only shrinks: a new allow is not an accepted way to pass the gate, and each allow is removed when its function is split (mostly with the state machine of `0002-agent-loop-state-machine.md`).
- A type's `impl` is kept in a few cohesive modules. `impl Agent` goes from 19 files to about five; `impl App` is to be regrouped around sub-states (not scheduled as of 2026-11-17).

As of 2026-11-17 the only allows left are on two test functions.

## Consequences

- The lint measures the thing that hurts: long functions that mix concerns.
- Related methods can sit together, so a reader opens one module for one responsibility.
- Tests can live beside the code they test without a line budget pushing them out.

What gets worse:

- Nothing mechanical stops a file from growing to a thousand lines of short functions; keeping modules cohesive is now a review judgement, not a check.
- A line threshold on functions can be gamed the same way the file limit was, by slicing a function into helpers that only make sense together.
- `cognitive_complexity` is a heuristic and will flag some clear `match` tables while passing some tangled code.
- While the allow list was long, it made the largest functions exempt, so the worst offenders were the ones the lint did not check.
- Regrouping `impl` blocks is pure movement across many files and conflicts with any other change to those files.
