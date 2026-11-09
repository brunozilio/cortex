---
name: debug
description: Help diagnose an issue with Superfik itself, from its logs and settings
disable-model-invocation: true
---

# Debug Skill

Help the user debug an issue they're encountering with Superfik: the app,
this session's agent, or its tools.

## Logs

Superfik writes its diagnostics to a log only on Windows; elsewhere they go
to standard error. Find the ones for the user's system:

* Windows - `%LOCALAPPDATA%\Superfik\logs\superfik-<date>-<pid>.log`, one
  file per launch (the newest is the current one).
* macOS - standard error of the app's process: nothing is kept when
  Superfik is opened from the Dock or Finder. To capture a run, quit it and
  start the binary from a terminal, e.g.
  `/Applications/Superfik.app/Contents/MacOS/superfik 2> ~/superfik.log`.
* Linux - standard error: the terminal it was started from, or the user
  journal when the desktop started it (`journalctl --user -b | grep -i superfik`).

The default level is `superfik=info,warn`. For more detail, start Superfik
with `RUST_LOG=superfik=debug` (or `superfik=trace`, which also logs every
frame's timing) in its environment, then reproduce the issue.

If no log covers the moment the issue happened, tell the user where the log
will be, how to raise the level if useful, and ask them to reproduce the
issue; then read it again.

For context, grep for `ERROR` and `WARN` lines across the whole file.

## Data

* App data (sessions, settings kept by the server, memory, notes, plugins):
  `~/Library/Application Support/Superfik` on macOS, `%APPDATA%\Superfik` on
  Windows, `$XDG_CONFIG_HOME/superfik` (or `~/.config/superfik`) on Linux.
* Local data (models, worktrees, browser profiles, the agent's sessions in
  `native-sessions`): the same folder, except `%LOCALAPPDATA%\Superfik` on
  Windows.

## Settings

Remember that the agent's settings are in:
* user - `~/.superfik/settings.json`
* project - `.superfik/settings.json`
* local - `.superfik/settings.local.json`

Skills are in `.superfik/skills`, agents in `.superfik/agents`, rules in
`.superfik/rules` (in the project, or under `~/.superfik` for the user).
Instructions come from `SUPERFIK.md` first, then `CLAUDE.md` and `AGENTS.md`.

## Instructions

1. Review the user's issue description; if they gave none, read the log and
   summarize any errors, warnings, or notable issues
2. The last 20 lines show the log's format. Look for `ERROR` and `WARN`
   entries, panics, stack traces, and failure patterns across the file
3. Check the settings and instruction files above when the issue may come
   from configuration
4. Explain what you found in plain language
5. Suggest concrete fixes or next steps
