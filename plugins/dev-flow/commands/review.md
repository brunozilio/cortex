---
description: Review the story's pull requests with the reviewer agents, fix the findings and answer the people's reviews
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `review` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when no Critical or Required finding is open and the required approvals are given, and ask to enter `merge`.
