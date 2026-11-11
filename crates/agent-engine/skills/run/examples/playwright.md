# Example: Browser-driven web app

You have a dev server that serves HTML to a browser. "Run the app" means
launching the dev server, driving a real browser against it, and
producing a snapshot or screenshot that proves the page rendered.

If the session has browser tools from an MCP server (Playwright MCP,
Chrome DevTools MCP or similar), use them. Otherwise drive headless
Chromium with Playwright from a small script.

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

The loop is: open the dev URL -> read the page as text -> act (click,
type, a key press) -> read it again to see the result -> screenshot the
frame that proves it.

With Playwright, a one-off script is enough for a single check:

```js
// smoke.mjs  (npm i -D playwright && npx playwright install chromium)
import { chromium } from 'playwright';
const browser = await chromium.launch();
const page = await browser.newPage();
page.on('console', m => m.type() === 'error' && console.log('console:', m.text()));
await page.goto('http://localhost:3000');
await page.getByRole('button', { name: 'New item' }).click();
await page.getByLabel('Title').fill('Smoke test');
await page.keyboard.press('Enter');
await page.getByText('Smoke test').waitFor();
await page.screenshot({ path: '/tmp/smoke.png' });
console.log(await page.locator('main').innerText());
await browser.close();
```

Read `innerText()` or an accessibility snapshot for checking: text is
exact and cheap. Take a screenshot when what matters is how the page
looks. Add `args: ['--no-sandbox']` to `launch()` only inside a
container.

For an interactive session over many steps, adapt
[electron.md](electron.md)'s REPL driver: import `{ chromium }` instead,
launch with `chromium.launch()`, get the page via
`(await browser.newContext()).newPage()` then `goto()` your dev URL, and
drop the Electron-only window introspection
(`.windows()`/`.firstWindow()`/the `windows` command).

## What to put in the skill

The project-specific bits only.

- **Dev command + port + stop.** The exact start line, any env vars it
  needs, and the command to stop it (per platform if they differ).
- **Auth.** Whatever gets a logged-in session - a sign-in sequence, a
  test account and where its password is kept, or a helper script that
  does the API dance and sets the cookie.
- **One representative interaction.** Not the whole app - one path that
  proves it's running, ending in a screenshot.
- **App-specific gotchas.** Only the ones you actually hit.

## Gotchas that recur

- **React controlled inputs.** Setting `el.value = '...'` in
  `page.evaluate` doesn't fire React's onChange. Use `fill()` or
  `type()` - they go through real input events.
- **Websockets / long-poll.** The page never goes idle; don't wait for
  `networkidle`. Wait for the element you actually need.
- **Slow first paint.** Vite/Next compile routes on demand; the first
  navigation can take 10s+. Raise the timeout rather than concluding
  it's broken.
- **Check for errors before declaring success.** A page can render its
  shell while every data fetch 500s - read the console and the dev
  server's log too.
