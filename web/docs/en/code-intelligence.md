---
title: Code intelligence
description: How {{NAME}} understands code beyond text search, with language servers for definitions, references and diagnostics, a tree-sitter code map that points prompts at the right files, and the blast radius of an edit.
---

Besides reading files and searching text, {{NAME}} uses two sources of knowledge about code:

- **language servers** (the same ones your editor uses) answer questions about definitions, references and types, report errors after each edit, and find who calls what an edit changed;
- a **code map**, a tree-sitter index of the repository's functions, types and callers, points each prompt at the code it is about.

## Language servers

### Built-in servers

| Server | Command | Files | Project markers |
|---|---|---|---|
| `rust-analyzer` | `rust-analyzer` | `.rs` | `Cargo.toml` |
| `typescript` | `typescript-language-server --stdio` | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `tsconfig.json`, `jsconfig.json`, `package.json` |
| `pyright` | `pyright-langserver --stdio` | `.py`, `.pyi` | `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt` |
| `gopls` | `gopls` | `.go` | `go.mod`, `go.work` |
| `clangd` | `clangd` | `.c`, `.h`, `.cc`, `.cpp`, `.hpp` | `compile_commands.json`, `CMakeLists.txt` |

A server is used when its command is on your `PATH`, or when {{NAME}} can install it for you.

> A language server runs the project's own code: rust-analyzer runs build scripts (`build.rs`) and procedural macros, tsserver loads the TypeScript in the project's `node_modules/typescript`, and other servers read the project's configuration in their own ways. So in a folder you have not [trusted](./sandbox-and-safety#workspace-trust) no language server starts: the `LSP` tool answers that the folder is not trusted and that you can trust it with `/trust`, and edits come back without diagnostics or callers. In a trusted folder servers run as described here, outside the sandbox.

### Installed on first use

When a built-in server is missing, the first **LSP** question about one of its files installs it into {{NAME}}'s local data folder, under `lsp/<server>`, and the question waits for the install:

| Server | Installed with |
|---|---|
| `rust-analyzer` | A pinned release downloaded from GitHub, checked against its SHA-256 (macOS, Linux and Windows) |
| `clangd` | A pinned release downloaded from GitHub, checked against its SHA-256 (macOS, Linux x86-64, Windows x86-64) |
| `typescript` | `npm` (`typescript-language-server` and `typescript`); needs Node.js |
| `pyright` | `npm`; needs Node.js |
| `gopls` | `go install`; needs Go |

