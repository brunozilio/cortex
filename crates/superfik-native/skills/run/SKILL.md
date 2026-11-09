---
name: run
description: Launch and drive this project's app to see a change working. Use when asked to run, start, or screenshot the app, or to confirm a change works in the real app (not just tests). First looks for a project skill that already covers launching the app; otherwise falls back to built-in patterns per project type (CLI, server, TUI, Electron, browser-driven, library).
---

**Running means launching the actual app and interacting with it** -
not the test suite, not an `import` of an internal function and a
`console.log`. The app as a user (human or programmatic) would meet
it: the CLI at its command, the server at its socket, the GUI at its
window.

## First: does a project skill already cover this?

A project skill that launches this app is the repo's verified path -
its author already cold-started it and committed what worked: the
exact install line, the env vars, the patches, the driver. Use it
instead of rediscovering. The repo's instruction files (`SUPERFIK.md`,
also `CLAUDE.md` and `AGENTS.md`) often hold the run command too.

Project skills live in `.superfik/skills/` (Superfik also reads
`.claude/skills/`), from the working directory up to the repo root:

```bash
d=$PWD; while :; do
  grep -Hm1 '^description:' "$d"/.superfik/skills/*/SKILL.md "$d"/.claude/skills/*/SKILL.md 2>/dev/null
  [ -e "$d/.git" ] || [ "$d" = / ] && break
  d=$(dirname "$d")
done
```

On Windows without a POSIX shell, Glob for `**/.superfik/skills/*/SKILL.md`
and `**/.claude/skills/*/SKILL.md` and Grep their `description:` lines.

The user may be on macOS, Windows or Linux, and the app runs on their
own computer: use the platform's commands, and launch the app the way
that platform does (a window really opens; no xvfb needed outside a
headless Linux machine).

- **One describes launching/driving this app** -> read that SKILL.md
  and follow it verbatim. Don't paraphrase; don't skip the patches.
- **Mega-repo, several plausible, no clear match** -> ask the user
  which unit to run.
- **Stale** (fails on mechanics unrelated to your task) -> tell the
  user; offer to refresh it with what worked this time.
- **Nothing about running** -> fall back to the patterns below.

## Otherwise: match the shape, use the pattern

Pick the row closest to your project. Each example walks through
launch + first interaction; ignore any trailing "write the skill"
section - you're using the recipe, not authoring one.

| Project type | Handle | Example |
|---|---|---|
| CLI tool | direct invocation, exit code, stdin/stdout | [examples/cli.md](examples/cli.md) |
| Web server / API | background launch + `curl` smoke | [examples/server.md](examples/server.md) |
| TUI / interactive terminal | tmux `send-keys` / `capture-pane` | [examples/tui.md](examples/tui.md) |
| Electron / desktop GUI | Playwright `_electron` REPL (xvfb only on headless Linux) | [examples/electron.md](examples/electron.md) |
| Browser-driven | dev server + the browser card (`mcp__superfik_browser__*`) | [examples/playwright.md](examples/playwright.md) |
| Library / SDK | import-and-call smoke script at the package boundary | [examples/library.md](examples/library.md) |

If nothing fits, start from the closest match and adapt. For a web
app, [examples/playwright.md](examples/playwright.md) - open it in
Superfik's browser card, no custom driver needed. For a desktop app,
[examples/electron.md](examples/electron.md) - it has the `_electron`
REPL driver skeleton and the tmux wrapping.

## Drive it, don't just launch it

Launching with no interaction proves the entrypoint resolves. That's
not running the app - it's typechecking with extra steps. Drive it to
a point where a user would see something:

- CLI -> type a representative command, check the exit code and output.
- Server -> hit the route the diff touches with `curl`, read the body.
- TUI -> `send-keys` a navigation, `capture-pane` the result.
- Web UI -> `browser_navigate` to the dev server, act, `browser_snapshot`
  or `browser_screenshot` the result.
- GUI -> click the button, screenshot the window. **Look at the
  screenshot** (Read the image file). A blank frame is a failure to
  launch.

If the fallback pattern didn't work out of the box - you had to
install packages, set env vars, patch config, or write a driver -
recommend in your report capturing that work as a project skill
(`.superfik/skills/run-<unit>/SKILL.md`; the skill-creator skill can
write it). If it just worked, don't.
