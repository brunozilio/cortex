---
title: Schedules
description: Repeat a task on an interval with /loop, have the agent schedule prompts with cron expressions or one-off wakeups, and list or delete what is scheduled.
---

A scheduled prompt is sent to your conversation later, as if you had typed it: every ten minutes, every weekday at 9:00, or once in half an hour. Use it to keep an eye on a build, a deploy or a pull request without polling by hand.

## Repeat a task with /loop

```text
/loop 5m check whether the deploy finished and tell me if it failed
```

`/loop` runs the task now, then has the agent schedule it again on the interval you gave:

| Interval | Schedule |
|---|---|
| `<n>s` | Every `⌈n/60⌉` minutes (seconds round up to whole minutes, 1 to 59) |
| `<n>m` | Every n minutes; 60 or more becomes whole hours, rounded down (`90m` is every hour) |
| `<n>h` | Every n hours; 24 or more becomes whole days, rounded down |
| `<n>d` | Every n days, up to 31; a longer one is not read as an interval, so the whole text is the task |

The agent creates the schedule with **CronCreate** and tells you how to stop it.

Without an interval, the agent paces itself:

```text
/loop babysit the PR until CI is green
```

It does the task, and when it finishes, unless the task is done for good, it schedules its own next wakeup (between 60 and 3600 seconds) to come back to it.

`/loop` with no task explains its usage: `/loop [5m|2h|1d] <task>`.

## Ask the agent to schedule something

You can also just ask: "every weekday at 9, summarize yesterday's commits", "remind me in 30 minutes to check the migration". The agent has four tools for it:

| Tool | Does |
|---|---|
| **CronCreate** | Schedules a prompt on a cron expression, recurring (the default) or only the next match |
| **ScheduleWakeup** | Sends a prompt once, after a delay of 60 to 3600 seconds |
| **CronList** | Lists the prompts scheduled for this session |
| **CronDelete** | Removes one by its id |

The agent is told that each firing is a full model turn: to wait for a fixed condition (an issue reaching a status, a file changing), it should use **Watch**, which checks without spending a turn; to follow a command's output, **Monitor**.

### Cron expressions

A cron expression has five fields, in your **local** time:

```text
┌ minute (0-59)
│ ┌ hour (0-23)
│ │ ┌ day of the month (1-31)
│ │ │ ┌ month (1-12)
│ │ │ │ ┌ day of the week (0-7, Sunday is 0 or 7)
│ │ │ │ │
*/10 * * * *
```

Each field takes `*`, a number, a range (`1-5`), a list (`15,45`) and a step (`*/5`, `8-18/2`). As in standard cron, when both the day of the month and the day of the week are restricted, a day matching either one fires.

| Expression | Fires |
|---|---|
| `*/10 * * * *` | Every 10 minutes |
| `0 9 * * 1-5` | At 9:00 on weekdays |
| `15,45 8-10 * * *` | At :15 and :45 past 8, 9 and 10 |
| `0 0 1 * *` | At midnight on the first of each month |

## List and delete scheduled prompts

```text
/schedules
Scheduled (/schedules delete <id> removes one):
  cron_3    every `*/10 * * * *`, in 4m: check whether the deploy finished
  cron_4    once, in 27m: check the migration
```

Each line shows the id, how it repeats, the minutes until it next fires and the prompt. `/schedules delete <id>` removes one.

## When a scheduled prompt fires

{{NAME}} checks every second for prompts that are due. A due prompt is sent only while the chat is idle: no run going and no question waiting for you. It appears in the chat as `Scheduled (<id>): <prompt>` and starts a run like a message you typed.

> [!IMPORTANT]
> Schedules live only as long as the {{NAME}} process that created them. They are not saved: quitting the chat drops them, and they fire only while it is open.

Limits:

- at most **50** scheduled prompts at a time;
- a recurring prompt stops after **7 days**;
- a cron expression must match a moment within the next year.

## Turn schedules off

Set `"schedules": false` under `features` in your settings (see [Feature flags](./experimental-features#turning-off-a-whole-built-in-feature)). `/schedules` then fails and due prompts are no longer delivered.

## Related

- [Slash commands](./slash-commands): `/loop` and `/schedules` with the other commands.
- [Branches and git](./branches-and-git#follow-a-pull-request-to-merge): following a pull request with a monitor instead of a schedule.
