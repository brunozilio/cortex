---
description: Merge the story's approved pull requests in dependency order
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `merge` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Entering this stage is the permission to merge. When every pull request is merged, ask to enter `qa`.
