---
description: Open the story's pull requests, link them in Jira and carry the checks to green
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `pr` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when every pull request is open with green checks and ask to enter `review`.
