# Threat model

Status: living; updated as phase 6 lands. This is the skeleton from the 2026-11-16 audit: the gaps below are known and mostly open. Phase 6.4 of the plan turns it into the full model.

The product is written as "the app", its binary as `<command>`, its project folder as `.<command>/`.

## What the app is, for this purpose

A local process that runs with the user's privileges, takes instructions from a language model, and on those instructions reads and writes files, runs shell commands and makes network requests. It is single-user and local (`decisions/0008-single-user-local-product.md`). There is no sandbox between the agent and the user's account except where stated.

## Assets

| Asset | Where | Why it matters |
|---|---|---|
| Credentials vault | `secrets.json` and `secrets.key` in the data folder; provider keys and tokens, MCP OAuth tokens | Gives access to the user's model accounts and to MCP servers |
| User files | The project, and everything else the user can read or write (home, dotfiles, SSH and cloud credentials, other repositories) | Confidentiality and integrity of the user's work and machine |
| Shell | The ability to run commands as the user | Equivalent to the user's whole account |
| Model accounts | The provider accounts the app is connected to | Cost, quota, and the standing of the account with the provider |
| Session data | Transcripts, trace, memory, checkpoints in the data folder | May contain source code, secrets pasted or printed, and prompts |
| The local web page | A loopback HTTP server with a per-process token | Shows session data and can attach a terminal to a session |

## Actors

| Actor | Trust | What they control |
|---|---|---|
| The user | Trusted | The terminal, the settings in the user folder, answers to questions |
| The model under prompt injection | Untrusted | Every tool call and its arguments. Anything the model read (a file, a web page, a tool result) may be steering it |
| A hostile repository | Untrusted until the user trusts the folder | Files in the project: `.<command>/settings.json`, `settings.local.json`, hooks, script tools, rules, instruction files, `mcp.json`, declared providers, language-server configuration |
| A hostile web page or MCP server | Untrusted | The content returned by web fetch and search, MCP tool results, MCP server metadata and OAuth endpoints, redirects |
| Another local user or process | Untrusted | Other ports on loopback, the shared temporary directory, a browser on the same machine. A process running as the same user can read the vault key and is out of scope until the keychain lands |

## Trust boundaries

1. User folder vs project folder. Settings in the user's folder may grant (allow rules, extra directories, MCP approval, script access, feature flags). A project's versioned settings may only restrict. Deny always wins. The boundary is drawn by workspace trust (`decisions/0004-workspace-trust.md`): until the user trusts a folder, nothing in it runs code or grants anything, `settings.local.json` included; the answer is kept in the data folder and reaches the engine from the handler, never from the client's request. A trusted folder is trusted whole, and stays trusted when its content changes.
2. Model vs tools. Every tool call passes a permission decision: mode, rules, hook decision, forced questions for risky commands, and in bypass a classifier that fails closed. Today the decision is driven by a string per tool and by text matching on shell commands (SEG-1, SEG-4, SEG-7, SEG-8).
3. Project boundary on the file system. Reading tools ask before reaching outside the project and added directories. Writing tools have no such check today (SEG-4).
4. External content vs instructions. Fetched web content is fenced and read by a separate model before the main model sees it; inbox notices are marked as data. MCP results are not fenced yet.
5. Scripts vs the host. JavaScript hooks, tools and providers run in an embedded runtime whose only way out is a host bridge. Today the bridge grants files, processes and network by default (SEG-5).
6. Process vs operating system. An optional write sandbox for shell commands exists on macOS only (SEG-9).
7. Browser vs the local web page. Loopback only, a random token exchanged for an `HttpOnly`, `SameSite=Strict` cookie, an origin check, and the API requires the cookie (SEG-12).
8. Secrets at rest. Entries are encrypted with XChaCha20-Poly1305 and bound to their entry name; the key sits in a file beside the vault, both readable only by the user (SEG-10).

Controls the audit confirmed as sound and that later work must not weaken: deny rules always win; untrusted layers can only restrict; the bypass classifier fails closed; secrets are kept out of debug output; language-server downloads are checked against a pinned SHA-256; the console is loopback-only with token and origin check; OAuth uses PKCE S256 and `state`; no shell command is built by concatenating configuration data.

## Known gaps

From the audit, found by reading code; none was reproduced by running an exploit. Each must get a failing test before its fix. "Closed by" names the phase of the restructuring plan.

