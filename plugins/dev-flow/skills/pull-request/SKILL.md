---
name: pull-request
description: How the development flow uses GitHub through the gh CLI - branch names carrying the Jira key, the pull request template, watching checks, answering reviews, merging in dependency order across several repositories. Use in the pr, review and merge stages, or when opening or merging a pull request for a story.
---

# Pull requests in the flow

GitHub is reached with `gh`. Run `gh auth status` first; if it fails,
tell the person and stop. In a folder that holds several repositories,
run each command inside the repository (`git -C <path>`, or `cd <path> &&
gh ...`).

The gate refuses `gh pr create` before the flow reached `pr`, and
`gh pr merge` before `merge`.

## Branch

`config.json` `github.branch` is the pattern, default
`feature/<ISSUE>-<slug>` (`feature/PAY-123-refund-captured-payment`),
cut from `github.base` (default `main`) after `git fetch`. The issue key
in the name is how the gate and Jira find the story: keep it.

## Opening

Before pushing: the repository's full local verification passes
(`dev-flow:stack`), the diff holds nothing unrelated to the story
(`git diff <base>...HEAD --stat`), no secret or generated file went in.

```sh
git push -u origin HEAD
gh pr create --base main --title "PAY-123: Refund a captured payment" --body-file .flow/PAY-123/pr-payments-api.md
```

The body, in the language `config.json` names. When the repository has
its own `.github/pull_request_template.md`, fill that one and add what is
missing from this:

```markdown
## What and why
<two or three sentences; link the Jira issue and the Confluence spec>

## Changes
- <by behaviour, not by file>

## How it was tested
- <commands run and their results; Zephyr cases: PAY-T45, PAY-T46>

## Rollout and rollback
- <migrations, flags, Terraform plan summary, how to undo>

## Checklist
- [ ] Acceptance criteria of PAY-123 covered by tests
- [ ] No secrets, no unrelated changes
- [ ] Docs and spec updated
```

Keep a PR reviewable: when one repository's change passes about 400
changed lines, split it along the plan's tasks and say the order.

## Checks

`gh pr checks <n> --watch` waits for them; the built-in `follow-pr` skill
does it without holding the turn. On a failure, read the log
(`gh run view <id> --log-failed`), fix the cause, run the same check
locally, push. Never rerun a failing check hoping it passes, and never
disable one.

## Review

- `gh pr view <n> --comments` and `gh api repos/{owner}/{repo}/pulls/<n>/comments`
  for inline comments.
- A requested change: change the code, push, reply saying what changed.
  A question: answer it. Disagreement: say why once, then follow the
  reviewer or ask the person.
- Never resolve a reviewer's thread, dismiss a review or approve your
  own PR.
- `gh pr view <n> --json reviewDecision,mergeStateStatus` says when it
  is ready.

## Merging

Only in the `merge` stage. Method from `github.merge` (`squash` default):

```sh
gh pr merge <n> --squash --delete-branch
gh pr view <n> --json state,mergeCommit
```

Several repositories: merge what others depend on first (Terraform and
shared libraries, then services, then the portal), and wait for each
one's main-branch pipeline before the next when the next needs its
artifact. A PR that cannot merge (conflicts, a failing check, a missing
approval) is reported, never forced: no `--admin`, no force push to the
base branch.
