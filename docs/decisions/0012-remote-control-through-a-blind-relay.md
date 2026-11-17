# 0012 Remote control: the console on a phone, through a blind relay

Status: Accepted, 2026-11-17

## Context

The owner wants to follow a session from a phone: the same page the console shows on the machine (runs, sessions, the terminal attached to a session), installed on the phone's home screen. Two written rules stood in the way: the vision listed remote control of a session over a network as a non-goal, and the console answers only on loopback.

A phone cannot reach a process on a laptop behind NAT, and a page is installable as a progressive web app (PWA) only from a secure origin (HTTPS, or `localhost`, which a phone does not have). The options weighed were a private overlay network (Tailscale), the local network with a home-made certificate authority, and a relay on the internet. The owner chose a relay they host themselves.

The console's terminal starts the app's own binary in a pseudo-terminal: whoever can open it can run commands as the user. Whatever carries it must not be readable or writable by the relay.

## Decision

**A relay, self-hosted.** A new binary crate, `relay`, does two things: it serves the page (the same `web/dist` the console embeds, plus a web manifest and a service worker), and it pairs WebSocket connections. The machine connects out to `wss://<relay>/machine?id=<machine id>`; a phone connects to `wss://<relay>/phone?machine=<machine id>`. The relay gives each phone connection a channel number and forwards binary frames (`open`, `data`, `close`, each with its channel) between the phone and the machine. It reads no frame's content. A machine takes its place only once it proves the key behind its id: the relay sends `challenge <ephemeral X25519 key> <nonce>`, the machine answers `proof <public key> <tag>` with the tag the SHA-256 of a label, the X25519 of its key with the ephemeral one, the nonce and the id, and the relay checks that the key hashes to the id and the tag matches before it answers `proven` (`tunnel::proof`; the Worker does the same with WebCrypto). It refuses a second machine with an id already connected, caps phones per machine, frame size and idle time, and keeps nothing on disk. TLS is the host's job (a reverse proxy or the platform in front of it).

**End-to-end encryption.** Every phone channel is a Noise session between the phone and the machine; the relay sees only handshake and transport messages.

- The machine has a static X25519 key, kept in a private file in the data folder. Its machine id is the first 16 bytes of the SHA-256 of the public key, in hex.
- A paired phone has its own static X25519 key, kept in the page's IndexedDB. The phone's public key is the device's identity: there is no separate token.
- Pairing uses `Noise_IKpsk2_25519_ChaChaPoly_SHA256`. `<command> remote pair` makes a one-time code (32 random bytes, valid for ten minutes, used once), records it as a pending pairing, and prints a QR code and a link: `https://<relay>/#pair=<machine public key>.<code>`. The code and the key travel in the fragment, which a browser never sends to the relay. The pre-shared key is the SHA-256 of the code. The phone's first handshake payload carries its device name. In `IKpsk2` the code enters only with the machine's reply, so the first message proves nothing about it: the machine consumes the pending pairing and records the device only when the phone's first sealed transport message opens (the page sends one at once, a no-op `close` of request 0), which only a phone holding the code can produce; a channel that has not proven it within ten seconds is closed.
- A paired phone connects with `Noise_IK_25519_ChaChaPoly_SHA256`. The machine accepts the handshake only if the phone's static key is a recorded device. Revoking a device removes the record and closes its open channels.

**The tunnel.** Inside the encrypted channel the phone sends requests by number: a fetch (method, path, body), an event stream (server-sent events) or a socket (the terminal's WebSocket), and the machine answers with a head, chunks and an end. The machine forwards each one to its own console on loopback, as a client carrying the console's token, so the console's guard, routes and limits stay the only door. Only `/api/` paths are forwarded; the page itself comes from the relay.

**Who connects.** The process whose console is running connects to the relay when the owner has named a relay and at least one device is paired, or a pairing is pending; until then it checks every few seconds and stays off the relay. One process at a time holds the machine's place: the relay refuses a second connection under the same machine id, and the process that was refused says so and tries again later. The machine key, the relay's address, the paired devices and the pending invitations live in one private document, `remote/remote.json` in the data folder, changed under its lock.

**The page.** One build serves both places. On the machine it talks to `/api/` directly; served by the relay, it talks through the tunnel. Its network calls (one fetch helper, the event feed, the terminal socket) go through a transport chosen at start. The service worker caches only the page's own files, never an API answer.

**A relay without a server.** For an owner without a server, `<command> remote up` publishes the same relay as a Cloudflare Worker on the owner's own account, through Cloudflare's REST API. It signs in to Cloudflare in the browser (OAuth authorization code with PKCE, a fixed loopback redirect, the client id, redirect and scopes named in `config.toml`) and keeps the sign-in, refreshed when stale, in the credentials vault; `CLOUDFLARE_API_TOKEN` still takes its place, for machines without a browser. The client id in `config.toml` is, for now, the one Cloudflare's own CLI (`wrangler`) uses, so the flow can be tried before the app registers its own client; it must be replaced by the app's own (Cloudflare's self-managed OAuth clients, made public for other accounts) before a release. A sign-in made with another client id is not used. The Worker (`crates/relay/worker/relay.js`) speaks the relay's protocol: one Durable Object per machine id holds the machine's socket and its phones' (hibernating WebSockets, the channel kept in each socket's attachment), the page is uploaded as data modules beside it, and the address is fixed, `https://<command>-relay.<account subdomain>.workers.dev`, so a page installed on a phone keeps working. A first attempt ran the relay on the machine behind `cloudflared`; it was dropped because it needed an external binary, and a domain for a stable address. Cloudflare's tunnel protocol is not a public API, so connecting without `cloudflared` was not an option. The two relays answer a text `ping` with `pong`, so phones and machines keep idle connections open through either.


The vision's non-goal "not a network-reachable surface" becomes: the machine accepts no connection from the network; it reaches out to a relay its owner runs, and only paired devices get through.

## Consequences

- The owner can follow and drive a session from a phone, from anywhere the relay is reachable, with an installable page.
- The machine still listens on loopback only; the console's guard is unchanged and remains the only way into the API.
- A relay that is compromised or malicious can read nothing and inject nothing into a channel.

What gets worse:

- The relay serves the page's code. A malicious relay can serve a page that reads the phone's device key and acts as that device. Running the relay yourself is the only answer here; this is written in the threat model as a known gap.
- Anyone who can reach the relay can flood it; the cost is availability, not access. Taking a machine's id needs the machine's key, which the relay checks before giving the place.
- The terminal can be opened from the phone. That is the point, and it is the most powerful thing the console offers: a lost phone that is still paired runs commands as the user until it is revoked. Keeping the terminal off for remote channels unless the owner turns it on was proposed and declined (2026-11-17): it stays on by default, because driving a session from the phone is what the feature is for. What answers for it is the pairing (one device key per phone, revocation cutting open channels within seconds) and, against a hostile relay serving the page, running the relay yourself.
- The app grows a server binary, a Noise implementation on two sides (Rust and the page's TypeScript, kept in step by fixtures written from the Rust side) and state in the data folder (`remote/`).
- This partly reverses the "no remote server" line of `0008-single-user-local-product.md`: there is still one user and no identity model, but there is now a server, and it is the owner's to run.
