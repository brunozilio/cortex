# Plugins and MCP servers

The app is a platform (ADR 0019): a team's integrations and workflows live
in plugins, kept in their own repositories, in the app's plugin format.

## A plugin

```
my-plugin/
  .{command}-plugin/plugin.json {"name": "my-plugin", ...; may hold "mcpServers"}
  skills/<name>/SKILL.md       offered as my-plugin:<name>
  commands/<name>.md           /my-plugin:<name>
  agents/<name>.md             my-plugin:<name>
  hooks/hooks.json             {"hooks": {"PreToolUse": [...]}}, joined to the settings' hooks
  .mcp.json                    {"mcpServers": {...}}, started as plugin_my-plugin_<server>
```

`${{COMMAND}_PLUGIN_ROOT}` in hooks and MCP servers is the plugin's folder. A
plugin whose hooks or servers do not parse is left out; the run goes on.
To install one, put its folder in `~/.{command}/plugins/` (a `git clone`
you `git pull` to update) and type `/reload` in the chat: it lists the
plugins, skills, agents and MCP servers found. Every message rereads them
anyway. Plugins also come from the backend (`Extensions::plugins`); on a
clash of names the backend's wins over `~/.{command}/plugins/`.

```sh
git clone git@github.com:you/my-config.git ~/.{command}/plugins/my-config
```

## MCP servers a person or a project configures

| Where | File | Starts |
|---|---|---|
| Project | `.{command}/mcp.json`, the nearest up from the run's folder (never the home folder's) | only once the owner approved it |
| User | `~/.{command}/mcp.json` | always |

On a clash of names the host's wins, then the project's, then the user's.
Names starting `{command}_` or `plugin_` are the platform's and are left
out. Strings may use `${VAR}` and `${VAR:-default}` from the environment,
so tokens stay out of the file:

```json
{"mcpServers": {
  "sonarqube": {"command": "docker", "args": ["run", "-i", "--rm", "-e", "SONARQUBE_TOKEN", "mcp/sonarqube"],
                "env": {"SONARQUBE_TOKEN": "${SONAR_TOKEN}", "SONARQUBE_URL": "${SONAR_URL:-https://sonarcloud.io}"}},
  "atlassian": {"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"}
}}
```

Approvals are read only from the user's `~/.{command}/settings.json` and
the project's `.{command}/settings.local.json`, never from a checked-in
`settings.json`, so a repository cannot approve itself:

```json
{"enabledMcpServers": ["sonarqube"], "disabledMcpServers": [], "enableAllProjectMcpServers": false}
```

## Managing them

```sh
{command} mcp                     # list: origin, ready / waiting for approval / off, target
{command} mcp approve jira        # let a project's server start (its .{command}/settings.local.json)
{command} mcp deny jira           # turn it off
{command} mcp add atlassian https://mcp.atlassian.com/v1/mcp -H 'Authorization: Bearer ${ATLASSIAN_TOKEN}'
{command} mcp add sonarqube npx -y sonarqube-mcp-server -e SONAR_TOKEN='${SONAR_TOKEN}'
{command} mcp remove atlassian    # your own servers live in ~/.{command}/mcp.json (0600)
```

In the chat, `/mcp`, `/mcp approve <name>` and `/mcp deny <name>`; a
change applies from the next message. Programs use
`Sdk::mcp_servers`, `approve_mcp_server`, `add_mcp_server` and
`remove_mcp_server` (`ClientCommand::ListMcpServers` and friends).
