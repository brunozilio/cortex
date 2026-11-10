# Native acceptance

Runs the real engine against loopback fixtures and temporary profiles; it
never reads the user's profiles, cookies or Keychain. Keep `CEF_PATH` set for
both building and running (the development bundler reads it at runtime), and
run modes one at a time: they share the example's bundle.

```sh
CEF_PATH=/path/to/engine cargo run -p superfik-browser --example native_smoke
```

`bash scripts/check-browser-native.sh` runs the whole matrix.
`SUPERFIK_ADHOC=1` signs ad hoc, as CI does. Each child has a 75-second
deadline.

| Flag | Checks |
| --- | --- |
| (none) | Two hardware identities across processes: page, iframe, worker, client-hint headers; default and private contexts. |
| `SUPERFIK_BROWSER_SOFTWARE_GL=1` | Debug builds only: WebGL through SwiftShader on a machine without a GPU (CI). |
| `SUPERFIK_EXTENDED_PROFILE_SMOKE=1` | Canvas (WebGL2, float reads), offline audio, fonts, WebGL/WebGPU identity, screen, WebRTC, real values, clicks at DPR 1 and 2, seed stability. |
| `SUPERFIK_PROXY_SMOKE=1` | Nine proxy cases (needs Python 3 and OpenSSL): CONNECT, workers, credential isolation, dead proxy, bad password, rejected TLS, no direct fallback, WebRTC UDP. |
| `SUPERFIK_AGENT_SMOKE=1` | Snapshots, stable refs, closed shadow roots, clicks, hidden passwords, detached refs. Combine with one of the flags below. |
| `SUPERFIK_INTERACTION_SMOKE=1` | Cross-origin frame click, typing, disabled/covered rejection, long snapshot pages, refs invalidated by navigation. |
| `SUPERFIK_WAIT_SMOKE=1` | Delayed text, failed-condition timeout, awaited Promises. |
| `SUPERFIK_OAUTH_SMOKE=1` | Opener, popup tab, switching, postMessage, close and return. |
| `SUPERFIK_FILES_SMOKE=1` | Uploads, staging confinement, symlink rejection, downloads without overwrite. |
| `SUPERFIK_DIALOG_SMOKE=1` | Dismiss, accept and prompt text. |
| `SUPERFIK_NAVIGATION_SMOKE=1` | An unfinished load errors after 20 s. |
| `SUPERFIK_COOKIE_SMOKE=1` | Cookies survive a restart and stay apart from other profiles. |
| `SUPERFIK_CLOSE_SMOKE=1` | Closing during startup cancels the queued agent request. |
| `SUPERFIK_FILL_SMOKE=1` | Vault fill alongside snapshots and actions. |
| `SUPERFIK_CAPTURE_SMOKE=1` | Typed submission reaches the host, secrets redacted, raw agent fills refused. |
| `SUPERFIK_PASSKEY_SMOKE=1` | A `localhost` page registers and asserts a passkey. |

Supported profile surfaces and their limits: `docs/platforms.md`.
