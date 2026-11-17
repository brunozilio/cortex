---
title: Branches and git
description: Branch a conversation from an earlier prompt, let {{NAME}} set up git, isolate work in worktrees, follow a pull request to merge, and run several sessions in one folder.
---

{{NAME}} works best in a git repository: changes can be reviewed with `/diff`, undone, and isolated in worktrees. This page covers the conversation branches you can make from the chat, and the git features around them.

## Branch from an earlier prompt

> [!NOTE]
> Controlled by the `branches` flag. Default: on.

`/branch <n>` starts a **new conversation** that contains everything before your n-th prompt of this one, and leaves this conversation as it is. Prompts are numbered from 1, oldest first, in the order you sent them.

```text
/branch 2
Branched: this is a new conversation from before that prompt; the files are as they were. The original stays in /resume.
```

After branching:

- the chat shows the new conversation, cut just before the chosen prompt;
- that prompt is put back in the input box, so you can edit it and send a different version;
- the original conversation stays whole and can be reopened with `/resume`.

Branching never changes files on disk. To also put the files back as they were at that prompt, use `/rewind` instead (see [Sessions](./sessions)).

> [!NOTE]
> `/branch` without a number lists the prompts you can branch from, numbered oldest first (`Nothing to branch from in this chat yet.` when there is none). To continue in a copy of the whole conversation, use `/fork`.

Branching is refused while the agent is working (`Wait for the agent to finish, or press Esc, before branching.`), for a number that matches no prompt (`There is no such prompt; /branch lists them.`), and when the conversation was compacted after that prompt (`the conversation was compacted since then, so it cannot branch there`).

## Initialize git in a new folder

> [!NOTE]
> Controlled by the `git-offer` flag. Default: on.

When you send the first message of a new conversation in a folder that is not inside a git repository, {{NAME}} holds the message and asks:

| Answer | What happens |
|---|---|
| **Yes, initialize git** | Runs `git init` and makes an empty first commit (`Initial commit`), then sends your message |
| **No, not now** | Sends your message without a repository; you may be asked again the next time you start {{NAME}} here |
| **No, and don't offer again** | Turns the `git-offer` flag off in your settings, then sends your message |

If git cannot make the first commit (usually because `user.name` and `user.email` are not set), the repository is still created and {{NAME}} warns that agents run without worktree isolation until there is a commit. `/features git-offer on` brings the offer back.

## Worktrees

A git worktree is a separate checkout of the same repository. {{NAME}} creates its worktrees under `{{DIR}}/worktrees/<id>` at the repository root, detached at the current commit (`HEAD`), so it holds the commits of the branch you are on. Uncommitted changes, staged or not, and untracked files are not carried over: commit them first if the worktree needs them. EnterWorktree's result names the commit (short hash and subject) and says how many uncommitted changes stayed behind.

Worktrees need a repository with at least one commit. Without one, work runs in the project folder and the result says `Ran without isolation: this folder is not a git repository with a commit, so no worktree was made.`

### A sub-agent in its own worktree

When the agent starts a sub-agent that edits files, it can ask for `isolation: "worktree"`. The sub-agent then works in a fresh worktree. When it finishes, a worktree with no commits and no uncommitted files is removed; one where the sub-agent changed something is kept, and its path is reported so the changes can be reviewed and merged. See [Agents](./agents).

### Move the whole session into a worktree

Ask the agent to work in a worktree ("do this in a separate worktree"). It uses two tools:

- **EnterWorktree** creates a worktree and moves the session there: from the next step on, files and commands are in the worktree, and so are later prompts of the session, until it leaves.
- **ExitWorktree** goes back to the project's checkout, with `action: "keep"` to leave the worktree on disk (its path is reported) or `action: "remove"` to delete it. Removing is refused while the worktree has uncommitted files or commits of its own, unless `discard_changes` is true.

Only the main agent can move the session, and only one worktree at a time. A resumed session goes back into its worktree; if the worktree was deleted, resuming fails with `The session worktree no longer exists; restore it before resuming`.

## Follow a pull request to merge

> [!EXPERIMENTAL]
> Controlled by the `experimental-pr-follow` flag. Default: on. With the flag off, the `follow-pr` skill is hidden from the agent.

The built-in `follow-pr` skill keeps a pull request moving until it is merged or closed. Ask for it after opening a PR:

```text
follow the PR until it's merged
```

It needs the GitHub CLI (`gh`) signed in; if `gh auth status` fails, the agent tells you and stops. Then it:

1. finds the PR you named, or the current branch's PR;
2. starts one **Monitor** that checks the PR every 60 seconds and prints a line only when its state, merge status, failing checks, reviews or comments change, so each change wakes the agent;
3. on a failing check, reads its log (`gh run view <id> --log-failed`), fixes the cause, runs the same check locally, commits and pushes;
4. on a new review or comment, changes the code when it asks for a change, or answers a question with `gh pr comment`; it never resolves a reviewer's thread;
5. when the PR is merged or closed, stops the monitor and tells you.

The agent asks you before anything it cannot undo, such as a force push, and never merges the PR unless you asked it to.

## Several sessions in one folder

> [!EXPERIMENTAL]
> Controlled by the `experimental-session-claims` flag. Default: on.

You can run several {{NAME}} sessions in the same project. Each time the agent edits or writes a file, its session records a claim on that file. If another session that is still running changed the same file less than 10 minutes earlier, the tool result warns the agent:

```text
<other-session>Another session (process 48121) changed this file 3 min ago. Its work and yours may overwrite each other: re-read the file before editing it again, and tell the user if you both need it.</other-session>
```

A session's own edits never count as a clash. Claims of sessions that ended, or older than an hour, are dropped. The claims are kept in your system's temporary folder, in `{{COMMAND}}-claims/`, one file per project.

> [!TIP]
> For two sessions that should not touch each other's files at all, have one of them work in a worktree.

## Related

- [Sessions](./sessions): `/resume`, `/fork`, `/rewind`.
- [Verification and review](./verification-and-review): checks that run before the agent finishes.
- [Skills](./skills): how built-in skills such as `follow-pr` are loaded.
