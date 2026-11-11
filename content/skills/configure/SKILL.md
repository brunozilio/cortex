---
name: configure
description: Change how {{NAME}} itself behaves by editing its settings and hooks — model fallback between providers (e.g. "fall back from chatgpt to kiro"), routing sub-agents to another model, the small, advisor or judge model, permissions, and enabled providers. Use when the user asks to configure, set up or change {{NAME}}'s own models, fallback, routing or settings.
---

# Configure {{NAME}}

You change {{NAME}}'s own configuration by editing its files. Settings
are read again at the start of every prompt, so a change applies from
the user's next message; nothing needs a restart.

## Where settings live

Files are JSON objects, read in this order (later wins, key by key):

1. `~/{{DIR}}/settings.json` — the user's own settings, every project.
2. `<project>/{{DIR}}/settings.json` — checked in, shared with the team.
3. `<project>/{{DIR}}/settings.local.json` — this user, this project, not
   committed.

Pick the file by scope: personal model choices go in
`~/{{DIR}}/settings.json`, or in `{{DIR}}/settings.local.json` when the
user wants it for this project only. Write to the checked-in
`settings.json` only when the user says the team should share it.

Before editing, Read the file (it may not exist; then create it with
just your keys). Keep every key already there. Show the user the final
snippet you wrote and the file's path.

## Naming models

A model is named `provider:model`, for example `chatgpt:gpt-6.1` or
`kiro:claude-sonnet-5`. The provider is the id of a connected provider;
a name without a provider stays on the run's provider. Never guess a
model id: run `{{COMMAND}} models` and pick from its list, or ask the
user. If a provider is not connected, tell the user to run
`{{COMMAND}} login` (or `/login` in the chat) for it.

## Model fallback

### Simple: `fallbackModel`

```json
{ "fallbackModel": ["kiro:claude-sonnet-5", "openai:gpt-6.1"] }
```

One model or a list. When the run's model stays overloaded (after the
provider's own retries), the run hands over to the first model of the
list it has not tried yet and keeps going on it. The list alone answers
only overloads; for auth, network or provider errors use a hook.
`--fallback-model` sets one model for one `{{COMMAND}} -p` run.

Use it when the user's main model is on one provider and they want
backups on others: main `chatgpt:…`, `fallbackModel` `["kiro:…"]`.

### Any error, any policy: a `ModelError` hook

A `ModelError` hook intercepts every failed model call, before the
`fallbackModel` list. A script in `{{DIR}}/hooks/` (project) or
`~/{{DIR}}/hooks/` (user) gets as input `model`, `kind` (`main` or
`sub-agent`), `mode`, `messages`, `error`, `errorKind` (`overloaded`,
`unauthorized`, `network`, `provider`, `too_long`), `attempt`, `tried`
(models that failed since the last answer) and `fallbacks` (the
settings' list). It answers one of:

- `{ hookSpecificOutput: { model: "kiro:…" } }`: forward the call there.
- `{ decision: "block", reason: "…" }`: stop, the run fails with the error.
- nothing: the `fallbackModel` list decides.

```js
export const event = "ModelError";
export const matcher = "main";
const chain = ["kiro:claude-sonnet-5", "openai:gpt-6.1"];
export default function (input) {
  if (input.errorKind === "too_long") return;
  const next = chain.find((model) => !input.tried.includes(model));
  if (next) return { hookSpecificOutput: { model: next } };
}
```

A model already in `tried` is never tried again before a call
succeeds, and a streak stops after a few switches, so a hook cannot
loop forever.

### Choosing the model per call: a `ModelRequest` hook

`ModelRequest` runs before every model call with the same input (no
`error`). Its matcher is `main` or `sub-agent`, so sub-agents can go to
a cheaper provider:

```js
export const event = "ModelRequest";
export const matcher = "sub-agent";
export default () => ({ hookSpecificOutput: { model: "kiro:claude-haiku-5" } });
```

Hooks can also live under `"hooks"` in a settings file, in the same
shape as the other hook events.

## Other model settings

- `smallModel`: runs the `Explore` agent when it names no model.
- `advisorModel`, `judgeModel`: the advisor and the verification judge.
- `enabledProviders`: `["groq"]` lets a provider declared by the project
  in `{{DIR}}/providers/*.js` start. Only the user's own files
  (`~/{{DIR}}/settings.json`, `settings.local.json`) can enable one.

## Permissions

`permissions.allow`, `permissions.ask` and `permissions.deny` take rules
such as `Bash(cargo test:*)`, `Edit(src/**)` or `mcp__server`. Only the
user's own files can allow; the checked-in file can only restrict.

## After the change

Confirm what will happen in one or two sentences, e.g. "From your next
message, when chatgpt is overloaded the run continues on
kiro:claude-sonnet-5." If the request cannot be expressed with these
settings and hooks, say exactly which part is not supported rather than
writing something that will not take effect.
