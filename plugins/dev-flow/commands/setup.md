---
description: Set the development flow up in this folder - write .flow/config.json and check Jira, Confluence, GitHub and Zephyr access
---

Set the development flow up for this folder. $ARGUMENTS

1. If `.flow/config.json` exists, read it and only fill what is missing.
   Otherwise start from `config.example.json` in the plugin's folder (two
   levels above the `dev-flow:flow` skill's base directory).
2. Find what you can before asking: the repositories here (`git` folders
   at the root or one level down) and each one's stack from its root
   files (`dev-flow:stack`), the default branch (`gh repo view --json
   defaultBranchRef`), the Jira project from recent branch names.
3. Ask the person for the rest in one round: Jira site and project, the
   status names their board uses for refine, implement, review, qa and
   done, the Confluence space and the specs parent page, the Zephyr
   project, the language for what people read.
4. Write `.flow/config.json`.
5. Check access and report each one plainly, without printing a secret:
   - Atlassian: `getAccessibleAtlassianResources` answers (otherwise
     `/mcp login plugin_dev-flow_atlassian`);
   - Jira: the project is readable and the configured status names exist
     among its statuses;
   - GitHub: `gh auth status`;
   - Zephyr: the token variable is set and
     `GET /v2/testcases?projectKey=<project>&maxResults=1` answers 200.
6. Say whether `.flow/` should be committed: `config.json` usually is,
   so the team shares it; the per-story files are the team's choice.