Only the built-in servers with their built-in command are installed this way. You are asked before each install, in every mode including bypass; plan mode and folders you have not trusted never install, and where nobody can be asked the install is refused unless an allow rule such as `LSP(install:pyright)` names it (see [Language-server installs](./sandbox-and-safety#language-server-installs)). After an install fails, it is not tried again for 30 seconds; an install that runs longer than 10 minutes is stopped. Until a server is installed, edits are not checked with it; while it installs, the result of an edit says so:

```text
<diagnostics>
rust-analyzer is still installing, so this file was not checked; later edits will be.
</diagnostics>
```

A server that sits idle for 10 minutes is stopped; it starts again when needed.

### The LSP tool

When at least one server is available, the agent gets the **LSP** tool. It asks the server of a file:

| Operation | Answers |
|---|---|
| `goToDefinition` | Where the symbol at a position is defined |
| `findReferences` | Where it is used |
| `goToImplementation` | Its implementations |
| `hover` | Its type and documentation |
| `incomingCalls` / `outgoingCalls` | Who calls a function, and what it calls |
| `documentSymbol` | The outline of a file |
| `workspaceSymbol` | A symbol found by name across the project (with `query`) |
| `diagnostics` | The file's current errors and warnings |
| `workspaceDiagnostics` | Every error and warning the file's server has reported for the project so far |

Lines and columns count from 1, as **Read** shows them. The agent is told to prefer it over Grep to find where a symbol is defined or used, and to ask `documentSymbol` before reading a long file.

`diagnostics` sends the file as it is on disk to its server and waits up to 10 seconds for the server's report. `workspaceDiagnostics` does the same, then lists what the server has reported for every file so far (servers such as rust-analyzer check the whole project; others report only the files that were opened). Both answer with a count and one line per problem, errors first, hints left out, at most 50 lines:

```text
1 error and 1 warning in 2 files
src/parse.rs:42:9 error: mismatched types: expected `u32`, found `i64`
src/lib.rs:7:5 warning: unused import: `std::fs`
```

When the server is not installed yet, the question asks first, as any first use does, and nothing is installed in a folder you have not trusted.

### Errors after every edit

After **Edit** or **Write** changes a file that an available server handles, {{NAME}} sends the new text to the server and adds its errors and warnings to the tool result, errors first, up to 20:

```text
<diagnostics>
src/parse.rs:42:9 error: mismatched types: expected `u32`, found `i64`
src/parse.rs:57:5 warning: unused variable: `count`
… and 3 more
</diagnostics>
```

Hints and information messages are left out.

### Configure servers

The `lspServers` setting changes a built-in server field by field, adds a server, or turns one off:

```json
{
  "lspServers": {
    "rust-analyzer": { "command": "/opt/rust-analyzer", "env": { "RA_LOG": "error" } },
    "gopls": { "disabled": true },
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" } }
  }
}
```

| Field | Meaning |
|---|---|
| `command` | The program to start |
| `args` | Its arguments |
| `extensionToLanguage` | File extensions it handles, mapped to language ids |
| `rootMarkers` | Files that mark a project root |
| `env` | Extra environment variables |
| `disabled` | `true` removes the server |

A new server needs at least `command` and `extensionToLanguage`. Later settings files win over earlier ones. A project's checked-in `{{DIR}}/settings.json` may change which files a server handles, its root markers or turn it off, but its `command`, `args` and `env` are ignored: only your own settings files can choose what program runs. Plugins can add servers too (see [Plugins](./plugins)).

## Blast radius

> [!EXPERIMENTAL]
> Controlled by the `experimental-blast-radius` flag. Default: on.

When an edit changes a function, type or other symbol, the code that uses it may need to change too. After **Edit** or **Write** changes a file in the project, {{NAME}} asks the file's language server which symbols the changed lines belong to (up to 3), then which other files reference them, and tells the agent:

```text
<callers>Other files use what this edit changed in src/client.rs; check they still fit before you finish:
- retry (4 places): src/a.rs:7, src/jobs/sync.rs:31,88, src/main.rs:12
</callers>
```

Up to 8 files are listed per symbol, with up to 3 lines each. References in the edited file itself are not listed. The check uses a server that is already available and never waits for an install.

## Code map

> [!EXPERIMENTAL]
> Controlled by the `experimental-code-map` flag. Default: on.

In a git repository, {{NAME}} keeps a tree-sitter index of the repository's code and adds to each prompt a short list of the files and symbols that match it, so the agent can open them instead of searching:

```text
<code-map>
Files and symbols that match this prompt (from a tree-sitter index; open them rather than searching):
- src/memory/dedup.rs: fn merge_near_duplicates (L11, called from 2 places: src/memory/mod.rs, src/book.rs)
- src/memory/mod.rs: struct MemoryBook (L118)
</code-map>
```

### What is indexed

| Language | Files |
|---|---|
| Rust | `.rs` |
| TypeScript | `.ts`, `.mts`, `.cts`, `.tsx` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Python | `.py`, `.pyi` |
| Go | `.go` |

The index follows `.gitignore` and skips `target` and `node_modules` folders and files over 512 KB. It records each file's functions, types and other declarations with their line, and which names each file calls.

### How prompts are matched

Your prompt is split into words, identifiers (such as `merge_near_duplicates` or `MemoryBook`) and paths. Each file is scored by its symbols and its path: an identifier or a path named exactly counts most, then whole words; files scoring under a quarter of the best are dropped. Up to 12 files are listed, each with up to 4 symbols and, for each symbol, how many places call it and from which files (up to 3 named). A prompt that matches nothing gets no map.

### When it runs

- Only for prompts you type: not for commands, side calls or memory optimization.
- Only inside a git repository, and never when the project folder is your home folder.
- The index is cached in {{NAME}}'s local data folder under `code-map/`, one file per project, and only files that changed since the last prompt are read again.

A project can turn the code map off for everyone by checking in `"experimental-code-map": false` under `features` in its `{{DIR}}/settings.json`; then the repository is never scanned.

## Related

- [Feature flags](./experimental-features#code-intelligence): the flags on this page.
- [Verification and review](./verification-and-review#affected-tests): running the tests an edit affects.
- [Settings](./settings): where `lspServers` goes.
