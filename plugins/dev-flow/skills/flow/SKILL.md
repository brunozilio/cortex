---
name: flow
description: Runs the team's development flow for one Jira story - refine, spec, plan, implement, pull request, review, merge, QA, done - with an approval gate between stages. Use when the user starts, continues or asks the status of a story or issue key (PAY-123), says "start the flow", "next stage", "refine this story", "open the PR for the story", "run QA" or "close the story", or when a session opens with flows in `.flow/`.
---

# Development flow

One Jira story travels through nine stages. You do the work of a stage,
show the evidence, and ask to enter the next one. A person approves every
move; nothing outside the stage you are in is yours to do.

```
(story) -> refine -> spec -> plan -> implement -> pr -> review -> merge -> qa -> done
```

Tools: Jira and Confluence through the plugin's Atlassian MCP server
(skills `dev-flow:jira`, `dev-flow:confluence`), GitHub through `gh`
(`dev-flow:pull-request`), Zephyr Scale through its REST API
(`dev-flow:zephyr`). Sonar, Veracode and Datadog are not part of this flow
yet: do not call them or wait on them.

## State

Everything the flow knows lives in `.flow/` in the folder the session
started in (the folder that holds the repositories, or the repository).

- `.flow/config.json`: the team's settings (Jira project, status names,
  Confluence space, Zephyr project, base branch, repositories and their
  stacks). If it is missing, run `/dev-flow:setup` first.
- `.flow/<ISSUE>.json`: one story's state. `.flow/<ISSUE>/`: its working
  files (`spec.md`, `plan.md`, `qa.md`).

```json
{
  "issue": "PAY-123",
  "title": "Refund a captured payment",
  "stage": "spec",
  "approval": "Refinement done: 4 acceptance criteria, 2 repositories, 5 points",
  "repos": [
    { "path": "payments-api", "stack": "java", "branch": "feature/PAY-123-refund", "pr": "" }
  ],
  "spec": { "file": ".flow/PAY-123/spec.md", "confluence": "" },
  "tests": ["PAY-T45"],
  "cycle": "",
  "history": [{ "stage": "refine", "date": "2026-11-16" }]
}
```

`stage` is the stage being worked on now. `approval` is one sentence
saying what the person approves by letting the file change: they see it
in the permission prompt, so make it concrete (what was produced, the
numbers, the link).

## The gate

Entering a stage is one action: **Write the whole state file with the new
`stage`** (never Edit it, never through Bash). The plugin's `gate.js` hook
turns that write into an approval prompt. It refuses a skipped stage, and
it refuses the outside actions of a stage that was not approved: opening
a PR before `pr`, merging before `merge`, moving the Jira issue before
`implement`, writing to Zephyr before `plan`. `terraform apply` and
creating a Jira issue always ask.

Rules that follow:

1. Finish the stage's exit list before you write the next stage. Say in
   the conversation what was done and what the person is about to
   approve, then write the file.
2. A denied write means "not yet": read the person's note, do what it
   asks, and offer the move again. Never work on the next stage while the
   file still says the previous one.
3. A refusal from the gate names the stage that is missing. Go back to
   it; do not look for another way to run the command.
4. Going back (QA found a bug, review asked for a redesign) is a write
   with the earlier stage; it asks too. Add why to `approval`.
5. Updates inside a stage (a PR link, test keys) are ordinary writes with
   the same `stage`.

## Starting and resuming

- **An issue key**: read `.flow/<ISSUE>.json`. If it exists, go on from
  its stage. If not, read the issue in Jira and start at `refine` (or at
  the stage the issue is really in, saying why in `approval`).
- **An idea with no issue yet**: draft the story (below), create it in
  Jira, then start at `refine`.
- **No argument**: list the open flows in `.flow/` with their stages and
  ask which one.

Write in the language `config.json` names (`language`) everything people
read: Jira text, Confluence pages, PR bodies, test scenarios.

## Story (before the flow starts)

Draft, do not create yet: a title; "As a ... I want ... so that ...";
context and why now; acceptance criteria as Given/When/Then; what is out
of scope. If the idea is vague, load `dev-flow:interview-me` and ask until
it is not. Show the draft, then create the issue with `dev-flow:jira`
(creating it asks for approval).

## Stages

Each stage lists what to do and the exit list. When a stage names a
skill, load it and follow it; this file only says how it plugs in.

### refine

Make the story ready to specify.

- Read the issue, its links, comments and any Confluence page it cites.
- Find the repositories it touches (`config.json` `repos`) and read
  enough code to judge the change. Record them in `repos` with stack.
- Ask the person every open question (`dev-flow:interview-me`); do not
  guess business rules.
- Rewrite the acceptance criteria so each one is testable, and name the
  risks, dependencies and anything out of scope.
- Update the issue in Jira (description, acceptance criteria, estimate if
  the team estimates) and move it to the configured `refine` status when
  one is set.

Exit: no open question, testable acceptance criteria in Jira, the
repositories named.

### spec

Write what will be built. Load `dev-flow:spec-driven-development`.

