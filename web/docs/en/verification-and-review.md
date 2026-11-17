---
title: Verification and review
description: Have a judge check each run against your criteria, let a reviewer read the changes, run the affected tests, and keep the agent from stopping early or going in circles.
---

{{NAME}} can check the agent's work before a run finishes. Some checks you ask for (verification criteria); others run on their own when their flag is on (automatic review, affected tests, follow-through, escalation).

## Verify a run against criteria

Tell {{NAME}} what "done" means, and a **judge** agent checks every run against it before the run finishes:

```text
/verification `cargo test --workspace` passes and the README documents the new --verbose flag
Each run will be judged against these before it finishes (a judge agent runs the checks).
```

While criteria are set, the status line shows `verify (yours)`.

| Command | Effect |
|---|---|
| `/verification` | Show the criteria in force and where they come from |
| `/verification <criteria>` | Set your criteria for this conversation |
| `/verification off` | Clear your criteria |

It is also listed as **Verification** in `/config`.

### How the judge works

At the end of a run in which the agent used at least one tool, the judge gets the criteria, your request, the agent's tool calls (the last 60) and its final message. It works with **Read**, **Glob**, **Grep**, **Bash** and **Skill**, for at most 50 turns, and answers with a verdict: passed or failed, a score from 0 to 1, and one check per criterion with its evidence. The chat shows the verdict:

```text
✓ verification passed (score 0.90)
```

```text
✗ verification failed, attempt 1/3 (score 0.50) · the agent tries again
  ✗ tests: 1 failed
```

When the work fails, the failed checks go back to the agent, which fixes them and finishes again. The judge runs at most **3** times per run. If the judge cannot give a verdict (it stops, or its answer is not valid), the run is marked failed with the reason and the agent is not sent back.

Runs in plan mode are not judged. Sub-agents are not judged on their own; the main run is.

### What the judge may run

The judge never asks you anything: a tool call that would need your approval is refused. Some commands are approved for it in advance:

- every command written in the criteria in backticks, or on its own line in a fenced code block (a leading `$ ` is dropped). `` `cargo test` passes `` lets it run `cargo test`;
- the rules listed under `verification.allow` in your own settings files (`~/{{DIR}}/settings.json`, `settings.local.json`, `managed-settings.json`):

```json
{
  "verification": {
    "allow": ["Bash(make check:*)"]
  }
}
```

A compound command is allowed only when every part of it is.

### Choose the judge's model

The judge uses the model of the **review** role: `judgeModel` in settings, or `/model review <model>` for the session. Without one, it uses the session's model. See [Models and providers](./models-and-providers).

### Criteria from a plan

When you approve a plan that has a **Verification** section (a `## Verification` heading, or a bullet like `- **Verification:** …`), that section becomes the criteria once you leave plan mode, and the status line shows `verify (plan)`. Criteria you set with `/verification` take precedence; clearing yours leaves the plan's in force.

### Verification in headless mode

`--verification` gives criteria to a [headless](./headless) run. It needs `--print`. The verdicts are printed as notes, and the command exits with status 1 when a verdict fails:

```bash
{{COMMAND}} -p --verification '`npm test` passes' "fix the failing date parser test"
```

## Project criteria

> [!NOTE]
> Controlled by the `project-criteria` flag. Default: on.

A project can check in `{{DIR}}/verification.md` with the criteria every run there should meet:

```markdown
- `cargo test --workspace` passes
- `cargo clippy --workspace --all-targets -- -D warnings` reports nothing
- Every new public function has a test
```

{{NAME}} uses the nearest such file, from the folder you work in up through its parents (your own `~/{{DIR}}/verification.md` is not read). The file must not be empty and must be 64 KB or smaller. The criteria apply when you have set none yourself and no plan's apply.

> [!IMPORTANT]
> Commands named in a project's `verification.md` are **not** approved in advance for the judge: a checked-in file cannot grant itself permissions. Only your `verification.allow` rules apply, so add the commands you trust there.

## Automatic code review

> [!NOTE]
> Controlled by the `auto-review` flag. Default: on.

When a run changed files in the project with **Edit**, **Write** or **NotebookEdit**, an independent reviewer reads the changes before the run finishes. It:

