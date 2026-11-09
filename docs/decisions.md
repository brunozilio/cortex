# Decisions

Decisions that shape Superfik. Code cites them as `ADR NNNN`.

## 0002 — Self-hosted first, organizations from day one

Teams run the Superfik server on their own infrastructure (one organization
per install, Docker or the install script), so code and conversations stay
inside the company. Every stored record is keyed by `OrgId` anyway, so one
server can later host many organizations without a data migration.

## 0003 — Roles are the server's

Roles are resolved by the server, never trusted from a client: organization
roles `Owner`, `Admin`, `Member`, `Guest`, and project roles `Maintainer`,
`Contributor`, `Viewer`. Admins manage everyone but owners; only owners make
owners; the last owner stays. How users sign in is ADR 0017. SSO (OIDC,
SAML) and SCIM wait behind the `Authenticator` seam until someone needs them.

## 0004 — Memory has scopes; sessions have visibility

Each memory lives in a scope (personal, project, team, organization) and a
role decides who reads and writes each. A session is private by default and
can be shared with people, a team or the project. Explicit scopes, because a
team's procedures ("how we deploy here") must reach everyone while a
member's own notes must not.

## 0005 — The desktop embeds the same server

Used alone, the desktop runs the Superfik server in its own process, as an
organization of one (`Principal::LocalOwner`), storing in `<app data>`.
Local and self-hosted are one code path, tested once. The app only chooses
an address: this computer or a team's server (one of them, ADR 0010).

## 0007 — The server's network edge is async

`superfik-server` serves clients over WebSocket with axum on tokio. The
backend stays thread-based (crossbeam workers); only the network edge is
async. No ORM: the stores are small and owned by the server.

## 0010 — Clients never run; one server per connection

A client (desktop or browser) only shows and asks. Everything else happens
on the one server it is connected to: runs, git, memory, notes, plugins,
vaults, browser pages, settings. The client reaches even the embedded server
only through `Backend`, so each rule exists once, on the server, and the
policy and budgets bind. Consequences: a member works on the server's
checkout of a project, not a local clone; every per-user store lives in the
server's data folder; settings are per user on the server (no
`settings.json`, no environment overrides); clients sign in with a session
cookie; agents run as their own OS user (`superfik-runner`), apart from the
server's stores and secrets.

## 0011 — Connections stay on the server

A member's connections to other services (GitHub, Linear…) are kept by the
server, sealed, per member and per connection, and every command acts as
the member who sent it, never as the server. A connection is made by
dynamic registration (MCP authorization: the server registers itself and
keeps the PKCE verifier; the client relays the redirect's code), by a
public client (device flow, PKCE), or with the organization's own app
credentials. The client relays redirects, so a server needs no public
address. Vault unlock secrets never leave the server.

## 0012 — Agents operate the product: routines, channels, autonomy

Agents that know the code, memory and accounts also operate what ships:
a customer reports a problem, the agent answers, a bug becomes an issue
and a fix. Three primitives: a **routine** (a run started by a schedule,
a message, or a webhook, with a budget; pure rules, time passed in); a
**channel** (a plugin capability that receives and sends messages:
WhatsApp, e-mail, Instagram, X, LinkedIn, behind `Channel`); and
**autonomy** per routine (Draft by default, Bounded, Auto). Routines run
on the organization's server, so in local mode only while the app is open.
Webhook tokens live with the server's secrets, compare in constant time,
and an unknown address and a wrong token answer the same 404; bodies and
messages reach the model fenced as untrusted. The unofficial WhatsApp
client is opt-in and warned about; the official APIs need each provider's
approval, which only the member can obtain.

## 0013 — Windows is a first-class desktop, x64 first

The same app, embedded server and browser cards as on macOS; what the
client can do is said by `Platform::WINDOWS`, never `cfg` below the shell.
Our own CEF build (Superfik's patches; stock CEF is refused), pinned by
SHA-256. An MSI (WiX), unsigned until signing is switched on. Small state in
Roaming `%APPDATA%\Superfik`, large or machine-bound data in
`%LOCALAPPDATA%\Superfik`. Ctrl is the primary modifier; commands with
secrets prefer Git Bash, then PowerShell; the system unlock is Windows
Hello. Files holding secrets get an ACL for the current user only.

## 0015 — Superfik is the engine

No agent CLI runs underneath. The agent loop is ours (`superfik-native`,
`EngineKind::Native`): it calls the provider's API itself, runs its tools as
Rust functions, and emits the same `EngineEvent`s, so the reducer, protocol
and canvas are engine-agnostic. Providers sit behind `ModelApi`, tested
against captured streams; local models run on MLX as Chat Completions. The
system prompt, tools, sessions and compaction are ours; the prompt loads
`SUPERFIK.md` and names Superfik. Schedules are routines (ADR 0012); the
vault is the Superfik Vault behind `VaultProvider`.

## 0016 — API keys and subscriptions are separate provider plugins

OpenAI API, ChatGPT Subscription, Claude Subscription and Anthropic API are
four independently connected plugins in `superfik-plugins` (manifests in
`catalog/<id>/`, pure contracts in `superfik_plugins::providers`); the native
engine does the IO. Each has its own credentials and model catalog. The
backend handles PKCE and loopback callbacks and keeps credentials in its
`SecretStore`, saving rotated refresh tokens; runs carry a redacted handle a
client cannot supply. Subscription adapters apply their own preludes; the
tools, loop and project instructions stay Superfik's.

## 0017 — Open source; servers keep their own users

Superfik is open source and depends on no outside account service. Alone,
the app runs its embedded server as the local owner, with an optional
Touch ID lock. Teams self-host a server (`scripts/install-server.sh` or
Docker) whose users belong to it (`superfik-identity`): an owner or admin
adds a user by name, the server answers a first-access code (7 days, kept
as its hash), and the user chooses a password (argon2id); later sign-ins
are name and password, answering a 90-day token. A new code ends the
password and every sign-in; five wrong tries lock a name for 15 minutes.
No e-mail is needed. Updates come from GitHub Releases, each manifest
checked with the Ed25519 update key.
