---
title: Accounts
description: Connect several accounts of one provider, switch between them, and let them take over from each other when one is busy.
---

A provider can hold more than one account: two API keys, or two subscriptions signed in one after the other. One account is in use at a time, and you choose whether the others stay idle, step in when it is busy, or take turns.

## Add another account

Connect the provider again with `{{COMMAND}} login` or `/login`, using another key or signing in as another user. The new account is added next to the ones you have and becomes the account in use.

Each account is named after what identifies it:

| Account | Its name |
|---|---|
| API key | `…` and the key's last four characters, for example `…bbbb` (keys shorter than 12 characters show only `…`) |
| Subscription | The account's email; else its account id; else the provider's name and the first eight characters of the user id; else the provider's name with a number, such as `Claude Subscription #2` |

Signing in again as an account you already have replaces its tokens instead of adding a second copy.

## Switch accounts

```text
/accounts
```

It opens a picker with every account of every connected provider, written `<provider> · <account>`, with the one in use marked `in use`. The cursor starts on the first account not in use. Pick one and it is used from the next message: the chat says `Using <account> from the next message.`

If no account is connected, it says `No account is connected: /login connects one.`

## Remove an account

```text
/accounts remove
```

It opens the same list, titled *Forget which account?*. The account you pick is forgotten, together with its stored key or tokens: `Forgot <account>.` If it was the one in use, the provider's first remaining account takes its place.

To forget every account of a provider at once, use `/logout` or `{{COMMAND}} logout`:

```bash
{{COMMAND}} logout
```

```text
  1. Anthropic API
  2. Grok Subscription
Forget which? 1
Forgot Anthropic API.
```

## Share the work between accounts

`/accounts balance` sets how the accounts of the current model's provider share the work. Without a value it lists the choices.

```text
/accounts balance failover
```

| Balance | What happens |
|---|---|
| `off` | Only the account in use. This is the default. |
| `failover` | The account in use, then the others when it is busy or signed out |
| `round-robin` | Each run starts on the next account, and the others take over when one is busy |

The chat confirms with `Accounts balance: <balance> (<what it does>).` The balance is kept per provider, so each provider can have its own.

### Takeovers

With `failover` or `round-robin`, an account hands the turn to the next one when the provider answers that it is overloaded or that the account is signed out. The chat shows:

```text
Account …aaaa is busy or signed out; going on with …bbbb.
```

An account takes over only if the failing one had not started answering yet; a turn that already streamed text is not repeated on another account. Network errors and other provider errors are not handed over. When every account has failed, the run fails with the last error, and the [fallback models](./models-and-providers#fallback-models) are tried next.

> [!TIP]
> Accounts spread work inside one provider; fallback models switch to another model, possibly at another provider. They work together: accounts first, then fallbacks.

## Commands

| Command | What it does |
|---|---|
| `/accounts` | Pick the account in use |
| `/accounts remove` | Forget one account |
| `/accounts balance` | List the balance choices |
| `/accounts balance off\|failover\|round-robin` | Set how the current provider's accounts share the work |
| `/login` | Connect a provider, or add an account to one |
| `/logout` | Forget a provider with all of its accounts |
| `{{COMMAND}} login` | Connect from a terminal, outside the chat |
| `{{COMMAND}} logout` | Forget a provider from a terminal |

See also [Models and providers](./models-and-providers) and [Slash commands](./slash-commands).
