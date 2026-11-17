---
title: Models and providers
description: Connect an LLM provider with an API key or a subscription, pick a model, and tune roles, fallbacks, effort and context.
---

{{NAME}} does not ship with a model of its own: it talks to the LLM providers you connect. You can connect several providers at once, and every model they offer shows up in one list. This page covers connecting providers, choosing models, giving models to roles, setting fallbacks, and tuning reasoning effort.

## Connect a provider

Run the login command from a terminal:

```bash
{{COMMAND}} login
```

It prints the providers to choose from, each with how it connects, and asks for a number:

```text
Connect a provider:
  1. Anthropic API (paste an API key)
  2. Claude Subscription (sign in with the browser)
  3. OpenAI API (paste an API key)
  ...
Choose 1-7:
```

- **API key providers** ask for the key. What you type is hidden and shown as `•`; pasting works. Esc or Ctrl+C cancels.
- **Browser sign-in providers** open the sign-in page in your browser. If it does not open, the URL is printed so you can visit it yourself.
- **Code sign-in providers** print a code and a URL (`Enter the code AB12-CD34 at https://github.com/login/device`), open the browser, and wait until you approve.

When it works, it prints `Connected <provider> (<account>).`

Inside the chat, `/login` opens the same list as a picker.

> [!TIP]
> Run `{{COMMAND}} doctor` after connecting: its `providers` and `models` lines confirm that the provider is connected and that it offers models. See [Troubleshooting](./troubleshooting).

### API keys versus subscriptions

| Kind | How it connects | What you pay |
|---|---|---|
| API key | You paste a key from the provider's console | The provider bills the key per token |
| Subscription | You sign in in the browser or with a device code | Your existing subscription plan |

Keys and sign-in tokens are kept in your data folder, sealed in `secrets.json` (readable only by you), whose key is in `secrets.key` beside it, or in your system keychain if you set `{{ENV}}_SECRETS=keychain`. Subscription tokens are renewed on their own when they expire. See [Files and directories](./files-and-directories).

> [!NOTE]
> Keys are read only from what you connect with `{{COMMAND}} login` or `/login`. {{NAME}} does not read provider keys from environment variables.

A key is trimmed of surrounding spaces; an empty key, or one with spaces inside, is refused.

## Supported providers

The providers are listed in this order in `{{COMMAND}} login` and `/login`:

| Provider | Id | Connects with | Default context window |
|---|---|---|---|
| Anthropic API | `anthropic` | API key | 200,000 tokens |
| Claude Subscription | `claude-subscription` | Browser sign-in | 200,000 tokens |
| OpenAI API | `openai` | API key | 400,000 tokens |
| ChatGPT Subscription | `openai-subscription` | Browser sign-in | 400,000 tokens |
| GitHub Copilot Subscription | `copilot-subscription` | Device code | 131,072 tokens |
| Kiro Subscription | `kiro-subscription` | Browser sign-in | 200,000 tokens |
| Grok Subscription | `grok-subscription` | Device code | 500,000 tokens |
| Antigravity Subscription | `antigravity-subscription` | Browser sign-in | 200,000 tokens |
| Declared providers | your choice | API key | 128,000 tokens unless declared |

When a provider's catalog says how large a model's window is, that number is used; otherwise the default in this table applies.

### Anthropic API

Paste a key from the Anthropic console. Turns go to the Messages API at `api.anthropic.com`, and the model list comes from the API's model catalog. Prices, short model names and thinking rules come from the built-in Anthropic model family.

### Claude Subscription

Signs in through the browser; the sign-in page sends you back to `http://localhost:<free port>/callback`. Turns go to the Messages API with your subscription. When the account's catalog answers empty, a fixed list of current models is offered.

### OpenAI API

Paste a key from the OpenAI platform. Turns go to the Responses API at `api.openai.com/v1`; only chat models from the catalog are listed.

### ChatGPT Subscription

Signs in through the browser with the callback `http://localhost:1455/auth/callback`, so port 1455 must be free while you sign in. Turns go to the Codex backend at `chatgpt.com/backend-api/codex`, always with a reasoning effort. Tokens are renewed before they expire, and once more after a 401.

### GitHub Copilot Subscription

Signs in with GitHub's device flow: you type the printed code at `github.com/login/device`. Classic `ghp_` tokens are refused. The picker shows what Copilot's `/models` lists, and each model runs on the best API it offers (`/v1/messages`, then `/responses`, then `/chat/completions`). Tool round-trips are not sent as premium requests: only turns that start with your own message are.

### Kiro Subscription

