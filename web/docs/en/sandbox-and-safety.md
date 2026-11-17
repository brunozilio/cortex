---
title: Sandbox and safety
description: The sandbox, secret redaction, snapshots before destructive commands, the dependency guard and the other checks that keep the agent's actions safe.
---

[Permissions](./permissions) decide what the agent may do. On top of them, {{NAME}} has a set of safety mechanisms that limit the damage a mistaken or manipulated action can do. Most are on by default; this page explains each one and how to turn it on or off.

## Overview

| Mechanism | On by default | Turned off with | Where |
|---|---|---|---|
| [Sandbox](#sandbox) | Yes | `features.experimental-sandbox` | macOS only |
| [Secret redaction](#secret-redaction) | Yes | `features.redact-secrets` | All systems |
| [Snapshots before destructive commands](#snapshots-before-destructive-commands) | Yes | `features.destructive-snapshot` | All systems |
| [Dependency guard](#dependency-guard) | Yes | `features.experimental-dependency-guard` | All systems |
| [Commands that expose secrets](#commands-that-expose-secrets) | Always | cannot be turned off | All systems |
| [Actions that always ask](#actions-that-always-ask) | Always | cannot be turned off | All systems |
| [Safety check in bypass mode](#safety-check-in-bypass-mode) | Always in bypass | use another mode | All systems |
| [Untrusted content](#untrusted-content) | Always | cannot be turned off | All systems |

Features are switched with `/features <name> on|off` (saved to `~/{{DIR}}/settings.json`) or the `features` key of your settings, see [Settings](./settings#features).

> [!IMPORTANT]
> The sandbox, secret redaction and the dependency guard are **security features**. A project's checked-in `{{DIR}}/settings.json` can turn them on but never off; only your own settings files can turn them off.

## Sandbox

> [!EXPERIMENTAL]
> The sandbox is the `experimental-sandbox` feature. It works on macOS only; on other systems `/features` shows it as "not on this system" and commands run unconfined.

On macOS, shell commands the agent runs (the `Bash` and `Monitor` tools), command hooks and background test runs are started inside the system sandbox (`/usr/bin/sandbox-exec`). Inside it:

- **Reading** is not restricted.
- **Writing** is allowed only in:
  - the project folder;
  - the [additional folders](./permissions#additional-directories);
  - the repository, when the session works in its own worktree;
  - the temporary folders (the system temporary folder and `/tmp`);
  - package caches in your home folder: `Library/Caches`, `.cache`, `.npm`, `.cargo/registry`, `.cargo/git`, `.cargo/.package-cache`, `.cargo/.package-cache-mutate`, `.cargo/.global-cache`, `go/pkg`, `.gradle`, `.rustup`, `.pnpm-store`, `.yarn`;
  - the folders in `sandbox.allowWrite`;
  - terminal devices such as `/dev/null` and `/dev/tty`.
- `{{DIR}}/settings.json` and `{{DIR}}/settings.local.json` in the project and in the added folders can **never** be written by a sandboxed command, so a command cannot grant itself permissions.
- **Network** access is allowed unless `sandbox.network` is `false`.

When a command fails with "operation not permitted", {{NAME}} adds a hint to its output telling the agent not to work around the sandbox and to tell you, so you can add a folder to `sandbox.allowWrite`.

### Configuring the sandbox

```json
{
  "sandbox": {
    "allowWrite": ["~/.m2", "/opt/shared-data"],
    "network": false
  }
}
```

| Key | Default | Description |
|---|---|---|
| `sandbox.allowWrite` | `[]` | Extra writable folders: absolute or starting with `~/`. Relative paths are ignored. Only read from your own settings files. |
| `sandbox.network` | `true` | `false` blocks all network access for sandboxed commands. A project's checked-in settings can turn it off but not back on. |

### Running a command outside the sandbox

When a command really needs to write elsewhere, the agent can ask to run it unsandboxed. You are asked every time, with "Run OUTSIDE the sandbox (it may write anywhere)", and the answer cannot be remembered. In `-p` runs, where nobody can answer, it is refused.

### Turning the sandbox off

```bash
/features experimental-sandbox off
```

or `{{COMMAND}} config set features.experimental-sandbox false`.

## Secret redaction

With `redact-secrets` on (the default), tool output is cleaned before the model, the transcript and the logs see it. The same applies to tool inputs shown on screen, live command output and full command outputs kept in files. Each secret is replaced with `[redacted]`:

- tokens with known prefixes: `sk-`, `sk_`, `ghp_`, `gho_`, `ghs_`, `ghu_`, `github_pat_`, `glpat-` (20 characters or more), `xoxb-`, `xoxp-`, `xoxa-`, `xoxs-`;
- AWS access key ids (`AKIA…`, `ASIA…`);
- JSON Web Tokens (`eyJ…` with three parts);
- the value in `key=value` or `key: value` when the key ends with `password`, `passwd`, `secret`, `token`, `api_key`, `apikey` or `api-key`;
- the word after `Bearer`;
- every value of 8 characters or more in the project's `.env` and `.env.*` files (except `.env.example` and `.env.sample`).

```text
config:
    API_KEY=[redacted]
    auth: Bearer [redacted]
```

Turn it off with `/features redact-secrets off`.

## Snapshots before destructive commands

With `destructive-snapshot` on (the default), {{NAME}} keeps a copy of the files a command is about to destroy, so `/rewind` can bring them back even though they were never committed. It looks for:

- `rm` with paths (every file under a removed folder is kept);
- `git clean` (untracked files; with `-x` or `-X` also ignored ones);
- `git reset --hard`;
- `git checkout .`, `git checkout -- …`, `git restore .`, `git restore -- …`;
- `git stash -u`, `--include-untracked`, `-a`, `--all`.

At most 5,000 files and 200 MB are kept per command; when more would be needed, the output says how many were left out. See [Sessions](./sessions) for `/rewind`.

## Dependency guard

> [!EXPERIMENTAL]
> The dependency guard is the `experimental-dependency-guard` feature.

Before a command installs a package, {{NAME}} looks the package up in its registry and asks you first when:

- it is **not published** (the name may be made up);
- it **looks like a popular package with a typo** (one letter added, missing or changed, or two neighbouring letters swapped, for example `expres` for `express`);
- it was **first published less than 30 days ago**.

| Ecosystem | Commands checked | Registry |
|---|---|---|
| npm | `npm`/`pnpm`/`bun` `add`, `install`, `i`; `yarn add` | registry.npmjs.org |
| Rust | `cargo add` | crates.io |
| Python | `pip`/`pip3`/`pipx install`, `uv pip install`, `uv add`, `poetry add`, `python -m pip install` | pypi.org |

The question shows "Check the package first: …" with the reason. It is asked in every mode, including bypass; in `-p` runs the command is refused.

## Commands that expose secrets

Some shell commands are refused outright, in every mode, because they would print secrets into the conversation:

- printing the environment: `env`, `set`, `export` or `export -p` on their own, and `printenv`;
- reading a secret file with a content reader (`cat`, `head`, `grep`, `base64`, `cp`, `curl`, `jq`, `openssl`…): `.env` and `.env.*` (except example, sample, template and dist variants), SSH keys (`id_rsa`, `id_ed25519`, `id_ecdsa`, `id_dsa`), `.npmrc`, `.netrc`, `.pypirc`, `.git-credentials`, `credentials`, `secrets.json`, and files ending in `.pem`, `.key`, `.p12`, `.pfx`.

The agent is told to let you run such a command yourself with `! <command>` if you need it.

## Actions that always ask

These are asked about every time, even in bypass mode and even with an allow rule, and cannot be remembered:

- `git push` to the default branch, or with `--force`, `--force-with-lease`, `--mirror`, `--delete`, `--prune`, or refspecs starting with `+` or `:`;
- `cargo`, `npm`, `pnpm` or `yarn` `publish`;
- `gh pr merge`, `gh release create|delete|edit`, `gh api`;
- `docker push`, `podman push`;
- fetching an address that never appeared in the conversation, after the session has read content from the web or an MCP server (such content can tell an agent to send data to an address it makes up).

A deny rule still refuses them. See [Permissions](./permissions#actions-that-always-ask).

## Safety check in bypass mode

In bypass mode nothing asks you, so {{NAME}} checks each action that is not a plain read with a separate model call before running it. The check sees the conversation, the action and your instruction files; an instruction that explicitly authorises that specific action counts, generic encouragement ("be autonomous") does not. Actions graded too risky are held back, and the agent is told to ask you or find another way, and to say what was held back. If the check cannot run, the action is held back too.

## Untrusted content

Output from tools is treated as data, not instructions. Invisible characters are removed and anything that imitates {{NAME}}'s own markers (`system-reminder`, `persisted-output`, `untrusted-content`) is escaped, so a web page or file cannot pose as a message from {{NAME}}. `WebFetch` refuses addresses on this computer or its private network (`localhost`, `127.0.0.1`, `10.x`, `192.168.x`, `169.254.169.254`, `*.local`, `*.internal`…).

## Other protections

- **Reading outside the project asks**, except in bypass mode or for [added folders](./permissions#additional-directories).
- **Plan mode** refuses every edit and command until you approve the plan.
- **Credentials** are sealed in `secrets.json`, encrypted with `secrets.key`, both readable only by you. See [Files and directories](./files-and-directories#data-folder).
- **Scripts** (JavaScript tools, skills, hooks and providers) can be denied files, processes or network with the [`scripts`](./settings#scripts) settings.
- **Project MCP servers** do not start until you approve them. See [MCP](./mcp).
