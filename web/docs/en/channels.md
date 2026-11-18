---
title: Channels
description: Keep MCP servers open as channels so messages from a chat, a webhook or another system reach the agent while you work.
---

A **channel** is an MCP server that pushes messages to {{NAME}}. Start the chat with `--channels` and {{NAME}} keeps those servers connected for the whole session: what they send (a message in a team chat, a webhook from CI, an alert) reaches the agent as an event, and the agent can answer through the same server's tools.

> [!EXPERIMENTAL]
> Channels use an experimental MCP capability. A server opts in by declaring it; the format below may change.

## Quick start

1. Configure the channel server like any MCP server, for example in `~/{{DIR}}/mcp.json`:

   ```json
   {
     "mcpServers": {
       "team-chat": {
         "command": "team-chat-mcp",
         "env": { "CHAT_TOKEN": "${CHAT_TOKEN}" }
       }
     }
   }
   ```

2. Start the chat with it as a channel:

   ```bash
   {{COMMAND}} --channels team-chat
   ```

   The chat confirms with *Channels open: team-chat.*

3. When someone writes in the chat room, the message reaches the agent. If the agent is idle, it is sent as a new message; if it is working, it is passed in between its steps.

Open several channels at once with a comma-separated list:

```bash
{{COMMAND}} --channels team-chat,ci-alerts
```

## Requirements

A channel must be:

- an MCP server configured for this folder and `ready` in `{{COMMAND}} mcp list` (a project server must be approved first; see [MCP servers](./mcp#approving-project-servers));
- a server that declares the channel capability (see below).

Any transport works: a local process, Streamable HTTP or HTTP + SSE.

If a channel cannot open, the chat says which and why, for example:

```text
Some channels did not open:
ci-alerts: no such MCP server here (or it waits for approval)
plain: plain is not a channel (it does not declare claude/channel)
```

A server that does not answer within 30 seconds is reported as *did not answer within 30 s*.

`--channels` applies to the interactive chat; it is not used with `--print`.

## How messages reach the agent

Each message reaches the agent as its own `channel` tag, with the message's `meta` values as attributes:

```text
<channel source="team-chat" chat_id="7" user="ana">
Can you deploy the fix to staging?
</channel>
```

- `source` is the server's name.
- Each `meta` value becomes an attribute; `&`, `"`, `<` and `>` in values are escaped.
- When channel messages arrive together with other events (a monitor, a background command, another session), the batch is introduced by a line telling the agent to treat the text as information, not as your instructions.
- While the chat is idle, it checks for new messages every second and sends them as a message; the transcript shows each one whole, as `inbound team-chat · ana: <text>` (the sender is the `user` meta value, when there is one).
- While a run is working, messages are passed to the agent between its steps.
- Up to 200 messages wait to be delivered; older ones are dropped beyond that. A message longer than 8,000 characters is cut.

> [!IMPORTANT]
> Channel messages reach the agent in every mode, `bypass` included. In `bypass` the agent runs without asking, so whoever can write to the channel can steer it: open a channel in that mode only when you trust everyone who can post to it. Messages from other sessions are still held in `bypass`.

## Answering through the channel

While a channel is open, the run uses that open connection for the server, so its tools (for example `mcp__team-chat__reply`) talk to the same process that delivered the message. The agent answers with them like any MCP tool, under the usual [permissions](./mcp#permissions-for-mcp-tools).

If you stop a run while a channel tool call is in flight, the call is cancelled at the server and the channel stays open.

The server's `instructions` (from its `initialize` result) are added to the agent's system prompt under *MCP server instructions*, cut to 2,048 characters. Use them to tell the agent that messages arrive as `<channel>` tags and that only the reply tool reaches the sender: what the agent writes in the transcript never does.

## Answering permission prompts from the channel

A channel can also answer the agent's permission prompts, so you can approve a call from your phone while the terminal waits. The server opts in by declaring a second capability, `claude/channel/permission`, beside `claude/channel`.

When a permission prompt opens, every open channel that declares it receives:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel/permission_request",
  "params": {
    "request_id": "kqzvn",
    "tool_name": "Bash",
    "description": "npm run deploy",
    "input_preview": "{\"command\":\"npm run deploy\"}"
  }
}
```

The server answers with:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel/permission",
  "params": { "request_id": "kqzvn", "behavior": "allow" }
}
```

- `request_id` is five lowercase letters (no `l`), easy to type back from a chat; case and surrounding spaces are ignored.
- `behavior` is `allow` or `deny`. *Always allow* is only offered in the terminal.
- The first answer wins, from the terminal or from any channel; the prompt then closes everywhere.
- A channel cannot allow a call to its own tools (`mcp__<server>__…`); such an allow is ignored and the prompt waits for another answer.
- An answer to a prompt that is no longer open is ignored. If two open prompts would get the same id, the second one is not sent to channels: answer it in the terminal.
- `input_preview` is the call's input as JSON, cut to 200 characters.

## Writing a channel server

A channel is a regular MCP server with two additions.

**1. Declare the capability** in the `initialize` result, under `experimental`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": {},
      "experimental": { "claude/channel": {} }
    },
    "serverInfo": { "name": "team-chat", "version": "1.0.0" }
  }
}
```

**2. Send a notification** for each message:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel",
  "params": {
    "content": "Can you deploy the fix to staging?",
    "meta": { "chat_id": "7", "user": "ana" }
  }
}
```

| Param | Meaning |
|---|---|
| `content` | The message's text. Required; an empty one is ignored. |
| `meta` | Optional key–value pairs, passed as attributes of the `channel` tag. A key must start with a letter or `_` and go on with letters, digits or `_`; other keys are dropped and named in the log. Values are cut to 200 characters, and control characters become spaces. |

How the notification travels depends on the transport:

- **Local process**: write it as a JSON line on stdout.
- **Streamable HTTP**: {{NAME}} opens a `GET` stream on the server's URL and reads notifications as server-sent events. If the stream drops, it reconnects, waiting from 1 second up to 60 seconds between tries.
- **HTTP + SSE**: send it on the SSE stream.

Offer tools such as `reply` so the agent can answer; use the `meta` values (a chat id, a thread) to let it say where.

## See also

- [MCP servers](./mcp) to configure and approve servers.
- [Permissions](./permissions) for modes such as `bypass`.
- [CLI reference](./cli-reference) for `--channels`.
