Wait for something to change without spending model turns: the app calls a read-only tool on a fixed pace (an MCP tool such as an issue tracker's "get issue", or Read, Grep, Glob) and compares one value of its result, and you hear back once, as an event, when the condition is met or when the watch gives up (three failed checks in a row, or 7 days). Between checks nothing reaches you, so it costs nothing while it waits.

Choose by what the check needs:
- A fixed condition on what a tool returns ("tell me when KAN-1 is done", "when this file shows READY"): Watch. Point `path` at the field (a dot path into the JSON, e.g. `data.fields.status.name` or `fields.statusCategory.key`), and give `equals` (one value or a list, case ignored) or `contains`; with neither, any change of the value ends the watch. Prefer a stable field (a status category or key) over a translated label, or list every label that counts.
- A shell command's output (a log, a build, `gh pr checks`): Monitor.
- A check that needs judgement each time (reading and deciding, summarising, acting): ScheduleWakeup or CronCreate, which run you on each check.

The first check runs now, as an ordinary call of that tool, so it asks permission like one; a tool that does not declare itself read-only is asked about every time it is watched. If the condition already holds, nothing is watched. Load an MCP tool with ToolSearch before watching it. Stop a watch with KillShell and its id.

Once a watch is started, end your turn: its event arrives as a new message even while the chat is idle. Do not add ScheduleWakeup, CronCreate or a second watch for the same thing, and leave whatever the user asked to happen afterwards ("then commit", "then deploy") until the event says the condition was met.