- reads each changed file around the changed lines and looks for real defects: wrong logic, a case the change breaks, a missing error path, a test that no longer tests what its name says;
- reads the `docs/` files and instruction files that describe what changed, and reports any statement the change made untrue or any rule it breaks;
- does not report style or things the change did not touch.

The reviewer can only read (**Read**, **Glob**, **Grep**), uses the session's model and has at most 30 turns. Each finding is **high** (a defect, or a documented rule broken) or **low** (out-of-date documentation, a minor risk). The chat lists them:

```text
Review of the changes:
[fix] src/parse.rs:42: the new branch returns before closing the file
[note] docs/cli.md: still says --verbose is a no-op
```

High findings go back to the agent once, as a review from the system that you have not seen; the agent fixes them or says why one is not a problem, and its final answer still answers what you asked. Low findings are only shown. The review runs once per run, only for the main agent. If the reviewer fails, the run finishes without findings.

## Affected tests

> [!EXPERIMENTAL]
> Controlled by the `experimental-affected-tests` flag. Default: on.

After the agent edits or writes a file in the project, {{NAME}} starts the tests that cover it in the background and tells the agent, which gets the result when they end:

```text
<tests>Started cargo test -q -p parser in crates/parser as bash_3 for this change; its result will reach you when it ends.</tests>
```

| Changed file | Tests started |
|---|---|
| `.rs` | `cargo test -q -p <crate>` in the nearest folder with a `Cargo.toml` naming a package, with `CARGO_TARGET_DIR` set to `target/agent-tests` in the project |
| `.go` | `go test .` in the file's folder |
| `.py` | `python -m pytest -q '<test file>'`, where the test file is the file itself when it is named `test_*.py`, otherwise `test_<name>.py` next to it, in a `tests/` folder next to it, or in the project's `tests/` |
| `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs` | `npm test --silent -- '<test file>'` when the project has a `package.json`, where the test file is the file itself when it ends in `.test` or `.spec`, otherwise `<name>.test.<ext>` or `<name>.spec.<ext>` next to it |

Nothing starts when no test is found. When the same tests are already running from an earlier edit, that run is stopped and replaced. The tests run like the agent's own background commands, inside the sandbox when it is on.

## Follow-through

> [!EXPERIMENTAL]
> Controlled by the `experimental-follow-through` flag. Default: on.

Sometimes an answer ends the turn while promising more ("Now I'll run the tests."). After a run in which the agent used a tool, {{NAME}} looks at the last 300 characters of its final answer for a promise: `I'll`, `I will`, `let me`, `I'm going to`, `I am going to`, `next I`, or the Portuguese and Spanish `vou`, `deixa eu`, `deixe-me`, `em seguida`, `voy a`. An answer that ends with a question is left alone.

When it finds one, a small model (the `smallModel`, or the session's model) reads your request and the answer and decides whether the work is done, waiting on something, or should continue. If it should continue, the agent is sent back:

```text
Your answer said there was more to do, but the turn was ending. Run the tests you mentioned.
Carry it out now, or say plainly why it cannot be done.
```

This happens at most twice per run, and only for the main agent.

## Escalation when the agent is stuck

> [!EXPERIMENTAL]
> Controlled by the `experimental-escalation` flag. Default: on.

The agent counts as stuck when the same tool call (same tool, same input) fails 3 times, or when the repeated-call guard has stopped it twice. Help then comes, at most twice per run:

- if an **Advisor** model is set (`advisorModel`) and the Advisor tool is available, the agent is told to call it with what it tried and what failed, and follow its advice;
- otherwise, if `escalationModel` is set, the next turns run on that model and the agent is told to step back and change approach. The chat shows the switch like a fallback.

With neither set, nothing happens. `escalationModel` is read only from your own settings files (`~/{{DIR}}/settings.json`, `settings.local.json`, `managed-settings.json`), never from a checked-in project file:

```json
{
  "advisorModel": "opus",
  "escalationModel": "opus"
}
```

## Related

- [Feature flags](./experimental-features#review): the flags on this page.
- [Headless mode](./headless): `--verification` in scripts and CI.
- [Learning](./learning): a failed verification counts as a correction for the lesson offered after a run.
