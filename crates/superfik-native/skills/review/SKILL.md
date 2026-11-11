---
name: review
description: Review the current changes (or a pull request, branch or path given after it) for bugs, at medium effort
---

Review the changes named in the arguments: a pull request number, a branch, a path, or, with no arguments, the working tree's uncommitted changes against `HEAD` (and the branch's commits against its base when the tree is clean).

Load the `code-review` skill with the Skill tool and follow it at the `medium` level, passing on these arguments: $ARGUMENTS

Report what you find as that skill says. Do not change any file unless the user asked for fixes.
