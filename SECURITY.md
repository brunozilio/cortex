# Security

Superfik keeps credentials, passkeys, sign-in tokens and a vault, and it
runs agents that execute code. Security reports matter here.

## Reporting a vulnerability

Report it **privately** through GitHub's
[private vulnerability reporting](https://github.com/brunozilio/superfik/security/advisories/new)
("Security" tab › "Report a vulnerability"). Never open a public issue, pull
request or discussion for it.

Include what you can:

- the affected component (desktop app, `superfik-server`, the web client,
  the updater, the browser engine) and version;
- the steps to reproduce, or a proof of concept;
- the impact you see, such as reading another user's data, escaping a
  sandbox or getting a token.

You will get an answer within a few days. A fix ships in a release, and the
advisory is published with it, crediting you unless you'd rather not be
named.

## Scope

- This repository: the app, `superfik-server`, `scripts/install-server.sh`
  and the release and update pipeline.
- The patched browser engine lives in
  [brunozilio/superfik-browser](https://github.com/brunozilio/superfik-browser)
  and has its own security policy. Bugs in Chromium or CEF themselves go
  upstream.
- A self-hosted server's own configuration (its TLS, its firewall, who holds
  its owner account) belongs to whoever runs it; see
  [docs/self-hosting.md](docs/self-hosting.md).

Especially interesting:

- a member of a server reaching another member's runs, sessions, secrets or
  the server's own data;
- an agent escaping its permission mode or its isolated runner user;
- a web page, webhook or channel message making the server or the app act;
- an update or server archive accepted without a valid signature.

Out of scope: vulnerabilities in model providers or other third-party
services, and attacks that need an already compromised machine or a
malicious server operator.

## Supported versions

Only the latest release gets security fixes. The app updates itself, and
`superfik-server` updates with the same installer it was set up with.
