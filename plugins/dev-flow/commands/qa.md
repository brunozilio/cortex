---
description: Run the story's Cucumber and Cypress scenarios in the QA environment and record the executions in Zephyr
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `qa` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when every scenario passed and is recorded in the cycle, and ask to enter `done`. A failure goes back to `implement`.
