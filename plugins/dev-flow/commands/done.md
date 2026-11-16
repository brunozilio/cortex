---
description: Close the story: move the Jira issue to done, update the Confluence spec and record lasting decisions
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `done` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

The state file stays as the record.