Signs in through the browser via `app.kiro.dev/signin`, with the callback `http://localhost:49153/oauth/callback`. Kiro accepts only that port, so it must be free while you sign in. Google, GitHub, Builder ID and organization (IAM Identity Center) sign-ins work; an organization sign-in then continues on your organization's page and needs a Kiro profile for your account, otherwise it fails with `signed in, but this account has no Kiro profile`. Tokens refresh on their own, also after a 401 or 403.

### Grok Subscription

Uses a SuperGrok or X Premium subscription. Signs in with xAI's device flow: a code and a link to approve it are shown, and {{NAME}} waits until you approve. Turns go to xAI's Responses API at `api.x.ai/v1`. Tokens last about six hours and are renewed before they expire and once after a 401. Only Grok chat models are listed (no image, video, speech or embedding models).

Only some Grok models accept an effort level (`grok-3-mini`, `grok-4.20-multi-agent`, `grok-4.3`, `grok-4.5`, `grok-4.6`, or what the catalog says); `grok-4.6` accepts `xhigh`, the others stop at `high`. Other Grok models reason on their own.

### Antigravity Subscription

Uses the Google account you use in Antigravity. Signs in through Google in the browser with the callback `http://localhost:51121/oauth-callback`, so port 51121 must be free while you sign in. After the sign-in your Cloud Code project is looked up (an account without one is set up for the free tier). Turns go to Google's Cloud Code backend (`daily-cloudcode-pa.googleapis.com`), which serves the Gemini, Claude and GPT-OSS models the picker lists; the thinking level is part of the model's name (`-low`, `-medium`, `-high`). Tokens refresh on their own, also after a 401.

This is not an official client: Google may restrict accounts that use Antigravity's sign-in outside Antigravity.

### Declared providers

Any other API that speaks one of the supported wires can be added with a JavaScript file in `~/{{DIR}}/providers/` (yours) or `{{DIR}}/providers/` in a project. The file's default export declares the provider:

```javascript
export default {
  id: "groq",
  name: "Groq",
  wire: "chat",
  base: "https://api.groq.com/openai/v1",
  headers: { "x-team": "core" },
};
```

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Lowercase letters, digits and dashes. It cannot be a built-in provider's id. |
| `name` | yes | The name shown in the login list and the model labels. |
| `wire` | yes | `chat` (`/chat/completions`), `responses` (`/responses`) or `messages` (`/messages`). |
| `base` | yes | The API's base URL: `https://`, or `http://` only on `localhost` or `127.0.0.1`. |
| `authHeader` | no | The header that carries the key. Default `authorization`. |
| `authPrefix` | no | Text put before the key. Default `Bearer `. |
| `headers` | no | Extra headers sent on every request. |
| `models` | no | Model ids to offer. Without it, the models come from `GET <base>/models`. |
| `window` | no | The context window in tokens. Default 128,000. |

Unknown fields are refused. A declared provider connects with an API key, like the built-in key providers, and appears last in the login list. Its models are named `<id>:<model>`, for example `groq:llama-4`.

A file can also export a `shape` function, which receives each request body and returns the body to send (or nothing to keep it as is). It runs with no file, process or network access and has two seconds:

```javascript
export default { id: "local", name: "Local", wire: "chat", base: "http://localhost:8080/v1" };

export function shape(body) {
  return { ...body, temperature: 0.2 };
}
```

