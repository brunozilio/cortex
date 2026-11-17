# 0010 Comments stay out of code; rationale lives in docs

Status: Accepted, 2026-11-16

## Context

The working agreement forbids comments in code: no line, doc or block comments in Rust, none in shell, Python or TOML. A hook rejects an edit that adds one and `xtask lint` fails on any. The rule is fully complied with across the repository.

The audit confirmed one benefit: function and test names are long and descriptive, because the name is the only place to say what something does.

It also listed the cost:

- No public item in any crate has documentation.
- The `unsafe` blocks in `crates/platform` carry no safety note.
- Magic numbers, pinned protocol versions and lint allows have no explanation next to them.
- The "why" was delegated to commit messages, but the earlier decisions file, glossary and architecture document were written and then deleted, so understanding a decision meant searching the git history.
- The project's own vocabulary (upkeep, chore, fleet, parked, steer, brief) had no glossary, and several names collide.
- The only explanatory text beside the code is a README in each provider folder, which exists because Markdown is outside the rule. They show the need to explain is real.

The owner decided the rule stays.

## Decision

Code carries no comments. What a comment would have said goes to one of four places under `docs/`:

- `docs/vision.md`: what the product is for, for whom, and what it will not do.
- `docs/GLOSSARY.md`: what each project term means and where it lives.
- `docs/decisions/`: one record per decision, with its context and its costs.
- `docs/threat-model.md`: assets, actors, trust boundaries, known gaps.

`docs/architecture.md` describes how the pieces fit today. Names, types and tests remain the first tool; a commit message still carries the context of one change. A change that alters a decision, a term or a boundary updates the matching document in the same commit.

The single existing exception is unchanged: JSDoc type annotations in `content/` JavaScript.

## Consequences

- The rule keeps doing what it does well: it forces intent into names and tests.
- There is one known place to look for "why", and it is reviewed like code.

What gets worse:

- Rationale is far from the line it explains. A reader of a function has no pointer to the decision behind it and must know to look in `docs/`.
- Documents drift. Nothing fails when code changes and its decision record does not, except for the parts a lint checks (the crate graph, the crate table).
- `unsafe` blocks still have no adjacent safety argument, which is the convention reviewers of Rust code expect.
- `cargo doc` output is signatures only, so the crates are not usable as documented libraries. That is acceptable while nothing is published as one (`0005-typed-application-api.md`), and a cost if that changes.
- Local, small explanations (why this constant, why this order) do not justify a decision record and so tend not to be written anywhere.
