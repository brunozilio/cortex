---
description: Implement the story's plan test-first, task by task, in each repository, with its automated scenarios
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `implement` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when every task is done and the full local verification is green, show the evidence, and ask to enter `pr`.