> [!IMPORTANT]
> A project's declared providers load only when you trust the project's folder (see [Workspace trust](./sandbox-and-safety#workspace-trust)) and have approved them: add their ids to `enabledProviders` in your own settings (`~/{{DIR}}/settings.json`, or the trusted project's `settings.local.json`). Until then `{{COMMAND}} doctor` reports that the provider is skipped or waits.

```json
{
  "enabledProviders": ["groq"]
}
```

`{{COMMAND}} doctor` reports on every file: how many loaded, or why one was left out.

## List the models

```bash
{{COMMAND}} models
```

It prints every model the connected providers offer, one per line: the id, then the label.

```text
anthropic:claude-sonnet-5-5              <label from the provider>
groq:llama-4                             llama-4
```

Model ids are `<provider>:<model>`. Wherever you name a model (`--model`, `/model`, roles, fallbacks), {{NAME}} accepts the full id, the part after the colon, or the label, ignoring case.

When a connected provider does not answer with its model list, the chat says so once for that provider (`Could not list <provider>'s models: <reason>`) and goes on with the models of the others.

## Choose a model

| Where | How |
|---|---|
| Starting the chat | `{{COMMAND}} --model <model>` (or `-m`) |
| One printed answer | `{{COMMAND}} -p --model <model> "..."` |
| In the chat | `/model` opens a picker; `/model <model>` switches directly |

The model you pick in the chat is remembered and used next time. Without `--model`, {{NAME}} uses the last model you picked, or the first model offered. In print mode, a model name that matches nothing stops the command with `no model named <name>`.

`/status` shows the model in use, with its id.

## Model roles

Some work does not need the session's model. Roles let you give it another one:

| Role | What it is used for |
|---|---|
| `plan` | Plan mode and the Plan agent |
| `explore` | The Explore agent that searches the code (falls back to the `small` model) |
| `small` | Background chores: titles, effort picks, web reading |
| `review` | The judge that checks finished work |
| `advisor` | The advisor the agent consults |

A role without a model uses the session's model.

| Command | What it does |
|---|---|
| `/model roles` | Lists each role, its model and what it does |
| `/model <role>` | Opens a picker for that role; its first entry, *Session model*, clears it |
| `/model <role> <model>` | Gives the role that model |
| `/model <role> default` | Clears the role (`none` and `off` work too) |

```text
/model plan claude-opus-5-5
/model small claude-haiku-5-5
/model review off
```

The roles you set in the chat are remembered. Your settings files can set three of them as well, and what you set in the chat wins over them:

| Setting | Role |
|---|---|
| `smallModel` | `small` |
| `judgeModel` | `review` |
| `advisorModel` | `advisor` |

See [Settings](./settings) for where the settings files live.

## Fallback models

When the model call fails because the model is overloaded, you are signed out, or the network fails, {{NAME}} can go on with another model. The chat shows `The model call failed; going on with <model>.`

| Command | What it does |
|---|---|
| `/model fallbacks` | Shows the chain |
| `/model fallbacks <model> [model…]` | Sets the chain, tried in order |
| `/model fallbacks off` | Clears the chain (`default` and `none` work too) |

```text
/model fallbacks claude-sonnet-5-5 claude-haiku-5-5
```

Fallbacks come from one of three places, the first that is set:

1. `--fallback-model <model>`: one model, for this run.
2. The chain set with `/model fallbacks` (remembered between chats).
3. `fallbackModel` in your settings: a model name, or a list of them.

```json
{
  "fallbackModel": ["claude-sonnet-5-5", "claude-haiku-5-5"]
}
```

A run tries each model at most once and switches at most eight times. Other failures, such as a provider error or a prompt that is too long, do not move down the chain.

> [!TIP]
> To share the load between several accounts of the same provider instead, see [Accounts](./accounts).

## Effort

Effort sets how much the model reasons before it answers.

| Level | Meaning |
|---|---|
| `low` | Quickest answers, least reasoning |
| `medium` | Balanced (the default) |
| `high` | More reasoning for harder work |
| `xhigh` | Extended reasoning |
| `max` | As much reasoning as the model allows |
| `auto` | Each request picks the model and the effort it needs |

`/effort` opens a picker; `/effort <level>` sets it directly; `/effort auto` turns on automatic picks. The choice is remembered.

Each provider turns the level into what its API understands. For example, current Claude models get adaptive thinking with the effort, and models that think with a token budget get a share of their output budget. A model that does not support the chosen level refuses the request with `model <model> does not support effort <level>`.

### Automatic effort

With `/effort auto`, before each request a quick call to the small model decides whether the request needs the session's model or the small model, and whether it needs low, medium or high effort. The chat shows the pick as `Auto: <model>, <effort> effort`. Bare go-aheads (`ok`, `okay`, `yes`, `y`, `go`, `go on`, `continue`, `proceed`, `sim`, `continua`, `pode`, `segue`, with or without a final `.` or `!`) skip that call and use the session's model at low effort. `/effort <level>` goes back to one fixed level.

## Context window

Each model has a context window: how many tokens of conversation it can read. {{NAME}} uses the window the provider's catalog gives for the model, or the provider's default from [the table above](#supported-providers).

- `/context` shows a bar of how full the window is, how much is free before compacting, and what fills it (system prompt, tools, MCP tools, agents, memory, skills, messages).
- When the conversation nears the window, it is compacted; `/compact [what to keep]` does it on demand. See [Chat](./chat).

## Prompt caching

{{NAME}} marks what stays the same between turns so providers can cache it: on the Messages wire the tools, the system prompt and the conversation are cached separately; on the Responses wire turns of one conversation share a cache key.

Provider caches expire after about five minutes without a call. With the `cache-warning` feature on, the status line counts down the last two minutes (`cache 1:45`) after a call that cached at least 10,000 tokens, then says `cache cold: next turn rewrites 48k tokens` once it has expired. See [Experimental features](./experimental-features) and [Budget and cost](./budget-and-cost).

## Forget a provider

```bash
{{COMMAND}} logout
```

It lists the connected providers and asks which to forget. `/logout` in the chat does the same with a picker. Forgetting a provider removes all of its accounts; to remove a single account, see [Accounts](./accounts#remove-an-account).
