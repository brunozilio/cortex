---
name: debug
description: Help diagnose an issue with {{NAME}} itself, from its logs and settings
disable-model-invocation: true
---

# Debug Skill

Help the user debug an issue they're encountering with {{NAME}}: the app,
this session's agent, or its tools.

## Logs

{{NAME}} writes its diagnostics to a log only on Windows; elsewhere they go
to standard error. Find the ones for the user's system:

* Windows - `%LOCALAPPDATA%\{{NAME}}\logs\{{COMMAND}}-<date>-<pid>.log`, one
  file per launch (the newest is the current one).
* macOS - standard error of the app's process: nothing is kept when
  {{NAME}} is opened from the Dock or Finder. To capture a run, quit it and
  start the binary from a terminal, e.g.
  `/Applications/{{NAME}}.app/Contents/MacOS/{{COMMAND}} 2> ~/{{COMMAND}}.log`.
* Linux - standard error: the terminal it was started from, or the user
  journal when the desktop started it (`journalctl --user -b | grep -i {{COMMAND}}`).

The default level is `{{COMMAND}}=info,warn`. For more detail, start {{NAME}}
with `RUST_LOG={{COMMAND}}=debug` (or `{{COMMAND}}=trace`, which also logs every
frame's timing) in its environment, then reproduce the issue.

If no log covers the moment the issue happened, tell the user where the log
will be, how to raise the level if useful, and ask them to reproduce the
issue; then read it again.

For context, grep for `ERROR` and `WARN` lines across the whole file.

## Data

* App data (sessions, settings kept by the server, memory, notes, plugins):
  `~/Library/Application Support/{{NAME}}` on macOS, `%APPDATA%\{{NAME}}` on
  Windows, `$XDG_CONFIG_HOME/{{COMMAND}}` (or `~/.config/{{COMMAND}}`) on Linux.
* Local data (models, worktrees, browser profiles, the agent's sessions in
  `native-sessions`): the same folder, except `%LOCALAPPDATA%\{{NAME}}` on
  Windows.

## Settings

Remember that the agent's settings are in:
* user - `~/{{DIR}}/settings.json`
* project - `{{DIR}}/settings.json`
* local - `{{DIR}}/settings.local.json`

Skills are in `{{DIR}}/skills`, agents in `{{DIR}}/agents`, rules in
`{{DIR}}/rules` (in the project, or under `~/{{DIR}}` for the user).
Instructions come from `{{INSTRUCTIONS}}` first, then `AGENTS.md`.

## Instructions

1. Review the user's issue description; if they gave none, read the log and
   summarize any errors, warnings, or notable issues
2. The last 20 lines show the log's format. Look for `ERROR` and `WARN`
   entries, panics, stack traces, and failure patterns across the file
3. Check the settings and instruction files above when the issue may come
   from configuration
4. Explain what you found in plain language
5. Suggest concrete fixes or next steps
