---
description: Show where each story of the development flow is and what its next approval needs
argument-hint: [ISSUE]
---

Read `.flow/*.json` (only $ARGUMENTS when an issue is given) and change
nothing.

For each story not yet `done`, show: the issue and title, the stage, what
of that stage's exit list in the `dev-flow:flow` skill is already met and
what is missing, and its links (spec page, pull requests, test cycle).
Check the live facts you can read cheaply instead of trusting the file:
the pull requests' state and checks (`gh pr view --json
state,reviewDecision,statusCheckRollup`) and the Jira status. Say when
the file and the facts disagree. End with the one next step per story.
