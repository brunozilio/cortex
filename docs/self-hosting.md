# Self-hosting the Superfik server

A team runs one server (ADR 0002, 0017). Everyone's app connects to it
from **Settings › Server**, and every agent runs on it (ADR 0010). It keeps
its own users: there is no outside account, license or payment.

## Install it (Linux)

On a Linux machine (x86-64; Debian 13, Ubuntu 24.04 or newer), as root:

```sh
curl -fsSL https://raw.githubusercontent.com/brunozilio/superfik/main/scripts/install-server.sh \
  | sudo sh -s -- --domain superfik.example.com
```

`scripts/install-server.sh` downloads the latest release's
`superfik-server-<version>-linux-x86_64.tar.gz` and its signature
(`.tar.gz.sig`), and **installs nothing unless the signature verifies**:
an Ed25519 signature over the archive's SHA-256, made with Superfik's
update key (the same key that signs the app's updates,
`superfik_update::UPDATE_KEY`; its public half is in the script), checked
with `openssl pkeyutl -verify -rawin` (OpenSSL 3 or newer: Debian 13 and
Ubuntu 24.04 have it). The release also carries `.tar.gz.sha256` for a
manual check. To check an archive yourself:

```sh
scripts/install-server.sh --verify superfik-server-1.2.3-linux-x86_64.tar.gz   # .sig beside it
```