- Write `.flow/<ISSUE>/spec.md`. In an existing system this is a delta
  spec: the change only. Cover each repository: behaviour, contracts
  (`dev-flow:api-and-interface-design` for a new or changed API), data,
  infrastructure (Terraform resources, IAM, what changes in AWS),
  rollout and rollback.
- Write the test scenarios in Gherkin, each tied to an acceptance
  criterion and marked `api` (Cucumber) or `web` (Cypress).
- Publish the spec to Confluence and link it in the issue
  (`dev-flow:confluence`). Record the page in `spec.confluence`.

Exit: the spec covers every acceptance criterion, each has at least one
scenario, the page is published and linked. Approving the next stage is
approving the spec.

### plan

Break the spec into tasks. Load `dev-flow:planning-and-task-breakdown`.

- Write `.flow/<ISSUE>/plan.md` (that skill's plan and task list go
  there, not in `tasks/`): small vertical tasks per repository, in
  dependency order, each with its verification command.
- Register the scenarios as Zephyr test cases linked to the issue
  (`dev-flow:zephyr`) and record their keys in `tests`, so the automated
  tests can carry them.
- If the team tracks sub-tasks in Jira, create them from the plan.

Exit: an ordered plan, every scenario with a Zephyr key.

### implement

Build it, one task at a time. Load `dev-flow:incremental-implementation`
and `dev-flow:test-driven-development`, plus `dev-flow:stack` for the
repository's language.

- In each repository: branch from the base as `config.json` says
  (default `feature/<ISSUE>-<slug>`); record it in `repos`.
- Move the issue to the `implement` status.
- For each task: failing test, code, passing test, commit
  (`<ISSUE>: what changed`). Follow `dev-flow:git-workflow-and-versioning`
  for commits and `dev-flow:source-driven-development` when using a
  framework feature you have not verified. Stuck on a failure: load
  `dev-flow:debugging-and-error-recovery`.
- Automate the scenarios as you go (`dev-flow:test-automation`): Cucumber features for
  the API, Cypress specs for the Angular portal, each tagged with its
  Zephyr key.
- Infrastructure: `terraform fmt`, `validate` and `plan` only. Applying
  is the pipeline's job or the person's.

Exit: every task done, the repository's full local verification green
(paste the commands and their results), `references/definition-of-done.md`
met.

### pr

Open the pull request. Load `dev-flow:pull-request`.

- Push each branch and open its PR with the template; link the issue and
  the spec. Record the URL in `repos[].pr`.
- Watch the checks until they finish (the built-in `follow-pr` skill, or
  `gh pr checks --watch`) and fix what fails.
- Move the issue to the `review` status and comment the PR links.

Exit: every PR open, checks green.

### review

Get the change reviewed and fix what the review finds.

- Start the `dev-flow:code-reviewer` and `dev-flow:test-engineer` agents
  in parallel on each PR's diff (`dev-flow:code-review-and-quality` is
  the standard). Fix every Critical and Required finding; answer the rest
  in the PR.
- Read the people's reviews (`gh pr view --comments`); change the code or
  answer. Never resolve a reviewer's thread for them.
- Wait for the approvals branch protection requires
  (`gh pr view --json reviewDecision`).

Exit: no open Critical or Required finding, required approvals given,
checks green after the last push.

### merge

Entering this stage is the permission to merge.

- Merge each PR with the configured method (`gh pr merge --squash`
  by default), in dependency order when several repositories change:
  infrastructure and providers before their consumers.
- Confirm each one merged (`gh pr view --json state,mergeCommit`) and
  comment the merge commits on the issue.
- Never force a merge past failing checks or missing approvals; report
  it and stay in this stage.

Exit: every PR merged.

### qa

Test the merged change in the QA environment. Load `dev-flow:test-automation`.

- Wait for the deploy to the QA environment the team's pipeline does;
  ask the person when you cannot see it.
- Move the issue to the `qa` status.
- Create a Zephyr test cycle for the issue, run the story's Cucumber and
  Cypress scenarios against QA, and record each execution with its
  result (`dev-flow:zephyr`). Record the cycle in `cycle`.
- Write `.flow/<ISSUE>/qa.md`: what ran, where, the results, evidence.
- A failure is a bug: describe it on the issue and go back to
  `implement` (rule 4).

Exit: every scenario of the story passed in QA and is recorded in the
cycle.

### done

Close the story.

- Move the issue to the `done` status with a closing comment: PRs, spec
  page, test cycle.
- Update the Confluence spec with what changed during the build, and
  record any lasting decision as an ADR (`dev-flow:documentation-and-adrs`).
- Leave `.flow/<ISSUE>.json` as the record.

## When something is missing

- The Atlassian server asks to sign in: tell the person to run
  `/mcp login plugin_dev-flow_atlassian`, and wait.
- `gh auth status` fails, or the Zephyr token variable is empty: say
  which and stop the stage there.
- A status name in `config.json` has no matching Jira transition: list
  the transitions the issue offers and ask which one to use; then fix
  `config.json`.
