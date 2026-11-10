---
name: pr
description: "Open a PR for the current branch: run the PR gate (tests, clippy, lint), have a sub-agent code-review the branch, then create the PR. `/pr [low|medium|high]` picks the review level (medium by default). Also use when writing a PR body."
---

## Opening a PR (`/pr`)

Stop at the first step that fails and say why; fix only what the user asks.

1. **Branch.** Refuse on `main`. Find the base (`origin/HEAD`, else
   `main`) and check the branch has commits ahead of it. If the tree has
   uncommitted changes, ask whether to commit them first.
2. **Gate.** Run `.superfik/hooks/pr-gate.sh`; it prints one line per check
   (tests, doctests, clippy, lint). On failure, show the failing check and its
   output. The `pre-pr.sh` hook runs the same gate again before
   `gh pr create`; that run is instant when the tree hasn't changed.
3. **Review.** Start one sub-agent (Agent tool) to run the `code-review`
   skill at the level from the arguments (`medium` when none) over the
   branch's diff against the base. Tell it the base and the .superfik/SUPERFIK.md
   `CARGO_TARGET_DIR` rule. It reports findings and changes nothing.
   - Correctness findings: list them and ask whether to fix them first or
     open the PR anyway.
   - Only cleanup findings: carry on and list them in the PR body.
4. **Push** with `git push -u origin HEAD`.
5. **Create** with `gh pr create --base <base>` and a body from the template
   below. Under **Evidence**, give each gate check's result and the review's
   level and findings (or "no findings").
6. Reply with the PR's URL.

## The PR body

Use this template for writing the PR body:

```markdown
## Summary

<diagram, diff-sketch, or tree>

## Evidence

- **Before:** <screenshot/output/failing test run>
  **After:** <screenshot/output/passing test run>

## Merge Danger

**Door:** <one-way or two-way>

<optional: description>

**Blast Radius:** <one-word description>

<optional: potential ramifications of merge>
```

## Sections

Skip all preambles and keep prose brief. Use the user's domain language from `GLOSSARY.md`.

### Summary

Pick the smallest view that makes the key point clear.

- Show logic or an algorithm as pseudocode:

```text
on(save)
  if content is unchanged
    return cached result
  write new content
  return fresh result
```

- Show runtime control flow as a call tree:

```text
submitForm
  createSession
    persistPrompt
    launchAgent
  navigateToSession
```

- Show UI structure as a component tree, including state and module boundaries that matter:

```text
<SessionPage> (apps/example/src/routes/session.tsx)
  useSessionEvents()
  <SessionToolbar>
    <RunSkillButton> (packages/ui)
```

- Show file responsibility or a broad refactor as a shallow file tree:

```text
src/
├── commands/       # parses user actions
├── sessions/       # owns session state
└── transport/      # sends API requests
```

- Show component interaction, control flow, or data flow with Mermaid:

```mermaid
sequenceDiagram
    participant User
    participant UI
    participant Daemon
    User->>UI: choose command
    UI->>Daemon: send expanded prompt
    Daemon-->>UI: stream result
```

- Use `diff` when the point is what changes and the surrounding shape already exists. Match the diff shape to the topic.

For a component change:

```diff
 <SessionPage>
   useSessionEvents()
   <SessionToolbar>
+    <RunSkillButton />
   <SessionTimeline>
+    <SkillResultCard />
```

For a file-layout change:

```diff
 src/
 ├── commands/
+│   └── show-me.ts       # expands the slash command
 ├── sessions/
-└── transport.ts
+└── transport/
+    ├── client.ts
+    └── stream.ts
```

For a call-tree or call-stack change:

```diff
 submitForm
   createSession
     persistPrompt
+    expandSkillMention
     launchAgent
-  navigateToSession
+  navigateToSession
+    subscribeToEvents
```

For a state or control-flow change:

```diff
 on(save)
-  write content
+  if content is unchanged
+    return cached result
+  write new content
+  invalidate cache
```

- Show the whole block when most of it is new, when omitted context would hide ownership or order, or when the user needs a copyable target shape:

```ts
function expandSkill(command: string): string {
  const skillName = command.slice(1);
  return `use the ${skillName} skill`;
}
```

#### Guidance

Place each visual next to the short text it supports. Keep only the calls, files, props, states, and boundaries needed to answer the user's current question or the options to resolve the current discussion point.

You may use one of these, you may use several, it is unlikely you will use all of them. Use your judgement and don't overwhelm the user.

### Evidence

Concrete evidence that the change works. Show a before and after.

Screenshots are S-tier - when the environment is set up for it and the change is visual.

Execution-based evidence is A-tier. Test results, console output. Show the exact test that now fails and passes, using pseudocode.

### Merge Danger

Describe whether it's a one-way or two-way door. You can walk back through two-way doors, but not one-way doors. A PR that is cheap to roll back is lower risk. Changes that involve destructive actions or hard-to-reverse decisions are one-way doors.

The blast radius is the potential impact or scope of the changes introduced by this PR. Consider all possibilities. Examples are layout shift, breakages for consumers, mobile responsiveness, etc.
