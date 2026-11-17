# 0008 Single-user local product: remove the multi-user seam

Status: Accepted, 2026-11-16

## Context

The working agreement required every command to carry a `Principal` and the backend seam to be kept "for the future remote, multi-user server". The audit found a skeleton with no server behind it:

- `Principal` (`crates/domain/src/principal.rs`) is created in one place in production and is always `LocalOwner`. It travels inside the message, so a real remote client could simply claim to be the local owner.
- The only `Authorizer` (`crates/backend/src/authorizer.rs`) accepts the local owner and rejects every `Member`.
- `SharedCredentials` (`crates/backend/src/providers/shared.rs`) is tested but never wired outside tests.
- There is no transport and no authentication. Features that run shell commands, read arbitrary paths, clone repositories and signal process ids take their arguments from the client.
- About a dozen process-global maps in the backend and the engine are keyed by a client number, not by a principal, which contradicts the seam it was meant to protect.

Meanwhile every feature pays for the passage through `Envelope`, `protocol` and the authorizer.

## Decision

The product is single-user and local. The multi-user concept is removed entirely: `Principal`, `Member`, the organisation and user ids, `Authorizer`, `SharedCredentials` and the `principal` field of the envelope. The client library's embedding API goes with it.

Data already on disk must keep opening, so the names derived from the old owner tag stay byte-identical:

- two constants keep the old values, the legacy owner tag `LocalOwner` and the owner directory `local`;
- the session file name (a hash that includes the owner tag), the agent-memory folder, the secrets entry names (which are also the cipher's associated data) and the `local/` path segments are unchanged;
- there is no data migration and the `local/` folders are not renamed.

Before any removal, one commit adds anchor tests with fixtures produced by the current code (a session file name, an agent-memory folder, a `secrets.json` that must still open, the `local/` paths). Removal then goes top-down, one commit per layer. In the same phase a single data root (`DataRoot`) is introduced for the vault, MCP authorisation and sessions, which makes tests hermetic. This is phase 2.3 of the plan.

## Consequences

- One fewer concept in every command, store and test.
- The typed application API (`0005-typed-application-api.md`) does not have to model identity.
- The documentation stops promising a server.

What gets worse:

- If a multi-user or remote product is ever wanted, per-owner partitioning and authorisation must be designed again from nothing.
- The on-disk layout keeps a `local/` segment and a `LocalOwner` tag that no longer mean anything; they are legacy constants that can never be changed without a migration.
- Run ownership checks between principals disappear; isolation between clients in one process rests only on the client number.
- The anchor tests freeze accidental details of the current file naming as a permanent contract.
