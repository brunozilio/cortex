---
description: Refine a Jira story until it is ready to specify: questions answered, testable acceptance criteria, repositories named
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `refine` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop at the stage's exit list and ask to enter `spec`.
