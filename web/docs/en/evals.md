---
title: Evals
description: Write repeatable test cases for the agent in {{DIR}}/evals and score them with {{COMMAND}} eval.
---

An eval is a task you give the agent again and again, with a way to tell whether it did it right. Evals let you check that a change to your instructions, skills, settings or model still gets the work done, and what it costs.

## Write your first eval

Each eval is a folder under `{{DIR}}/evals/` in your project, with a `case.json` and, if the task needs files, a `fixture/` folder:

```text
{{DIR}}/evals/
└── rename/
    ├── case.json
    └── fixture/
        └── src/
            └── lib.rs
```

`case.json` says what to ask and how to check the work:

```json
{
  "prompt": "Rename greet to welcome",
  "mode": "bypass",
  "checks": [
    { "file_contains": { "path": "src/lib.rs", "text": "fn welcome" } },
    { "file_lacks": { "path": "src/lib.rs", "text": "fn greet" } },
    { "answer_contains": "welcome" },
    { "command": "cargo check -q" }
  ],
  "rubric": "Only the function and its callers changed."
}
```

Run it:

```bash
{{COMMAND}} eval
```

```text
Running rename…
rename  pass    5/5  $0.14
1 of 1 cases passed, $0.14
```

## How a case runs

1. The `fixture/` folder is copied into a new, empty scratch folder. Without a `fixture/`, the scratch folder stays empty. The fixture itself is never changed.
2. The agent runs there once, in print mode, with the case's prompt and `mode`: `{{COMMAND}} -p --output-format json -C <scratch> [--mode <mode>] [--model <model>] [--trust] -- <prompt>`, with `--trust` when your project's folder is trusted.
3. Each check is scored against the agent's answer and the files left in the scratch folder.
4. If the case has a `rubric`, a judge run grades the work against it.

The scratch folder is deleted when the case is done.

> [!IMPORTANT]
> The agent runs in the scratch folder, not in your project, so your project's `{{DIR}}` settings and `{{INSTRUCTIONS}}` are not found there. Put whatever the case needs (instructions, `{{DIR}}/settings.json`, skills) inside `fixture/`. Your own settings in `~/{{DIR}}` still apply. The scratch folder is treated as trusted when your project's folder is trusted; otherwise the fixture's hooks, scripts, MCP servers and `settings.local.json` are skipped (see [Workspace trust](./sandbox-and-safety#workspace-trust)).

> [!WARNING]
> Print mode denies every permission request. Without a `mode`, the run uses `edits`: the agent can change files but any command that asks for permission is refused. Set `"mode": "bypass"` when the task needs to run commands, as the scratch folder is a throwaway copy.

## case.json reference

| Field | Type | Required | Meaning |
|---|---|---|---|
| `prompt` | string | yes | What to ask the agent. It cannot be empty. |
| `mode` | string | no | The permission mode for the run: `ask`, `edits`, `plan` or `bypass` |
| `checks` | list | one of `checks` or `rubric` | Checks scored after the run |
| `rubric` | string | one of `checks` or `rubric` | Criteria a judge grades the work against |

A case with neither checks nor a rubric is refused with `case.json needs checks or a rubric`. Other fields are ignored.

### Checks

Each check is an object with one key:

| Check | Passes when |
|---|---|
| `{"file_contains": {"path": "...", "text": "..."}}` | The file exists and contains the text |
| `{"file_lacks": {"path": "...", "text": "..."}}` | The file does not contain the text, or does not exist |
| `{"answer_contains": "..."}` | The agent's answer contains the text, ignoring case |
| `{"command": "..."}` | The command, run with `sh -c` in the scratch folder, exits with 0 |

Paths are relative to the scratch folder. A `command` check's output is discarded and it gets no input, so it only counts by its exit status.

### Rubric

When a case has a `rubric`, a second run of {{NAME}} acts as the judge, in plan mode and in the same scratch folder, so it can read the files but not change them. It receives the rubric, the task, the agent's answer and the list of changed files (`A` added, `M` modified, `D` deleted, compared with the fixture), and must reply with a JSON object:

```json
{ "passed": true, "why": "Only lib.rs changed." }
```

The rubric counts as one more check. If the judge fails, it is reported as `rubric: <why>`; if its reply has no such object, as `rubric: the judge answered out of form`; if the judge's run itself failed, as `rubric: the judge failed: <error>`.

## Scoring

A case scores one point per check that passes, plus one for the rubric. It passes when every point is won. The cost of the agent's run and the judge's run are added together.

When something goes wrong before the checks, the case scores 0/1 with a note:

| Note | Cause |
|---|---|
| `case.json: ...` | The file is not valid JSON or does not fit the format |
| `case.json needs a prompt` | `prompt` is empty |
| `no scratch folder: ...` | The scratch folder could not be created |
| `could not copy the fixture: ...` | The fixture could not be copied |
| `the agent failed: ...` | The agent's run failed, for example no model is connected |

A failed check is noted as `failed: <check>`, for example `failed: src/lib.rs contains "fn welcome"` or ``failed: `cargo check -q` succeeds``.

## Run evals

| Command | What it does |
|---|---|
| `{{COMMAND}} eval` | Runs every case, in name order |
| `{{COMMAND}} eval rename fix-bug` | Runs only these cases |
| `{{COMMAND}} eval --json` | Prints the scores as JSON |
| `{{COMMAND}} --model <model> eval` | Runs every case, and the judge, with that model |
| `{{COMMAND}} -C <path> eval` | Runs the evals of another project |

A folder is a case only if it holds a `case.json`. While running, each case is announced on standard error as `Running <name>…`.

If `{{DIR}}/evals` does not exist, the command stops with `No evals in <folder>: add <name>/case.json and an optional <name>/fixture/`; if no case matches the names given, with `No eval case matched in <folder>`.

### Table output

```text
rename   pass    4/4  $0.12
fix-bug  fail    1/3  $0.30
           failed: a.txt contains "fixed"
           rubric: touched the tests
1 of 2 cases passed, $0.42
```

Each case shows its name, `pass` or `fail`, the points won, and its cost, followed by its notes. The last line counts the cases that passed and the total cost.

### JSON output

`--json` prints a list with one object per case:

```json
[
  {
    "name": "rename",
    "passed": 4,
    "total": 4,
    "cost_usd": 0.12,
    "notes": []
  }
]
```

| Field | Meaning |
|---|---|
| `name` | The case's folder name |
| `passed` | Points won |
| `total` | Points possible: one per check, plus one for a rubric |
| `cost_usd` | The agent's and judge's cost together |
| `notes` | Why points were lost |

### Exit code

`{{COMMAND}} eval` exits with 0 when every case passed and 1 otherwise, so it can gate a CI job:

```bash
{{COMMAND}} eval --json > eval-results.json
```

> [!TIP]
> Run the same evals with different models to compare them: `{{COMMAND}} --model <a> eval` and `{{COMMAND}} --model <b> eval`. See [Models and providers](./models-and-providers).

See also [Headless mode](./headless), which each case uses under the hood.
