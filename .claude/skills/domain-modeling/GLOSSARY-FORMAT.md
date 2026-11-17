# Glossary Format

The glossary is `docs/GLOSSARY.md`. It already exists; read it before adding to it and match the entries around the one you add.

## Structure

```md
# Glossary

{Short introduction: what the file is for and how paths are written.}

## {Cluster of related terms}

**Run.** One execution of the engine: one prompt from start to a terminal event. Lives in `crates/agent-engine/src/engine/mod.rs`.

**Turn.** One iteration of the agent loop inside a run: one model call plus the tool calls that reply asked for. It does not mean "one user prompt and its answer"; that is a run. Lives in `crates/agent-engine/src/agent/mod.rs`.
```

Each entry is one paragraph: the term in bold followed by a full stop, what it is, what it is easily confused with, and a closing "Lives in" sentence with the path, relative to the repository root, of the main place the term lives.

## Rules

- **Be opinionated.** When multiple words exist for the same concept, pick the best one and say in the entry which other words not to use for it.
- **Name the collisions.** When the same word names two things in the code, say so in the entry and say which one the term means.
- **Keep definitions tight.** A few sentences. Define what it IS, not how it is implemented.
- **Keep the "Lives in" path current.** A change that moves a term's home updates its entry in the same commit.
- **Never spell the product name.** Files under `docs/` write "the app" for the product, `<command>` for its binary, `.<command>/` for its project folder and `~/.<command>/` for its user folder.
- **Only include terms specific to this project's context.** General programming concepts (timeouts, error types, utility patterns) don't belong even if the project uses them extensively. Before adding a term, ask: is this a concept unique to this context, or a general programming concept? Only the former belongs.
- **Group terms under subheadings** when natural clusters emerge. If all terms belong to a single cohesive area, a flat list is fine.

## One glossary

This repo is a single context: every term goes into `docs/GLOSSARY.md`, under the `##` cluster it belongs to. Do not create per-crate glossaries.
