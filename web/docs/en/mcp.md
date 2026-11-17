---
title: MCP servers
description: Connect {{NAME}} to external tools and data through Model Context Protocol servers, over HTTP, SSE or a local process.
---

The [Model Context Protocol](https://modelcontextprotocol.io) (MCP) is an open standard for giving AI agents tools and data. Connect an MCP server to {{NAME}} and its tools become tools the agent can call: search your issue tracker, query a database, read a design file, post to chat.

{{NAME}} supports:

- remote servers over **Streamable HTTP** and the older **HTTP + SSE** transport, with OAuth sign-in;
- local servers started as a **process** that speaks over stdin and stdout;
- **JavaScript servers** run by {{NAME}}'s built-in script runtime;
- servers brought by [plugins](./plugins).

## Quick start

Add a remote server:

```bash
{{COMMAND}} mcp add tracker https://mcp.example.com/mcp
```

Add a local server started with `npx`, passing a token from your environment:

```bash
{{COMMAND}} mcp add -e GITHUB_TOKEN='${GITHUB_TOKEN}' github -- npx -y @modelcontextprotocol/server-github
```

Check what a session in this folder will use:

```bash
{{COMMAND}} mcp list
```

```text
tracker  user           ready                 https://mcp.example.com/mcp
github   user           ready                 npx -y @modelcontextprotocol/server-github
```

New servers are picked up on the next prompt. Ask the agent to use them ("find the open bugs assigned to me in tracker") or name a tool directly.

> [!TIP]
> You can also ask the agent to set a server up for you, for example "configure the Jira MCP". The built-in `configure` skill edits {{NAME}}'s own `mcp.json` and tells you what it wrote.

## Where servers are configured

| File | Scope | Starts |
|---|---|---|
| `~/{{DIR}}/mcp.json` | Your servers, every project | Right away |
| `{{DIR}}/mcp.json` in the project (the nearest one found in the working folder or a folder above it) | The project's servers, checked in for the team | Only after you approve each one |
| `~/{{DIR}}/mcp/*.js` | Your JavaScript servers | Right away |
| `{{DIR}}/mcp/*.js` in the working folder | The project's JavaScript servers | Only after you approve each one |
| An installed plugin's `.mcp.json` (or `mcpServers` in its manifest) | Wherever the plugin is installed | Right away |

> [!NOTE]
> {{NAME}} reads its own `mcp.json` files only. A `.mcp.json` at a project's root, or another app's configuration, is not read.

### File format

`mcp.json` holds an `mcpServers` object. A server with a `url` is remote; a server with a `command` is a local process:

```json
{
  "mcpServers": {
    "tracker": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer ${TRACKER_TOKEN}" }
    },
    "legacy": {
      "type": "sse",
      "url": "https://old.example.com/sse"
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "${GITHUB_TOKEN}" }
    },
    "sonar": {
      "type": "stdio",
      "command": "sonar-mcp",
      "env": { "SONAR_HOST": "${SONAR_HOST:-https://sonar.local}" }
    }
  }
}
```

| Field | Meaning |
|---|---|
| `type` | `http` (Streamable HTTP), `sse` (HTTP + SSE) or `stdio` (a local process). Optional: a server with only a `url` is `http`; one with a `command` is `stdio`. |
| `url` | The server's address (`http` and `sse`) |
| `headers` | Request headers (`http` and `sse`) |
| `command` | The program to start (`stdio`) |
| `args` | Its arguments (`stdio`) |
| `env` | Environment variables added to the process (`stdio`) |
| `sandbox` | Where the process may write and whether it reaches the network (`stdio`): `{ "writable": [...], "network": false }`, or `false` to start it outside the sandbox. `writable` and `false` count only in your own `~/{{DIR}}/mcp.json`. See [MCP servers in the sandbox](./sandbox-and-safety#mcp-servers-in-the-sandbox). |

A server with a value of the wrong type or outside its choices (another `type`, a `sandbox` of another shape, `args` that is not a list) is left out; the other servers in the file still start. A key {{NAME}} does not know is ignored. Both are named in `{{COMMAND}} mcp list` with the file and the key's path, for example `warning: /home/me/{{DIR}}/mcp.json: mcpServers.docs.type: expected one of "stdio", "http", "sse", found "websocket"`.

The file may name its JSON Schema, so an editor can complete keys and underline mistakes; {{NAME}} ignores the key. The schema is copied to `~/{{DIR}}/schemas/mcp.schema.json`, so your own file can use a relative path, and a project's file the full path to your copy:

```json
{
  "$schema": "./schemas/mcp.schema.json",
  "mcpServers": {}
}
```

See [JSON Schema](./settings#json-schema) for both schemas.

### Environment variables in the file

In `~/{{DIR}}/mcp.json` and the project's `{{DIR}}/mcp.json`, any string can use:

| Syntax | Value |
|---|---|
| `${NAME}` | The variable `NAME` from {{NAME}}'s environment when the server starts |
| `${NAME:-default}` | `NAME`, or `default` when it is not set |

A variable that is not set and has no default is left as written. Use this for every secret: never write a token into the file.

### Server names

A server name may use letters, digits, `-` and `_`. Names starting with `{{COMMAND}}_` or `plugin_` are reserved: a configured server with such a name never starts.

When the user's and the project's files declare the same name, an approved project server wins; while the project's is not approved, yours is used.

## The `{{COMMAND}} mcp` command

| Command | What it does |
|---|---|
| `{{COMMAND}} mcp` or `{{COMMAND}} mcp list` | Lists the servers, where each comes from and whether it starts |
| `{{COMMAND}} mcp get <name>` | Shows one server: its origin, state, target, file and stored entry, with secrets hidden |
| `{{COMMAND}} mcp add <name> <url-or-command> [-- args…]` | Adds one of your servers to `~/{{DIR}}/mcp.json` |
| `{{COMMAND}} mcp remove <name>` | Removes one of your servers |
| `{{COMMAND}} mcp approve <name>` | Lets a project server start |
| `{{COMMAND}} mcp deny <name>` | Turns a project server off |
| `{{COMMAND}} mcp login <name>` | Signs in to a remote server in the browser (OAuth) |
| `{{COMMAND}} mcp logout <name>` | Forgets the sign-in to a remote server |

`add` takes these options, anywhere before `--`:

| Option | Meaning |
|---|---|
| `-e`, `--env KEY=VALUE` | An environment variable for a local server (repeatable). Quote `'${VAR}'` so the shell leaves it for {{NAME}} to read at start. |
| `-H`, `--header "Name: value"` | A header for a remote server (repeatable). Quote `'${VAR}'` the same way. |
| `--transport sse` | Use the older HTTP + SSE transport for a URL (`http` by default) |
| `--force` | Replace a server of yours that already has this name |

A target starting with `https://` or `http://` makes a remote server; anything else is the command of a local server, and what follows it are its arguments. Write `--` before the server's own arguments when any of them starts with `-`: everything after `--` belongs to the server. An argument starting with `-` without a `--` before it is refused with a hint to add one.

```bash
{{COMMAND}} mcp add -H 'Authorization: Bearer ${API_TOKEN}' docs https://docs.example.com/mcp
{{COMMAND}} mcp add --transport sse legacy https://old.example.com/sse
{{COMMAND}} mcp add jira jira-mcp -- --stdio
{{COMMAND}} mcp add github -- npx -y @modelcontextprotocol/server-github
```

`add` refuses a name with other characters, a reserved name, a server that has neither a command nor a URL, or a name you already use (unless `--force` is given; the error names the existing entry). It also refuses a secret written out: a header, environment value or argument that looks like a token, or a header or environment value whose name says it holds one (`Authorization`, `GITHUB_TOKEN`, `API_KEY`…), unless the value reads a `${VAR}`. `remove` only removes servers from your own `~/{{DIR}}/mcp.json`. `approve`, `deny` and `get` refuse a name that is not listed, and say which names are.

### The list

Each line shows the name, the origin, the state and the target (URL or command line). Headers and environment values are never shown.

| Origin | Meaning |
|---|---|
| `user` | `~/{{DIR}}/mcp.json` or `~/{{DIR}}/mcp/` |
| `project` | The project's `{{DIR}}/mcp.json` or `{{DIR}}/mcp/` |
| `plugin <name>` | An installed plugin |
| `host` | Given by an app that embeds {{NAME}} |

| State | Meaning |
|---|---|
| `ready` | Starts with the next prompt |
| `waiting for approval` | A project server you have not approved |
| `off` | A project server you turned off |
| `skipped: folder not trusted` | A project server in a folder you have not trusted: it is listed, but nothing of the project starts |

A configuration file that is not valid JSON is named with its error. When it is your own `~/{{DIR}}/mcp.json`, `{{COMMAND}} mcp list` fails (exit code 1); a project's broken `{{DIR}}/mcp.json` is a `warning:` line and the list goes on. A value left out of either file (see [File format](#file-format)) is also a `warning:` line.

## In the chat: `/mcp`

| Command | What it does |
|---|---|
| `/mcp` | Lists the servers, like `{{COMMAND}} mcp list` |
| `/mcp approve <name>` | Approves a project server |
| `/mcp deny <name>` | Turns a project server off |
| `/mcp login <name>` | Signs in to a remote server in the browser |
| `/mcp logout <name>` | Forgets that sign-in |

A change applies from the next message. `/reload` re-reads `~/{{DIR}}` and the project and counts the servers that are ready, waiting for approval or off.

## Approving project servers

A server in a project's checked-in `{{DIR}}/mcp.json` runs a program or calls a URL on your behalf, so two things must be true before it starts:

1. You trust the project's folder. Until then its servers are not started; `{{COMMAND}} mcp list` shows them as `skipped: folder not trusted`, and `{{COMMAND}} doctor` counts them on its trust line. See [Workspace trust](./sandbox-and-safety#workspace-trust).
2. You approve that server. Trusting the folder does not approve its servers.

Approvals live in your own settings, never in the project's shared file:

- `{{COMMAND}} mcp approve <name>` (or `/mcp approve <name>`) adds the name to `enabledMcpServers` in the `settings.local.json` next to the project's `mcp.json`, and removes it from `disabledMcpServers`.
- `{{COMMAND}} mcp deny <name>` does the opposite.

You can also edit the keys yourself:

```json
{
  "enabledMcpServers": ["jira"],
  "disabledMcpServers": ["sonar"],
  "enableAllProjectMcpServers": false
}
```

| Key | Meaning |
|---|---|
| `enabledMcpServers` | Project servers allowed to start |
| `disabledMcpServers` | Project servers turned off; wins over the other two |
| `enableAllProjectMcpServers` | `true` approves every project server |

These keys count only in your own files: `~/{{DIR}}/settings.json`, `~/{{DIR}}/managed-settings.json`, and the `settings.local.json` of a project whose folder you trust. In a project's checked-in `{{DIR}}/settings.json` they are ignored.

## Signing in with OAuth

Remote servers that use OAuth need a sign-in:

```bash
{{COMMAND}} mcp login tracker
```

{{NAME}} opens the server's sign-in page in your browser and waits for the answer on a local address. It:

1. finds the server's authorization server from its OAuth metadata;
2. registers itself as a client (the server must allow dynamic client registration);
3. signs in with PKCE;
4. keeps the tokens sealed in its secrets store, and refreshes them when they expire.

From then on, every request to that server's URL carries `Authorization: Bearer <token>`, unless the server's configuration already sets an `Authorization` header. Only `https` addresses, or `http` to this computer, are accepted for sign-in. A local (`stdio`) server cannot sign in. `{{COMMAND}} mcp logout <name>` forgets the tokens.

## Tools, prompts and resources

### Tool names

Each tool a server offers becomes a tool named:

```text
mcp__<server>__<tool>
```

For example `mcp__github__create_issue`. A plugin's server is named `plugin_<plugin>_<server>`, so its tools look like `mcp__plugin_acme_jira__search`.

When more than 20 MCP tools are available, they are not all sent to the model up front: the agent gets a `ToolSearch` tool and loads the MCP tools it needs by searching for them.

### Prompts

A server that offers prompts can be used as a command in the chat:

```text
/mcp__<server>__<prompt> arg1 arg2 the rest
```

Words fill the prompt's arguments in order; the last argument gets the rest of the line. The prompt's text is sent as your message.

### Resources

When a server offers resources, the agent gets two more tools: `ListMcpResources` to list them and `ReadMcpResource` to read one.

### Results

Text, images, embedded text resources and resource links in a tool's result reach the agent. Structured content is added as JSON when the text does not already contain it.

## Permissions for MCP tools

MCP tools count as actions. In `ask` and `edits` modes the agent asks before calling one, unless a rule allows it; in `bypass` mode they run. A rule can name a whole server or one tool:

```json
{
  "permissions": {
    "allow": ["mcp__jira"],
    "deny": ["mcp__github__delete_repo"]
  }
}
```

| Rule | Matches |
|---|---|
| `mcp__jira` | Every tool of the `jira` server (not of a server called `jiraffe`) |
| `mcp__github__delete_repo` | That one tool |

See [Permissions](./permissions) for how rules combine. A custom agent can be limited to some servers with `mcpServers` in its frontmatter (see [Agents](./agents)), and a hook can check MCP calls with a matcher such as `^mcp__github__` (see [Hooks](./hooks)).

## JavaScript servers

A `.js` file in `~/{{DIR}}/mcp/` (or in the working folder's `{{DIR}}/mcp/`) is a server named after the file. Its default export lists the tools:

```javascript
export default {
  capabilities: ["readFile"],
  tools: {
    add: {
      description: "Adds a and b.",
      input: { type: "object", properties: { a: { type: "number" }, b: { type: "number" } } },
      run: (input) => String(input.a + input.b),
    },
    notes: {
      description: "Reads the project's notes.",
      run: (input, ctx) => ctx.readFile("NOTES.md"),
    },
  },
};
```

`math.js` with this content offers `mcp__math__add` and `mcp__math__notes`. A tool returns text (or a value, sent as JSON); a thrown error is a failed call. The server lists in `capabilities` each host function its tools call (`readFile`, `writeFile`, `exec`, `fetch`); a call it did not list fails. Each listed call is judged by your permission rules and mode like the matching tool (`ctx.readFile` like `Read`, `ctx.exec` like `Bash`), and the `scripts` setting stays the outer limit (see [Capabilities](./sandbox-and-safety#capabilities) and [Settings](./settings#scripts)).

> [!WARNING]
> Breaking change: a JavaScript server that calls `ctx.readFile`, `ctx.writeFile`, `ctx.exec` or `ctx.fetch` without listing it in `capabilities` now fails with an error saying what to add.

## How servers start

At a session's first prompt, {{NAME}} starts every `ready` server (MCP protocol version `2025-06-18`). The servers stay connected for the session's later prompts; each prompt asks them for their tools again. A server is started again only when its entry or its approval changed, or when its process has stopped; a server that is no longer `ready` is closed. When {{NAME}} lets the session go (see [`SessionEnd`](./hooks#sessionend)), its servers are closed. JavaScript servers are loaded again at every prompt.

A server that fails to start, or to list its tools, is left out of that run with a warning in the log; the run goes on without it.

- A local server runs as a child process with your environment plus its `env`, inside the [sandbox](./sandbox-and-safety#mcp-servers-in-the-sandbox) when it is on. When it stops, the error quotes the last lines it wrote to stderr. A call may take up to 10 minutes.
- A remote server keeps the session id it receives across its calls.

## Troubleshooting

| Symptom | What to check |
|---|---|
| A project server never starts | `{{COMMAND}} mcp list` shows it `waiting for approval`: run `{{COMMAND}} mcp approve <name>`. |
| A server is listed but its tools are missing | It failed to start or list its tools. Run the command by hand, check the URL, and look at the log (see [Troubleshooting](./troubleshooting)). |
| `${TOKEN}` reaches the server as written | The variable is not set in the environment you started {{NAME}} from. |
| `{{COMMAND}} mcp login` says no OAuth metadata, or no registration | The server does not support OAuth sign-in with dynamic registration. Use a header with a token instead. |
| A server named `{{COMMAND}}_…` or `plugin_…` is ignored | These prefixes are reserved. Rename it. |

## See also

- [Channels](./channels) to let an MCP server push messages to the agent.
- [Plugins](./plugins) to share servers.
- [Permissions](./permissions).
