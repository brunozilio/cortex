---
name: follow-pr
description: Follow a pull request until it is merged or closed - wake on each failing CI check and each new review comment, fix or answer it, push, and keep watching. Use after opening a PR when the user wants it carried to merge, or asks to "watch", "babysit" or "follow" a PR.
---

# Follow a pull request to merge

You keep a PR moving without the user polling it. It needs the GitHub CLI
(`gh`) signed in; if `gh auth status` fails, tell the user and stop.

## 1. Find the PR

Use the number the user gave, or the current branch's PR:
`gh pr view --json number,url,state,headRefName`. If there is none, say so
and stop.

## 2. Watch it with Monitor

Start one `Monitor` with a loop that prints a line only when something
changes, so each line wakes you:

```sh
pr=<number>; last=""
while true; do
  now=$(gh pr view "$pr" --json state,mergeStateStatus,statusCheckRollup,reviews,comments \
    --jq '[.state, .mergeStateStatus,
           ([.statusCheckRollup[]? | select(.conclusion=="FAILURE") | .name] | join(",")),
           (.reviews | length), (.comments | length)] | @tsv')
  if [ "$now" != "$last" ]; then echo "$now"; last="$now"; fi
  case "$now" in MERGED*|CLOSED*) exit 0;; esac
  sleep 60
done
```

Then end your turn and wait; do not poll with Bash yourself.

## 3. When a line arrives

- **A failing check**: read its log (`gh run view <id> --log-failed`),
  fix the cause in the code, run the same check locally, commit and push.
- **A new review or comment**: read it (`gh pr view <pr> --comments`).
  Change the code when it asks for a change; answer a question in a reply
  (`gh pr comment`). Never resolve a reviewer's thread for them.
- **MERGED or CLOSED**: stop the monitor with KillShell and tell the user.

Ask the user before anything you cannot undo, such as a force push, and
never merge the PR yourself unless they asked you to.
