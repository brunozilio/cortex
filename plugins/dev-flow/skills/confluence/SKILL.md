---
name: confluence
description: How the development flow publishes and reads Confluence through the Atlassian MCP server - find existing documentation for a story, publish a spec page under the team's specs parent, update it after the build, record an ADR. Use when a flow stage reads or writes a Confluence page.
---

# Confluence in the flow

Confluence is reached through the same Atlassian MCP server as Jira
(`mcp__plugin_dev-flow_atlassian__<tool>`; load the schemas with
ToolSearch, `+atlassian confluence`, and trust them over this file).
`getAccessibleAtlassianResources` gives the `cloudId`.

## Reading before writing

During `refine` and `spec`, look for what the team already wrote:
`searchConfluenceUsingCql` with the story's terms inside the configured
space (`space = "ENG" AND text ~ "refund"`), then `getConfluencePage` for
the hits. Cite the pages you relied on in the spec.

## Publishing the spec

The spec is written first as `.flow/<ISSUE>/spec.md`; Confluence gets a
copy for the people who do not open the repository.

1. Find the space (`getConfluenceSpaces`, key from `config.json`
   `confluence.space`) and the parent page (`confluence.specsParent`).
2. Look for a page titled `<ISSUE> - <story title>`. Update it with
   `updateConfluencePage` when it exists; otherwise
   `createConfluencePage` under the parent.
3. Send the Markdown as it is (the server takes Markdown). Start the page
   with the issue link, the flow's stage and the date.
4. Record the page URL in the state file (`spec.confluence`) and in the
   Jira description.

Never overwrite a page someone else edited without reading its current
version: fetch it, merge their changes into `spec.md`, publish the merge.

## After the build

In `done`, update the page with what really shipped: decisions changed
during review, the final contracts, the PR and test-cycle links. A
decision that outlives the story (a new pattern, a chosen trade-off) is
an ADR: follow `dev-flow:documentation-and-adrs` and publish it under the
team's decisions page when `config.json` names one
(`confluence.decisionsParent`), otherwise in the repository's `docs/`.
