---
description: Write the story's spec with its Gherkin scenarios, publish it to Confluence and link it in Jira
argument-hint: <ISSUE>
---

Load the `dev-flow:flow` skill and work on the `spec` stage of the story $ARGUMENTS.

Read `.flow/config.json` and the story's state file first. If the state says an earlier stage, finish that one and ask to move forward; never skip a stage. If it says a later one, say so and ask before going back. With no issue given, list the open flows and ask which one.

Stop when the spec is published and ask to enter `plan`; that approval is the spec's approval.
