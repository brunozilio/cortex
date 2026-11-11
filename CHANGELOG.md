# Changelog

## [0.7.0](https://github.com/brunozilio/lite/compare/v0.6.0...v0.7.0) (2026-11-11)


### Features

* static musl Linux builds and an install script ([#9](https://github.com/brunozilio/lite/issues/9)) ([d57fedf](https://github.com/brunozilio/lite/commit/d57fedf2f580bf66645c60c60e690bf37323aabf))

## [0.6.0](https://github.com/brunozilio/lite/compare/v0.5.0...v0.6.0) (2026-11-11)


### Features

* event inbox, streaming Monitor, MCP channels, messages between sessions and PushNotification ([#6](https://github.com/brunozilio/lite/issues/6)) ([fdc2ffe](https://github.com/brunozilio/lite/commit/fdc2ffec48947e91ff29632da14faaf132b2e50c))
* MCP channels over streamable HTTP, and cancelling a channel call without ending it ([#8](https://github.com/brunozilio/lite/issues/8)) ([e9acb7d](https://github.com/brunozilio/lite/commit/e9acb7d7e764e0f9cf983644d40cb47a778fe500))

## [0.5.0](https://github.com/brunozilio/lite/compare/v0.4.0...v0.5.0) (2026-11-11)


### Features

* Claude Code parity: steering, permission rules, worktrees, schedules, MCP OAuth and SSE, inline chat ([#4](https://github.com/brunozilio/lite/issues/4)) ([660a674](https://github.com/brunozilio/lite/commit/660a674a2021ac090ecbd00d3429094253765669))

## [0.4.0](https://github.com/brunozilio/lite/compare/v0.3.1...v0.4.0) (2026-11-11)


### Features

* **daemon:** leave 5 s after the last session closes instead of after 30 idle minutes ([e416da5](https://github.com/brunozilio/lite/commit/e416da588328a169295f370f3003d6cf0e015c03))
* ExitPlanMode and EnterPlanMode — the agent puts its plan to the user in a box, approval picks the mode and implementation starts in the same run ([e8ee980](https://github.com/brunozilio/lite/commit/e8ee980d4af4cbf343850be32c0ec68e1e0e4104))
* messages typed while the agent works reach it mid-run (SteerRun) ([5ce3567](https://github.com/brunozilio/lite/commit/5ce3567fbf96bd9a634bd7a768476c0960d49630))
* superfik update — replace the binary with the latest GitHub release, checked against its SHA-256 ([c2ef33d](https://github.com/brunozilio/lite/commit/c2ef33db5b70ac369086abcd4ce4f23182f213db))


### Bug Fixes

* **tui:** the / suggestions scroll through every match, and ↑ from the first wraps to the last ([4542285](https://github.com/brunozilio/lite/commit/45422857a1a8100f91ba4eaa240735ca8c4b7e5f))

## [0.3.1](https://github.com/brunozilio/lite/compare/v0.3.0...v0.3.1) (2026-11-10)


### Bug Fixes

* **lsp:** a checked-in project setting never says what a language server runs ([e9746e8](https://github.com/brunozilio/lite/commit/e9746e85ca19ea71e16c89312a15b4297c9f91ed))

## [0.3.0](https://github.com/brunozilio/lite/compare/v0.2.0...v0.3.0) (2026-11-10)


### ⚠ BREAKING CHANGES

* read only Superfik's folders; nothing from .claude, ~/.claude or CLAUDE.md
* Superfik's own plugin format (.superfik-plugin/, ${SUPERFIK_PLUGIN_ROOT}); Claude Code plugins are not read

### Features

* /rewind and /retry in the chat, Session::rewind in the SDK ([76503e6](https://github.com/brunozilio/lite/commit/76503e6a5e6e42d3896a1e158bc2a2b8f68b1103))
* /verification, a judge agent that grades a run against criteria ([c1c0d90](https://github.com/brunozilio/lite/commit/c1c0d90ec1ff1a4c87a7b4927c559c5d2f3d996e))
* background commands (Bash run_in_background, BashOutput, KillShell) ([821d786](https://github.com/brunozilio/lite/commit/821d7860eda754dce47d31cca3ef5721ee3b6411))
* **cli:** /btw asks a side question without interrupting the agent ([3f4085b](https://github.com/brunozilio/lite/commit/3f4085bb21f411210e25342d873d2f543395b086))
* **cli:** /diffs browses the changes full screen ([18e8ba0](https://github.com/brunozilio/lite/commit/18e8ba0d6a6eaa09ab8005e4dc0741806c43542d))
* **cli:** `superfik`, the terminal chat over the SDK ([ac11269](https://github.com/brunozilio/lite/commit/ac11269fb002836cf8850a31d7a2bc4abe714aa3))
* **cli:** diffs, word jumps, prompt history across chats, a calmer look ([c8a9761](https://github.com/brunozilio/lite/commit/c8a9761e72063823bb096a562bbd19504f2deb92))
* **cli:** show what the agent is doing, Claude Code style ([4e1a648](https://github.com/brunozilio/lite/commit/4e1a6481bbae187f7d4d73de1e8f540f1b680426))
* **cli:** the chat's side calls: a title, the next prompt, heads-ups, a recap ([767e9b6](https://github.com/brunozilio/lite/commit/767e9b676580bf38d32f0f175bc891e9e87ba292))
* host extensions for the SDK: local folders, MCP servers, in-process tools ([d0107c2](https://github.com/brunozilio/lite/commit/d0107c214c844ce84c2f3135569e1be8d6b9186f))
* Kiro and GitHub Copilot subscriptions; project and user MCP servers ([004476b](https://github.com/brunozilio/lite/commit/004476b42410df1fb99bf2b27006e9bf90ebc36f))
* **lsp:** an edit's result carries the language server's errors and warnings ([75bad3d](https://github.com/brunozilio/lite/commit/75bad3d56f61f1cb20c2ec729d50b5ada76c45a7))
* **lsp:** language server client and a pool that keeps servers between runs ([2b960d7](https://github.com/brunozilio/lite/commit/2b960d7f7311d3695c1909c7e950c2b5455f2e79))
* **lsp:** language server rules in core: servers, roots, queries, answers, diagnostics ([dcf3bea](https://github.com/brunozilio/lite/commit/dcf3bea6d981dc442caba3e2fdbedfb64d32641c))
* **lsp:** lspServers in settings and a plugin's .lsp.json change the language servers ([a47de0e](https://github.com/brunozilio/lite/commit/a47de0ed433736f306627f9d44e379d3ac7927c4))
* **lsp:** the LSP tool, offered when a language server is installed ([fc51b04](https://github.com/brunozilio/lite/commit/fc51b04895f5dda8c097449b85650ff2e525a1b1))
* MCP servers from the project and the user, with the owner's approval ([ff6b938](https://github.com/brunozilio/lite/commit/ff6b9385b43d5f9a29e94cb531d122d3ebf33ff7))
* **memory:** a new memory that means the same revises the older one ([8108d9b](https://github.com/brunozilio/lite/commit/8108d9bc392cd0a1b764d5a3a34a923a5c345763))
* **memory:** forgetting and revising are reversible ([70f31d8](https://github.com/brunozilio/lite/commit/70f31d8d2ecc8e1d263b626f5765ddf5a4bcbbab))
* **memory:** old episodes fade in recall unless they keep being used ([2e32553](https://github.com/brunozilio/lite/commit/2e325532faab3d6412215820a94343d31bf80b67))
* **memory:** redact keys, tokens and passwords before a memory is kept ([46b3369](https://github.com/brunozilio/lite/commit/46b3369be84464b9f3198b203f7397439084e6aa))
* **memory:** the memory optimizer is back, with undo, forgotten and restore ([c48f908](https://github.com/brunozilio/lite/commit/c48f9088a4201267c1038e12a7e228c6cf4b7974))
* **mlx:** superfik pull/rm for local models, like ollama ([ad89a48](https://github.com/brunozilio/lite/commit/ad89a480417e10ee9f1b2ccb7ad0016a3801c681))
* **native:** checkpoints per prompt, and SessionCommand::Rewind ([2cf8d9e](https://github.com/brunozilio/lite/commit/2cf8d9e09b9f31e5666646baa7035a3643ea3a78))
* one background daemon per computer for every terminal ([fe2ad1b](https://github.com/brunozilio/lite/commit/fe2ad1bb20969831cf80127371d787f00ab43889))
* plugins are folders in ~/.superfik/plugins; /reload says what is loaded ([857a15a](https://github.com/brunozilio/lite/commit/857a15a74f15f83e5128e681adf273cac879680a))
* plugins reach a run whole: hooks and MCP servers too (ADR 0019) ([5e42736](https://github.com/brunozilio/lite/commit/5e4273671ae176eb2f8c7e4827ef82d764d0f91f))
* **pr:** /pr runs the PR gate and a code review before opening the PR ([16b9b2a](https://github.com/brunozilio/lite/commit/16b9b2adec70ad0c1e3a3a8a11de29c917284b0d))
* read only Superfik's folders; nothing from .claude, ~/.claude or CLAUDE.md ([2aa7cbf](https://github.com/brunozilio/lite/commit/2aa7cbf53a340aaef54bb361283ce61b7912f357))
* **sdk:** side calls on Session: title, suggest, recap, heads_up ([e3c3554](https://github.com/brunozilio/lite/commit/e3c3554504023a428fdf339e3e01b04c19fbdda6))
* **sdk:** superfik-sdk: sessions, agents, skills, commands, memory and MCP ([9c0618a](https://github.com/brunozilio/lite/commit/9c0618af769edd17ec240a7621d2a0cbfbd169de))
* SendMessage continues a finished sub-agent with its conversation intact ([7fc67e6](https://github.com/brunozilio/lite/commit/7fc67e60e0235f4e3ca91e66d4414fb4199991d4))
* Superfik's own plugin format (.superfik-plugin/, ${SUPERFIK_PLUGIN_ROOT}); Claude Code plugins are not read ([acc1218](https://github.com/brunozilio/lite/commit/acc1218b90010c9e886c4cda90821d0a7e2c9f1b))


### Bug Fixes

* **cli:** link clang's runtime on macOS; Lite keeps its own data ([5d4c879](https://github.com/brunozilio/lite/commit/5d4c8798322118e5ac8ba2aac01469497b151b28))
* keep secrets in a sealed file, so the keychain stops asking ([6255713](https://github.com/brunozilio/lite/commit/62557131fc75c13ad50b0179c8ce9d91104eb945))
* **lsp:** places under the working folder's real path show relative to it; clangd smoke test ([dcb3d6d](https://github.com/brunozilio/lite/commit/dcb3d6d3b1781c9df915eeb5b25708a59d2258a0))
* **memory:** keep an unreadable book aside, fsync saves, never fall back to a temp folder ([2a7e90a](https://github.com/brunozilio/lite/commit/2a7e90a4643dfc25007337c062cdf09afa2b3b2a))

## Changelog
