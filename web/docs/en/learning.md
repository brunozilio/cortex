---
title: Learning
description: Turn your corrections into lasting lessons with /learn, keep decisions with what was ruled out, and check a branch against what the project remembers with /check.
---

When you correct the agent, the correction usually matters beyond the current task. {{NAME}} can notice it, propose one lesson to keep for every future session of the project, and later check new changes against what was learned.

## Lessons from your corrections

> [!EXPERIMENTAL]
> Controlled by the `experimental-learning` flag. Default: on.

During a conversation {{NAME}} notes each time you:

- refuse a tool call and say what to do instead;
- send a message while the agent is working, redirecting it;
- see the agent stopped for repeating the same call;
- get a failed verdict from the [verification judge](./verification-and-review#verify-a-run-against-criteria).

After a run, once at least two such signals have piled up, a side call reads the conversation and proposes **one** lesson, or none if nothing lasting was learned:

```text
Lesson from this run: remember "This project uses pnpm, never npm"
Why: you refused npm install and said to use pnpm, twice.
/learn keep saves it, /learn skip drops it.
```

The signals are then cleared, so the next lesson needs new corrections.

| Lesson | Proposed when | `/learn keep` does |
|---|---|---|
| Memory | A fact or way of working to remember | Saves it as a procedural [memory](./memory) |
| Skill | A repeatable procedure worth writing down | Asks the agent to create or update the skill with the `skill-creator` skill |
| Rule | You keep allowing or refusing the same command | Saves the `allow` or `deny` rule in the project's `{{DIR}}/settings.local.json` |
| Instruction | You stated a rule for how all work in the project is done | Adds a line under `## Learned` in the project's instructions file |

`/learn skip` drops the lesson. With no lesson waiting, `/learn` answers `No lesson is waiting: one is offered after a run where you corrected the agent.`

The proposal must rest on what you said or did; the model is told never to invent a preference, and never to put keys, tokens or passwords in a lesson.

### Instruction lines

An instruction lesson is appended to `{{INSTRUCTIONS}}` at the project root (or `{{DIR}}/{{INSTRUCTIONS}}` when only that one exists), under a `## Learned` heading that is created when missing:

```markdown
## Learned
- Run cargo nextest, never cargo test
```

A line already there is not added twice. A line that looks like it holds a secret is refused: `The instruction holds what looks like a secret, so it was not written.`

> [!NOTE]
> Saving a rule or an instruction goes through the `suggested-rules` feature. With that flag off, `/learn keep` cannot save those two kinds.

## Structured decisions

> [!EXPERIMENTAL]
> Controlled by the `experimental-structured-learnings` flag. Default: on.

A decision is worth more with what was tried and dropped: that is what saves the next session from trying it again. With this flag on, the agent can save a decision as three parts instead of one sentence:

```json
{
  "remember": [{
    "decided": "parse dates with chrono, not time",
    "ruled_out": ["time: no strftime-compatible formats"],
    "watch_out": ["chrono's Local is not available on wasm"]
  }]
}
```

It is kept as one memory that reads:

```text
Decided: parse dates with chrono, not time. Ruled out: time: no strftime-compatible formats. Watch out: chrono's Local is not available on wasm.
```

When such a memory is shared with [`/memory share`](./memory#team-memory), the team file is written with `## Decided`, `## Tried and ruled out` and `## Watch out` sections, and with your git `user.name` and today's date in its front matter. Team files written that way are read back as decisions.

## Check a branch against memory

> [!EXPERIMENTAL]
> Controlled by the `experimental-learning-check` flag. Default: on.

Before committing or opening a pull request, `/check` has the agent compare your changes with what the project's memory says about the code they touch:

```text
/check
```

{{NAME}} finds the changes since the branch left the default branch (`origin/HEAD`, else `main`, else `master`; uncommitted changes against `HEAD` when you are on it), including new untracked files. It picks up to 12 memories, yours and the team's: first those tied to a changed file, then those whose words best match the changed files and lines. It then sends the agent a prompt asking it to read `git diff`, change nothing, and report each conflict as:

```text
✗ src/retry.rs:18
  sets the retry count to 10
  [4] Keep retries at 4: the upstream rate limit resets every 5 attempts
· 1 conflicts · fix them, or say why in the commit message
```

or `✓ No conflicts with <n> memories`. A change that agrees with a memory, or that no memory speaks to, is not a conflict.

`/check` needs a git repository (`/check needs a git repository.`). With no change or no relevant memory it answers `Nothing to check: no change, or no memory is about the changed files.` With the flag off it answers `/check is turned off (experimental-learning-check); /features turns it on.`

## Related

- [Memory](./memory): where lessons and decisions are kept.
- [Feature flags](./experimental-features#learning-and-memory): the flags on this page.
- [Instructions files](./instructions-files): the file instruction lessons are added to.
