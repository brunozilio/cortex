# Example: Browser-driven web app

You have a dev server that serves HTML to a browser. "Run the app" means
launching the dev server, driving a real browser against it, and
producing a snapshot or screenshot that proves the page rendered.

Don't write a browser driver. Use Superfik's browser card: the
`mcp__superfik_browser__*` tools drive a real Chromium page that the
user sees beside the conversation and can take over.

## Dev server

Find the dev command (`package.json` `scripts.dev`, `Makefile`,
README, SUPERFIK.md), start it in the background, and wait for it to
actually serve:

```bash
npm run dev > /tmp/dev.log 2>&1 &   # or yarn dev, pnpm dev, make serve, ./dev.sh
timeout 30 bash -c 'until curl -sf http://localhost:3000 >/dev/null; do sleep 1; done'
```

Don't `sleep 5` - poll the port. Stop by killing the port's listener
-- `lsof -ti:3000 -sTCP:LISTEN | xargs -r kill` - before relaunching,
or the next run hits `EADDRINUSE`. (`$!` after `npm run dev &` is only
the npm wrapper; npm doesn't forward SIGTERM to the server it spawned,
so the port kill is what actually frees it.) Avoid `pkill -f` with a
broad pattern - it can match the agent's own command line, or a server
of the user's, and kill it.

On Windows (PowerShell), start it with
`Start-Process npm -ArgumentList 'run','dev' -RedirectStandardOutput dev.log -PassThru`,
poll with `Invoke-WebRequest http://localhost:3000 -UseBasicParsing`,
and free the port with
`Get-NetTCPConnection -LocalPort 3000 -State Listen | % { Stop-Process -Id $_.OwningProcess }`.

The port may already be taken by something the user runs. Check before
starting, and pick another port rather than killing what isn't yours.

## Drive

With the browser tools, the loop is: `browser_navigate` to the dev URL
-> `browser_snapshot` to read the page as text (each element you can
click or type into carries a `[ref=N]`) -> act (`browser_click`,
`browser_type`, a key press) -> `browser_snapshot` again to see the
result -> `browser_screenshot` for the frame that proves it.

For example, to check a "New item" flow:

1. `browser_navigate` to `http://localhost:3000`
2. `browser_snapshot` -> find the "New item" button's ref, confirm the
   page shows "Dashboard"
3. `browser_click` that ref, `browser_type` "Smoke test" into the title
   field's ref, press Enter
4. `browser_snapshot` -> "Smoke test" is listed
5. `browser_screenshot` -> the evidence
6. Check the browser console for errors (`browser_evaluate` can read
   what the page logged, if the app records it), and the dev server's
   log for 500s.

Prefer snapshots for reading: they are text, exact and cheap. Take a
screenshot when what matters is how the page looks.

**If the browser card isn't available** (no browser tools in this
session, or you need a scripted, repeatable run): adapt
[electron.md](electron.md)'s REPL driver - the structure and commands
transfer, but it's `_electron`-specific:
import `{ chromium }` instead, launch with
`chromium.launch()` (add `args: ['--no-sandbox']` only inside a
container), acquire the page via
`(await browser.newContext()).newPage()` then `goto()` your dev URL, and
drop the Electron-only window introspection
(`.windows()`/`.firstWindow()`/the `windows` command).

## What to put in the skill

The project-specific bits only. The browser tools handle the mechanics.

- **Dev command + port + stop.** The exact start line, any env vars it
  needs, and the command to stop it (per platform if they differ).
- **Auth.** Whatever gets a logged-in session - a sign-in sequence of
  `browser_type`/`browser_click`, a test account and where its password
  is kept, or a helper script that does the API dance and sets the
  cookie.
- **One representative interaction.** Not the whole app - one path that
  proves it's running, ending in a screenshot.
- **App-specific gotchas.** Only the ones you actually hit.

## Gotchas that recur

- **React controlled inputs.** Setting `el.value = '...'` through
  `browser_evaluate` doesn't fire React's onChange. Use `browser_type` -
  it goes through real key events.
- **Websockets / long-poll.** The page never goes idle. Snapshot until
  the element you actually need is there.
- **Slow first paint.** Vite/Next compile routes on demand; the first
  navigation can take 10s+. Snapshot again rather than concluding it's
  broken.
- **Signed-in sessions are the user's.** The browser card keeps cookies
  per conversation. Never sign out of or change an account beyond what
  the task needs.
- **Check for errors before declaring success.** A page can render its
  shell while every data fetch 500s - read the dev server's log too.
