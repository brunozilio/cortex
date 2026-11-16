---
description: Run the development flow for a story from wherever it is to done, stopping for approval at every stage
argument-hint: <ISSUE | idea for a new story>
---

Load the `dev-flow:flow` skill and run the flow for: $ARGUMENTS

- An issue key: go on from the stage in `.flow/<ISSUE>.json`, or start at
  `refine` when there is no state yet.
- Anything else is an idea for a new story: draft it, create it in Jira
  after showing the draft, then start at `refine`.
- Nothing: list the open flows in `.flow/` and ask which one.

Carry the story stage after stage. At the end of each stage, say what was
done and what is about to be approved, then write the state file with the
next stage; go on only once that write is allowed. Stop when the story is
`done`, when a gate is denied, or when something only a person can give
is missing (a sign-in, a token, a deploy to QA).