The script is fetched from `main`; pin it to a release tag
(`…/superfik/v1.2.3/scripts/install-server.sh`) and read it first if you
prefer. A fork signs with its own key: it replaces `UPDATE_KEY` (and the
script's PEM), or installs with `--public-key its-key.pem --repository
owner/name`.

It puts the verified files under `/usr/local/lib/superfik-server` (the
server, its native worker, `native-as-runner` and CEF's files for browser
pages), root's alone whatever owner the archive recorded, then runs
`superfik-server install`, which:

- makes the `superfik-runs` group, the `superfik` system user (in it) that
  owns the data folder `/var/lib/superfik` (mode `0700`), and the
  `superfik-runner` user agents' tools run as (home
  `/var/lib/superfik-runner`);
- makes the checkouts folder `/var/lib/superfik-runs` (`superfik:superfik-runs`,
  mode `2770`) and `/etc/sudoers.d/superfik-runner`, checked with `visudo`
  before it goes in place, which lets `superfik` run exactly one command as
  the runner: `/usr/local/lib/superfik-server/superfik-native-worker`;
- adds the first owner, `admin`, and prints their **first-access code**
  (once), and gives the data folder to `superfik`;
- writes `/etc/systemd/system/superfik.service` (below);
- with `--domain`, installs Caddy, serves the domain over HTTPS (Caddy gets
  the certificate) in front of the server on `127.0.0.1:7420`, and keeps
  the domain in the server's settings; without it the server listens on
  `0.0.0.0:7420` and TLS is yours to put in front;
- enables and **restarts** the service, last.

Every step must succeed or the install stops and names it. Run the same
command again to upgrade: it verifies and replaces the files, keeps the
users and data, and restarts the server. `--domain` takes a host name only,
and `--user` a plain system user name (not root, not `superfik-runner`).

Point the domain's DNS at the machine before installing. Then, in the
app: **Settings › Server › First access**, the domain, `admin`, the code,
and the password you choose.

### The service

The unit sets `SUPERFIK_DATA`, `SUPERFIK_LISTEN`,
`SUPERFIK_CHECKOUTS=/var/lib/superfik-runs` and
`SUPERFIK_NATIVE_WORKER=/usr/local/lib/superfik-server/native-as-runner`,
with `UMask=0002` so checkouts stay group-writable for the runner (the
data folder stays `0700`). It is hardened with `ProtectSystem=strict`
(writes only to the data folder, the checkouts and the runner's home),
`ProtectHome=yes` and `PrivateTmp=yes`. It deliberately does **not** set
`NoNewPrivileges=`, nor anything that implies it (`SystemCallFilter=`,
`RestrictSUIDSGID=`, `PrivateDevices=`, `ProtectKernel*=`,
`MemoryDenyWriteExecute=`, `RestrictNamespaces=`, …) or narrows the
capability bounding set: tools reach `superfik-runner` through `sudo`, a
set-uid program, and each of those would stop it. After installing,
`systemd-analyze security superfik` shows what remains.

A server without `SUPERFIK_NATIVE_WORKER` refuses members' native agent
runs (it never runs their tools in its own process, which holds the
accounts and secrets); it warns at start, and a worker path that is not a
file stops it from starting.

## Run it with Docker

```sh
docker build -f deploy/Dockerfile -t superfik-server .
docker run -d --name superfik -p 7420:7420 \
  -v superfik:/var/lib/superfik -v superfik-runs:/var/lib/superfik-runs superfik-server
docker exec superfik superfik-server init          # the first owner and their code
```

Put it behind a reverse proxy that terminates TLS (Caddy, nginx, a cloud
load balancer) and forwards WebSocket upgrades on `/ws`.

## The command line

The same binary manages the server, even while it runs (it reads the
files again when they change, and writes `accounts.json` under a lock, so
neither loses the other's change). With Docker, prefix `docker exec
superfik`. Run it as the data folder's owner, `sudo -u superfik
superfik-server …` (it refuses anyone else, root included, so no file in
the folder ever becomes one the service cannot read).

```sh
superfik-server init [--owner NAME]          # the first owner, once; prints their code
superfik-server user list
superfik-server user add ana [--role admin]  # member by default; prints ana's code
superfik-server user code ana                # a new code: her password and sign-ins end
superfik-server user role ana member
superfik-server user remove ana
superfik-server config                       # the domain
superfik-server config domain superfik.example.com   # "-" clears it
superfik-server install [--domain D] [--user U]
```

| Variable | Default | Meaning |
|---|---|---|
| `SUPERFIK_LISTEN` | `0.0.0.0:7420` (a Mac: `127.0.0.1:7420`) | Address to listen on. |
| `SUPERFIK_DATA` | `/var/lib/superfik` (a Mac: `~/Library/Application Support/Superfik/server`) | Users (`accounts.json`), settings (`server.json`), sessions and memory. Back it up. |
| `SUPERFIK_SECRET_KEY` | `secrets.key` in the data folder | 64 hex digits sealing the plugins' connection secrets (`secrets.json`). Hand it in from a KMS to keep it off the volume. |
| `SUPERFIK_WEB_ORIGINS` | none | Comma-separated origins whose pages may sign in and connect, besides the server's domain (Settings › Server). The server's own host is accepted only when it is an IP address or `localhost`: a browser reaching it by any other name must use the domain or be listed here. |
| `SUPERFIK_WEB` | unset | The folder of the web app's bundle (`index.html`, `superfik_web.js`, `superfik_web_bg.wasm`), served at `/` beside `/ws`. Unset: no web app. Text in Chinese, Japanese or Korean makes the page fetch Noto Sans CJK from `fonts/NotoSansSC-Regular.otf` and `fonts/NotoSansKR-Regular.otf` in this folder when present, else from cdn.jsdelivr.net. |
| `SUPERFIK_GITHUB_CLIENT_ID` | unset | Your GitHub App's client id, for members to sign in to GitHub (see below). |

To try it on a Mac, `cargo run -p superfik-server -- init` and then
`cargo run -p superfik-server` are enough: the data stays in
`~/Library/Application Support/Superfik/server` and it listens on this
computer only; connect the app to `localhost:7420`.

## Users and signing in

Owners and admins add users by name (Settings › Server › Users, or the
command line); the server answers a **first-access code**
(`XXXX-XXXX-XXXX`, 7 days, used once, kept only as its hash). The user's
first access (the server's address, their name and the code) chooses their
password (10 characters or more, argon2id). From then on they sign in with
name and password. Five wrong tries in a row lock a name for 15 minutes. A
forgotten password is a new code (**New code**), which also ends every
sign-in; removing a user ends them for good. Admins manage everyone but
owners; only owners make owners, and the last owner stays.

Signing in answers a token (90 days) the app keeps in the system keychain
and connects with from then on; browsers sign in at `/sign-in` and keep it
in the session cookie (`superfik_session`, `HttpOnly`, `Secure`,
`SameSite=Strict`), sent on the `/ws` upgrade. Serve the server over HTTPS
so browsers keep the cookie.

| Route | |
|---|---|
| `POST /auth/first-access` `{name, code, password}` | The first access; answers `{token}` and sets the cookie. |
| `POST /auth/sign-in` `{name, password}` | Later sign-ins; the same answer. |
| `GET/POST/DELETE /auth/session` | Whether the cookie signs in; set it from a kept token; sign out. |
| `GET /sign-in` | The browser's sign-in page. |

The app connects over HTTPS unless the address says `http://`; plain HTTP
to another machine asks the user to confirm, since passwords and tokens
would travel unencrypted. Give the server a domain (`install --domain`).

Owners and admins also set, in Settings › Server, the **domain** users
reach the server at (its HTTPS origin may then sign in from a browser); it
holds at once, without a restart. The server sends no e-mail.

## Audit log

Every command a member sends is appended to `<data>/audit.jsonl` (who,
which command, when, and whether it was `allowed` or `refused`; never its
content). The log rotates at 16 MiB and keeps the five previous files.
Owners and admins export it for a SIEM with their sign-in token:

```sh
curl -H "Authorization: Bearer $TOKEN" \
  "https://superfik.example.com/admin/audit?since=1792022400&limit=1000"
```

The answer is JSON lines (`application/x-ndjson`), oldest first, from
`since` (Unix seconds; 0 or absent: everything), at most `limit` lines
(default 1,000, up to 10,000) after skipping `skip`. A full page carries an
`x-audit-next` header with the query for the next one.

## Rolling the app out with device management (MDM)

Push a configuration profile with a custom settings payload for the
bundle `com.brunozilio.superfik` (Jamf, Kandji, Intune and others all
offer one). macOS writes it to `/Library/Managed Preferences`, where the
app reads it at launch:

| Key | Type | Effect |
|---|---|---|
| `RequireOrganization` | boolean | No prompt goes out until the app is connected to the team's server |

On Windows the same keys go in `%ProgramData%\Superfik\managed.json`.
On Linux they go in `/etc/superfik/managed.json`.

## Agents running on the server

Every agent of a member connected to the server runs on it (ADR 0010):
the app opens a project from the repositories, and the server keeps a
checkout of it for that member, under `<checkouts>/<member>/<remote>`.
The server clones `https://<remote>.git`, only from the hosts in
`SUPERFIK_GIT_HOSTS` (comma-separated; github.com, gitlab.com and
bitbucket.org by default), never stopping to ask for credentials. Give
git credentials for your forge (a credential helper or `GIT_CONFIG_*`
variables). Before every prompt the server checks the member's role and
the policy: the permission mode, the models, and the budget.
At most four checkouts are prepared at once.

The image ships the native engine and executes file, shell and MCP stdio tools
as `superfik-runner`, an unprivileged user. That user shares the `superfik-runs`
group with checkouts in `/var/lib/superfik-runs` (`SUPERFIK_CHECKOUTS`) and cannot
open the backend data folder (`/var/lib/superfik`, mode 0700: accounts, secrets,
and audit log). The backend invokes `/usr/local/bin/native-as-runner`
(`SUPERFIK_NATIVE_WORKER`); sudo permits only `superfik-native-worker`.
Model credentials remain in the backend, and accounts are connected in
Settings → AI. The image does not install or invoke the Claude Code CLI.

Browser cards opened on the server reach the web only through a proxy the
server runs on loopback: only `http`/`https`, and never loopback, private,
link-local (cloud metadata) or other internal addresses, checked on every
connection a page makes, redirects included.

```sh
docker run -p 7420:7420 \
  -v superfik:/var/lib/superfik -v superfik-runs:/var/lib/superfik-runs superfik-server
```

Members' checkouts are apart by folder, not by user: an agent may read
another member's checkout.

Members sign in to GitHub on the server through your GitHub App, so
repositories, clones, pull requests and reviews act as each of them.
Create a GitHub App (user-to-server tokens with expiry, **Device flow**
enabled; permissions: contents and pull requests read & write, metadata
read), and set its client id as `SUPERFIK_GITHUB_CLIENT_ID`. In the app,
"Sign in with GitHub" in a repository picker shows a code to type at
github.com; the server keeps the token sealed in `<data>/secrets.json` and
renews it. Without the variable, members cannot sign in to GitHub, and the
server's own `gh` is never used for them.

A member's commands may name only folders inside their own checkouts:
anything else (a folder of their computer, another member's checkout,
`..`) is refused before it runs. Members' memory, notes, sessions and
plugins live under `<data>/members`.