| ID | Sev. | Gap | Actor | Closed by |
|---|---|---|---|---|
| SEG-1 | high | "Read-only" Bash runs without a question in every mode, including plan, and the classifier can be bypassed: environment assignments and `sudo` are dropped before analysis, flags that write or execute are not filtered, and "inside the project" is a text test | Model; repository | Phase 1 stopgap (no auto-approval); phase 6.2 (shell parser, program-and-flags list, opt-in) |
| SEG-2 | high | Hooks and scripts from a repository run with no trust step: `hooks` is accepted from the project's versioned settings and script hooks in every ancestor's project folder are registered automatically | Repository | Closed in phase 6.1 (workspace trust): project hooks, hook scripts, script tools, script skills, MCP servers and declared providers load only in a trusted folder, and never from a folder above the project root. Residual: a trusted repository that later pulls a hostile change runs it; trust is not re-asked when content changes |
| SEG-3 | high | `settings.local.json` counts as the owner's by name only; a repository that commits one gains allow rules, MCP approval, extra directories and language-server commands | Repository | Closed in phase 6.1: in an untrusted folder the file is read like the versioned one and can only restrict. Residual: in a trusted folder a committed `settings.local.json` is still the owner's; and the model can write the file (SEG-4 asks before it does) |
| SEG-4 | high | In the default mode `Write` and `Edit` write to any path without a question; the outside-project check exists only for reads, and editing tools do not go through the sandbox. Combines with SEG-2 and SEG-3 to keep privilege | Model | Phase 1 (outside-project check for writing tools; always ask for `.git/` and the project folder); phase 6.2 (write effects) |
| SEG-5 | high | Scripts have disk, processes and network by default; `ctx.exec` is not confined the way command hooks are; the script runtime has no memory limit | Repository; model | Phase 1 (confine `ctx.exec`, restrictive default outside the user folder); phase 6.3 (declared capabilities, memory and time limits) |
| SEG-6 | high | SSRF in web fetch: the private-host check looks only at the text of the input URL, redirects are followed, the resolved address is never validated | Web page; model | Phase 1 |
| SEG-7 | high | The secrets block is escaped by a wildcard (`cat .env*`) or by any program outside its list | Model | Phase 6.2 |
| SEG-8 | medium | Deny rules can be stepped around by wrappers, quoting or a variable; in bypass those commands run | Model | Phase 6.2 |
| SEG-9 | medium | The sandbox exists only on macOS and confines only writes; on Linux its configuration is accepted and ignored. Bash, MCP servers and language servers inherit the whole environment | Model; MCP server | Phase 6.3 (Landlock on Linux, explicit error where unsupported). Environment inheritance has no phase yet |
| SEG-10 | medium | The vault key sits beside the encrypted file with no keychain. Any error reading the key generates a new one over it, making the vault unreadable. A failure to open falls back to memory silently, so credentials vanish on exit | Local process; none (data loss) | Phase 1 (distinguish "not found", visible warning on fallback); phase 5.5 (OS keychain) |
| SEG-11 | medium | MCP OAuth: a prefix test on the host accepts a non-loopback name as loopback; authorisation requests every advertised scope | MCP server | Phase 1 |
| SEG-12 | medium | Console: the cookie name carries no port, so two instances invalidate each other and the token is sent to other loopback services; on an authorisation error the page shows sample data as real; `POST /api/prompt` starts runs and no page code uses it | Local process; web page | Phase 1 (cookie per port, `Host` validation, error screen, route removed) |
| SEG-13 | medium | A language server is installed without consent: the first `LSP` call, a reading tool, can run `npm install` without `--ignore-scripts` or `go install` | Model; repository | Phase 6.3 |

## Not yet covered

To be written as phase 6 lands:

- The web terminal (a WebSocket to a pseudo-terminal running the app's own binary): its security review is a precondition of phase 0.1.
- MCP tool results as untrusted content (phase 6.3).
- Tool input validation against each tool's schema before it runs (phase 6.3).
- Files in the shared temporary directory (per-run MCP configuration, memory edits, large tool results), some of which are not private today.
- Plugins: installed by `git clone` into the user folder and loaded as user-level content.
- Supply chain of the build (`cargo deny` is phase 10).

## Out of scope here

The subscription providers' client identity and the license are open decisions owned by the tech lead and are not analysed in this document.
