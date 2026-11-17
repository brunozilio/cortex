# ADR Format

ADRs live under `docs/decisions/`, one file per decision, named `NNNN-kebab-title.md` (for example `docs/decisions/0009-function-size-lint.md`). Other documents cite one by its file name, e.g. `decisions/0010-comments-stay-out-of-code.md`. Read two existing records before writing a new one.

## Template

```md
# NNNN {Short title of the decision}

Status: Accepted, YYYY-MM-DD

## Context

{The situation that forced a choice: what was true, what it cost, what was measured. Facts, not the answer.}

## Decision

{Present tense: what we decided. A short list when the decision has several parts.}

## Consequences

{What this buys.}

What gets worse:

- {Each cost the decision accepts, stated plainly.}
```

The value is in recording *that* a decision was made, *why*, and what it costs. Keep each section as short as the decision allows. A rejected alternative earns a sentence in Context only when someone would suggest it again.

Files under `docs/` must not spell the product name: write "the app" for the product and `<command>` for its binary.

## Replacing a decision

Write a new record with the next number and change the old record's status line to `Status: Superseded by NNNN-kebab-title.md`. Point citations of the old file at its successor.

## Numbering

Take the highest number among the files in `docs/decisions/` and increment by one, zero-padded to four digits.

## When to offer an ADR

All three of these must be true:

1. **Hard to reverse**: the cost of changing your mind later is meaningful
2. **Surprising without context**: a future reader will look at the code and wonder "why on earth did they do it this way?"
3. **The result of a real trade-off**: there were genuine alternatives and you picked one for specific reasons

If a decision is easy to reverse, skip it: you'll just reverse it. If it's not surprising, nobody will wonder why. If there was no real alternative, there's nothing to record beyond "we did the obvious thing."

### What qualifies

- **Architectural shape.** "We're using a monorepo." "The write model is event-sourced, the read model is projected into Postgres."
- **Integration patterns between contexts.** "Ordering and Shipping communicate via domain events, not synchronous HTTP."
- **Technology choices that carry lock-in.** Database, message bus, auth provider, deployment target. Not every library: just the ones that would take a quarter to swap out.
- **Boundary and scope decisions.** "Customer data is owned by the Customer context; other contexts reference it by ID only." The explicit no-s are as valuable as the yes-s.
- **Deliberate deviations from the obvious path.** "We're using manual SQL instead of an ORM because X." Anything where a reasonable reader would assume the opposite. These stop the next engineer from "fixing" something that was deliberate.
- **Constraints not visible in the code.** "We can't use AWS because of compliance requirements." "Response times must be under 200ms because of the partner API contract."
- **Rejected alternatives when the rejection is non-obvious.** If you considered GraphQL and picked REST for subtle reasons, record it; otherwise someone will suggest GraphQL again in six months.
