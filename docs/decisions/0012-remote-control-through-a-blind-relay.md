# 0012 Remote control: the console on a phone, through a blind relay

Status: Accepted, 2026-11-17

## Context

The owner wants to follow a session from a phone: the same page the console shows on the machine (runs, sessions, the terminal attached to a session), installed on the phone's home screen. Two written rules stood in the way: the vision listed remote control of a session over a network as a non-goal, and the console answers only on loopback.

A phone cannot reach a process on a laptop behind NAT, and a page is installable as a progressive web app (PWA) only from a secure origin (HTTPS, or `localhost`, which a phone does not have). The options weighed were a private overlay network (Tailscale), the local network with a home-made certificate authority, and a relay on the internet. The owner chose a relay they host themselves.

The console's terminal starts the app's own binary in a pseudo-terminal: whoever can open it can run commands as the user. Whatever carries it must not be readable or writable by the relay.

## Decision

**A relay, self-hosted.** A new binary crate, `relay`, does two things: it serves the page (the same `web/dist` the console embeds, plus a web manifest and a service worker), and it pairs WebSocket connections. The machine connects out to `wss://<relay>/machine?id=<machine id>`; a phone connects to `wss://<relay>/phone?machine=<machine id>`. The relay gives each phone connection a channel number and forwards binary frames (`open`, `data`, `close`, each with its channel) between the phone and the machine. It reads no frame's content. It refuses a second machine with an id already connected, caps phones per machine, frame size and idle time, and keeps nothing on disk. TLS is the host's job (a reverse proxy or the platform in front of it).

**End-to-end encryption.** Every phone channel is a Noise session between the phone and the machine; the relay sees only handshake and transport messages.

- The machine has a static X25519 key, kept in a private file in the data folder. Its machine id is the first 16 bytes of the SHA-256 of the public key, in hex.
- A paired phone has its own static X25519 key, kept in the page's IndexedDB. The phone's public key is the device's identity: there is no separate token.
- Pairing uses `Noise_IKpsk2_25519_ChaChaPoly_SHA256`. `<command> remote pair` makes a one-time code (32 random bytes, valid for ten minutes, used once), records it as a pending pairing, and prints a QR code and a link: `https://<relay>/#pair=<machine public key>.<code>`. The code and the key travel in the fragment, which a browser never sends to the relay. The pre-shared key is the SHA-256 of the code. The phone's first handshake payload carries its device name; the machine, on a valid handshake, consumes the pending pairing and records the device.
- A paired phone connects with `Noise_IK_25519_ChaChaPoly_SHA256`. The machine accepts the handshake only if the phone's static key is a recorded device. Revoking a device removes the record and closes its open channels.

**The tunnel.** Inside the encrypted channel the phone sends requests by number: a fetch (method, path, body), an event stream (server-sent events) or a socket (the terminal's WebSocket), and the machine answers with a head, chunks and an end. The machine forwards each one to its own console on loopback, as a client carrying the console's cookie, so the console's guard, routes and limits stay the only door. Only `/api/` paths are forwarded; the page itself comes from the relay.

**Who connects.** The process whose console is running connects to the relay when the owner has named a relay and at least one device is paired, or a pairing is pending; until then it checks every few seconds and stays off the relay. One process at a time holds the machine's place: the relay refuses a second connection under the same machine id, and the process that was refused says so and tries again later. The machine key, the relay's address, the paired devices and the pending invitations live in one private document, `remote/remote.json` in the data folder, changed under its lock.

**The page.** One build serves both places. On the machine it talks to `/api/` directly; served by the relay, it talks through the tunnel. Its network calls (one fetch helper, the event feed, the terminal socket) go through a transport chosen at start. The service worker caches only the page's own files, never an API answer.

**A relay on the machine itself.** For an owner without a server, `<command> remote up` runs the same relay inside the CLI, on `127.0.0.1` only, and publishes it with Cloudflare's `cloudflared`: a named tunnel on the owner's own host name (signing in, creating the tunnel and pointing the DNS the first time), or a quick tunnel on a `trycloudflare.com` address that changes at every start. The tunnel connects out, so the machine still opens no port. The remote book then keeps two addresses: the public one, written into pairing links, and the local one, which the chat connects to directly. A page installed on a phone belongs to one origin, so with a quick tunnel the phone pairs again after every start; the named tunnel is the way for daily use.

The vision's non-goal "not a network-reachable surface" becomes: the machine accepts no connection from the network; it reaches out to a relay its owner runs, and only paired devices get through.

## Consequences

- The owner can follow and drive a session from a phone, from anywhere the relay is reachable, with an installable page.
- The machine still listens on loopback only; the console's guard is unchanged and remains the only way into the API.
- A relay that is compromised or malicious can read nothing and inject nothing into a channel.

What gets worse:

- The relay serves the page's code. A malicious relay can serve a page that reads the phone's device key and acts as that device. Running the relay yourself is the only answer here; this is written in the threat model as a known gap.
- Anyone who can reach the relay can claim an unused machine id or flood it; the cost is availability, not access (the machine id is a hash of a key the impostor does not hold, and a phone completes no handshake with the wrong key).
- The terminal can be opened from the phone. That is the point, and it is the most powerful thing the console offers: a lost phone that is still paired runs commands as the user until it is revoked.
- The app grows a server binary, a Noise implementation on two sides (Rust and the page's TypeScript, kept in step by fixtures written from the Rust side) and state in the data folder (`remote/`).
- This partly reverses the "no remote server" line of `0008-single-user-local-product.md`: there is still one user and no identity model, but there is now a server, and it is the owner's to run.
