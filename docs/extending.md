# Extending models: providers and routing

## Your own provider

A file in `~/.{command}/providers/*.js` declares a provider. Its default
export says which of the built-in wire formats it speaks, so the
request and stream code is the engine's own:

```js
export default {
  id: "groq",
  name: "Groq",
  wire: "chat",
  base: "https://api.groq.com/openai/v1",
  models: ["llama-4-scout"],
};
```

| Field | Meaning |
|---|---|
| `id` | Lowercase letters, digits and dashes; not a built-in provider's id. Models are named `id:model`. |
| `name` | Shown by `/login` and the model picker. |
| `wire` | `chat` (`/chat/completions`), `responses` (`/responses`) or `messages` (Anthropic's `/messages`). |
| `base` | The API root with its version, such as `…/v1`; https, or http on localhost. |
| `authHeader`, `authPrefix` | Where the key goes; `authorization` and `Bearer ` by default. |
| `headers` | Extra headers sent with every call. |
| `models` | The models offered; without it the provider's `GET /models` is read. |
| `window` | The context window, 128000 by default. |

A file may also `export function shape(body) { … }`: it gets each
request body just before it is sent and returns the body to send (or
nothing to keep it). It runs with no files, processes or network and
at most 2 seconds; an error stops the call with the provider's name.

The file is read when the app starts and again on `/reload` (`backend::native::
declare_providers`, `agent_engine::declared_providers`), checked and
registered beside the built-in providers (`llm_providers::declared`).
Connect it with `/login` like any provider that takes an API key. A file
that cannot be read or declares something invalid is left out; `/doctor`
(and `{command} doctor`) names each one and why, and the log has it too.

A project can bring providers in `.{command}/providers/*.js` too, but
one starts only when the owner approves its id in their own settings
(`~/.{command}/settings.json` or the project's `settings.local.json`):

```json
{ "enabledProviders": ["groq"] }
```

## Routing

A model named `provider:model` (the catalog's own id form) runs on that
provider with the owner's key for it, wherever a model is named:

- `fallbackModel` / `--fallback-model`: `"openai:gpt-6.1-sol"` takes over
  when the run's model stays overloaded.
- The `ModelRequest` hook runs before each model call and may answer
  `{"hookSpecificOutput": {"model": "groq:llama-4-scout"}}`. Its matcher
  is the kind of call, `main` or `sub-agent`, so sub-agents can go to a
  cheaper provider.
- The `ModelError` hook runs when a call fails and may name one model to
  try next.

Any other name stays on the run's provider. A provider that is not
connected says how to log in.
