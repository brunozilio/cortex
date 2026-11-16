# dev-flow

The team's development flow as a Cortex plugin: one Jira story carried
from refinement to done, with an approval at every stage. Nothing here
is compiled into the app; it is skills, commands, agents, two JavaScript
hooks and one MCP server.

```
(story) -> refine -> spec -> plan -> implement -> pr -> review -> merge -> qa -> done
```

| Stage | What happens | Tools |
|---|---|---|
| story | an idea becomes a Jira story (asks before creating) | Jira |
| refine | questions answered, testable acceptance criteria, repositories named | Jira, Confluence |
| spec | spec with Gherkin scenarios, published and linked | Confluence, Jira |
| plan | ordered tasks per repository, scenarios registered | Zephyr, Jira |
| implement | test-first, task by task; Cucumber and Cypress scenarios automated | git, the stack's tools |
| pr | pull requests opened, checks carried to green | GitHub |
| review | reviewer agents and people's reviews, findings fixed | GitHub |
| merge | approved pull requests merged in dependency order | GitHub |
| qa | scenarios run in QA, executions recorded | Zephyr, Cypress, Cucumber |
| done | issue closed, spec updated, decisions recorded | Jira, Confluence |

Stacks covered: Java services, Python services, Terraform on AWS and
Angular portals (`skills/stack/`). Sonar, Veracode and Datadog are not
part of the flow yet.

## Install

```sh
ln -s "$PWD/plugins/dev-flow" ~/.cortex/plugins/dev-flow
```

(or copy the folder there; `cortex plugin install <git-url>` once it has
its own repository). Then, in the chat:

1. `/reload` lists the plugin, its skills and its MCP server.
2. `/mcp login plugin_dev-flow_atlassian` signs in to Jira and Confluence.
3. `gh auth login` in a terminal, if `gh auth status` fails.
4. `export ZEPHYR_TOKEN=...` in the shell that starts the app (a Zephyr
   Scale API token from your Jira profile).
5. `/dev-flow:setup` writes `.flow/config.json` for the folder and checks
   every access.

## Use

| Command | Does |
|---|---|
| `/dev-flow:start PAY-123` | runs the flow from the story's stage to done |
| `/dev-flow:start <idea>` | drafts the story, creates it, then runs the flow |
| `/dev-flow:story <idea>` | only drafts and creates the story |
| `/dev-flow:refine`, `spec`, `plan`, `implement`, `pr`, `review`, `merge`, `qa`, `done` `PAY-123` | works on one stage |
| `/dev-flow:status` | where each story is and what its next approval needs |

A story's state is `.flow/<ISSUE>.json`, its spec, plan and QA report
are in `.flow/<ISSUE>/`. A new session is told which flows are open
(`hooks/resume.js`), so a story goes on across sessions and people.

## Approvals

Entering a stage is one write of the state file with the new `stage`.
`hooks/gate.js` runs before every `Write`, `Edit`, `Bash` and MCP call
and:

- turns a stage change into a permission prompt showing the file, with
  the sentence in `approval` saying what is being approved (this asks
  even in modes that otherwise allow edits);
- refuses a skipped stage, and asks again when a story goes back;
- refuses a stage's outside actions before that stage was approved:
  `gh pr create` before `pr`, `gh pr merge` before `merge`, a Jira
  transition before `implement`, a write to Zephyr before `plan`;
- always asks before `terraform apply`/`destroy` and before creating a
  Jira issue.

It is a guard against drifting past an approval, not a security
boundary: it reads the state file the agent can write.

### Customising in JavaScript

The top of `hooks/gate.js` is the configuration:

```js
export const gates = {
  refine: "allow",
  spec: "ask",
  plan: ({ state }) => (state.repos.length > 1 ? "ask" : "allow"),
  merge: ({ state, ctx }) =>
    state.repos.every((repo) => ctx.exec(`gh pr checks ${repo.pr}`).code === 0) ? "ask" : "deny",
  ...
};
```

- `stages`: the order. Add or remove a stage here and in
  `skills/flow/SKILL.md`.
- `gates`: per stage, `"ask"`, `"allow"`, `"deny"`, or a function of
  `{ issue, from, to, state, previous, ctx }` returning one of them
  (anything else asks). `ctx` reads files, runs commands and fetches
  URLs, within the `scripts` access of your settings.
- `guards`: outside actions tied to a stage (`stage`) or to a fixed
  decision (`decision`), matched by tool name and, for `Bash`, by
  command. This is where Sonar, Veracode or Datadog checks would go.

`npm test` in this folder runs the hooks' tests (Node 20 or later).

## Credits

The engineering skills (`spec-driven-development`,
`planning-and-task-breakdown`, `incremental-implementation`,
`test-driven-development`, `code-review-and-quality` and others), the
`code-reviewer` and `test-engineer` agents and the reference checklists
come from [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills),
Copyright (c) 2025 Addy Osmani, MIT License. See `NOTICE.md` for the list
and `LICENSES/agent-skills-MIT.txt` for the license.
