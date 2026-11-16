---
description: Break the approved spec into ordered tasks per repository and register the scenarios in Zephyr
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `plan` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when the plan is written and every scenario has a Zephyr key, and ask to enter `implement`.
