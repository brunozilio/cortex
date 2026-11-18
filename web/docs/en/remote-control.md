---
title: Remote control
description: Follow and drive this machine's sessions from your phone, through a relay you own, end-to-end encrypted, with the page installed on the home screen.
---

Remote control puts the [console](./trace-console) on your phone: the sessions, their runs, and a terminal attached to a session. The page installs on the phone's home screen like an app. The phone never reaches your computer directly: both connect to a relay you own, and everything between them is end-to-end encrypted, so the relay forwards bytes it cannot read.

## Set it up

You need a relay first. The quickest is a Cloudflare Worker on your own account, published by {{NAME}}:

```bash
{{COMMAND}} remote up
```

The first time, it opens the browser to sign in to Cloudflare: choose the account and allow it. It publishes the relay at `https://{{COMMAND}}-relay.<your subdomain>.workers.dev`, makes it this machine's relay, and prints a QR code that pairs your first phone. It fits in Cloudflare's free plan, and nothing stays open on your computer but the chat.

Then, in a chat:

```text
/remote pair
```

Scan the QR code (or open the link) on the phone and let the page connect. A pairing link works for one phone, once, within ten minutes.

On an iPhone, a page added to the home screen keeps its data apart from Safari, so a pairing made in Safari does not reach it. Add the page to the home screen, open it from there and paste the pairing link into the field it shows while it is not paired.

The page opens on **Chats**: the sessions, each as a chat. Tap one to read and answer it, allow or deny what the agent asks with the buttons, and use **+** for a new chat in the folder where the machine's chat is open. **Trace**, at the top, leads to the trace and the terminal.

The machine is reachable while a chat is open with remote control on. Once a relay is named and a phone is paired, every new chat connects by itself; only one chat on the machine holds the relay at a time, and the others say so.

## Manage phones

| Command | What it does |
|---|---|
| `/remote` | The relay, this machine's id, the paired phones and whether this chat is connected |
| `/remote pair` | Invite one more phone |
| `/remote off` | Disconnect this chat from the relay until the next `/remote pair` or the next start |
| `{{COMMAND}} remote devices` | List the paired phones |
| `{{COMMAND}} remote revoke "iPhone"` | Forget a phone; its open connections close within seconds |
| `{{COMMAND}} remote logout` | Forget the Cloudflare sign-in `remote up` keeps (the relay keeps running) |

Run `{{COMMAND}} remote up` again after updating {{NAME}}: it publishes the new page and relay, and paired phones keep working. All commands are in the [CLI reference](./cli-reference#remote).

## Run your own relay

Instead of Cloudflare, you can run the `relay` program from this repository on any server, behind HTTPS (a phone installs the page only from an HTTPS address):

```bash
cargo build --release -p relay
RELAY_LISTEN=127.0.0.1:8080 ./target/release/relay
```

Put a reverse proxy with TLS in front of it (Caddy, nginx, your platform's load balancer) and name it on the machine:

```bash
{{COMMAND}} remote relay https://relay.example.com
```

`relay --listen <address:port>` or `RELAY_LISTEN` choose where it listens; the default is `0.0.0.0:8080`. The relay keeps nothing on disk.

## How it is protected

- **End-to-end encryption.** Every phone talks to the machine over a Noise session (`IK`, and `IKpsk2` while pairing). The relay sees only sealed bytes.
- **Pairing.** The pairing code travels in the link's `#` part, which a browser never sends to a server. A phone is recorded only after it proves it holds the code. Each phone keeps its own key; there is no shared password.
- **The relay checks the machine.** A machine gets its place on the relay only after proving it holds the key its id is derived from, so nobody else can take your machine's place.
- **One door.** The machine forwards only the console's API to its own console on `127.0.0.1`, with the console's own checks; it opens no port.

## Know the risks

- **The terminal is on.** A paired phone can open a terminal that runs commands as you. That is the point of the feature, and it stays on by default. If you lose a phone, revoke it: `{{COMMAND}} remote revoke "<name>"`.
- **The relay serves the page.** The page your phone runs comes from the relay. A relay that is compromised, or the Cloudflare account it runs on, could serve a page that reads the phone's key and acts as that phone, terminal included. Encryption does not protect against that: only running the relay yourself, on an account or server you trust, does.
- **Availability.** Anyone who can reach the relay can try to flood it. That can stop remote control from working, but it gives no access.
