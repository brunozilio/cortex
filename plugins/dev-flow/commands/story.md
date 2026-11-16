---
description: Draft a Jira story from an idea and create it after approval
argument-hint: <idea>
---

Load the `dev-flow:flow` and `dev-flow:jira` skills and write a story for: $ARGUMENTS

Follow the flow skill's "Story" section: ask what is unclear
(`dev-flow:interview-me` when the idea is vague), draft the story in the
team's template and language, and show the draft. Create the issue in
Jira only after showing it; creating asks for approval. Then say the new
key and offer to start the flow at `refine` with `/dev-flow:start <ISSUE>`.
