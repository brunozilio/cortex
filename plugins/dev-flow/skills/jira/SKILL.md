---
name: jira
description: How the development flow reads and writes Jira through the Atlassian MCP server - find the site, read an issue, create a story from the team's template, update acceptance criteria, move an issue by status name, comment with links. Use whenever a flow stage touches a Jira issue.
---

# Jira in the flow

Jira is reached through the plugin's Atlassian MCP server. Its tools are
named `mcp__plugin_dev-flow_atlassian__<tool>`; when their schemas are not
loaded yet, load them with ToolSearch (`+atlassian jira`). Read the
schema before the first call: the server's parameters win over this file.

## Before the first call

1. `getAccessibleAtlassianResources` gives the `cloudId` every other
   call takes. Pick the site `.flow/config.json` names (`jira.site`).
2. A sign-in error means the person must run
   `/mcp login plugin_dev-flow_atlassian`. Say so and wait.

## Reading

- One issue: `getJiraIssue` with the key. Read the description,
  acceptance criteria, links, sub-tasks and comments before refining.
- Several: `searchJiraIssuesUsingJql`, for example
  `project = PAY AND sprint in openSprints() AND status = "To Do"`.
- Its Confluence pages and links: `getJiraIssueRemoteIssueLinks`.

## Creating a story

Creating an issue always asks the person (the flow's gate). Show the
draft first, then `createJiraIssue` with the project and issue type from
`config.json` (`jira.project`, `jira.storyType`, default `Story`).

```
Summary: <verb + object, under 80 characters>

As a <role>, I want <capability>, so that <benefit>.

Context
<why now, what exists today, links>

Acceptance criteria
1. Given <state>, when <action>, then <result>.
2. ...

Out of scope
- ...
```

Write it in the language `config.json` names. One story is one
deliverable change; split one that needs "and" in its summary.

## Updating

- Description and fields: `editJiraIssue`. Send the whole new
  description; do not drop text other people wrote, add to it.
- Comments: `addCommentToJiraIssue`. Use one comment per stage event
  (spec published, PRs opened, merged, QA result), with links, not prose.

## Moving an issue

Statuses differ per team, so the flow maps its stages to status names in
`config.json` (`jira.statuses`: `refine`, `implement`, `review`, `qa`,
`done`). To move:

1. `getTransitionsForJiraIssue` lists what the issue can do now.
2. Pick the transition whose target status is the configured name.
3. `transitionJiraIssue` with that transition's id.

A stage with no status configured moves nothing. When no transition
leads to the configured status, list the ones offered and ask. The gate
refuses a move before the flow reached `implement`, and moving to the
`done` status belongs only to the `done` stage.

## Links

- Pull requests: comment the URL; the GitHub integration, when the team
  has it, links by the issue key in the branch and the PR title.
- Zephyr test cases are linked from the Zephyr side (`dev-flow:zephyr`).
- The spec page: add it to the description under "Spec".
