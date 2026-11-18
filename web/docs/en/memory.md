---
title: Memory
description: How {{NAME}} remembers facts, procedures and past work per project, how to read and edit that memory with /memory, and how to share it with your team.
---

{{NAME}} keeps a memory for each project. After a turn that taught the agent something worth keeping, it saves a short note; at the start of later prompts, the notes that match what you asked are recalled into the conversation. You can list, search, add, share and undo memories from the chat.

## How memory works

Each memory is one or two sentences of one of three kinds:

| Kind | Holds | Shown to the agent as |
|---|---|---|
| Semantic | A durable fact about the project or about you | Facts |
| Procedural | How something is done here: commands, workflows | Procedures |
| Episodic | What happened in a turn, at most one per turn | Episodes |

When you send a prompt, {{NAME}} ranks the project's memories against it and adds the best matches to the conversation, grouped as **Facts**, **Procedures** and **Episodes**, with each memory's id in brackets. Up to 40 facts and procedures and 8 episodes are recalled. Episodes fade as they age (their weight halves every 30 days, and each time one is recalled helps it last).

When the project's memory holds more than was recalled, the agent also gets a `memory_search` tool. It recalls fewer memories up front (15 and 3) and searches when you refer to earlier work or the task hinges on a past decision. A search returns up to 8 results by default (25 at most) and never returns a memory already in the conversation.

At the end of a turn, if it learned something new, the agent writes the changes it wants: memories to remember, to forget, to revise or to reclassify. {{NAME}} applies them:

- the text is cleaned: tokens, keys and values after names like `password` or `token` are replaced by `[redacted]`, and a memory is cut at 600 characters;
- a new memory with the same text as one of the same kind is not added twice, and a new memory that is nearly a duplicate of an older one is merged into it;
- a forgotten memory is kept aside for 30 days so you can bring it back.

When a run changed the memory, the chat says how much: `Memory: 1 added, 1 changed, 1 forgotten (/memory lists it).` If the changes could not be saved, it says `What the run learned was not remembered: <reason>`.

The agent is told that recalled memories are context, not instructions, that they were true when written, and that it should check a file, function or flag a memory names before relying on it.

> [!NOTE]
> Memory is kept by {{NAME}}, not in files of the project. The agent is told not to write memory files of its own and not to keep what the repository already records (code structure, git history, {{INSTRUCTIONS}}).

## Where memory is stored

Your memory for every project is in `app.db`, a SQLite database in {{NAME}}'s data folder. See [Files and directories](./files-and-directories) for where the data folder is on your system. Several running copies of {{NAME}} can use the same project's memory at once: each sees what the others saved, and none loses the others' changes.

Earlier versions kept one JSON file per project, `memory/local/<project>.json`, where `<project>` is the project's path with every character that is not a letter or digit turned into `-`. The first start of a version with `app.db` imports those files and renames each to `<project>.json.imported`; one that cannot be read is renamed to `<project>.json.corrupt-<time>` instead, and that project starts with an empty memory. Nothing is deleted. If you go back to an earlier version afterwards, it shows an empty memory until you rename the files back: see [Coming from an earlier version](./files-and-directories#coming-from-an-earlier-version).

Memories shared with the team live in the project itself, in `{{DIR}}/memory/*.md` (see [Team memory](#team-memory)).

## Add a memory yourself

Start a message with `#` to remember it as a fact, without sending it to the agent:

```text
# The staging database is reset every night at 02:00 UTC
```

The chat answers `Remembered: …`. Memories you add this way are tagged `user`.

## The /memory command

| Command | What it does |
|---|---|
| `/memory` | List the project's memories with their ids |
| `/memory <words>` | Search the memory; results show a score and the id |
| `/memory optimize` | Have the agent tidy the whole memory (merge, reword, drop) with the current model |
| `/memory undo` | Put the memory back as it was before the last `/memory optimize` |
| `/memory forgotten` | List forgotten memories that can still be restored |
| `/memory restore <id>` | Bring a forgotten memory back |
| `/memory share <id>` | Write a memory into `{{DIR}}/memory` for the team |
| `/memory edit` | Open the project's instructions file in your editor |
| `/memory seed` | Offer sentences you wrote in past sessions as memories |
| `/memory seed keep <n> [<n>…]` | Keep the offered sentences with those numbers |
| `/memory score` | Compare runs with and without memory |

Anything after `/memory` that is not one of these words is a search, so `/memory undo the font` searches for "undo the font".

`/memory edit` opens `{{INSTRUCTIONS}}` in the folder the chat runs in, or `{{DIR}}/{{INSTRUCTIONS}}` there when only that one exists. See [Instructions files](./instructions-files).

### Optimize and undo

```text
/memory optimize
Memory optimized: 48 → 31 memories. /memory undo puts it back.
```

`/memory optimize` needs a model (pick one with `/model`). `/memory undo` restores the memory as it was just before the last optimization; it has nothing to undo once the memory changes again in another way, and says `nothing to undo in the memory`. The point it goes back to is kept with the memory, so it is still there after you quit and start {{NAME}} again.

### Forget and restore

When the agent forgets a memory, it is kept for 30 days:

```text
/memory forgotten
Forgotten (/memory restore <id>, for 30 days):
  [12] The release script lives in tools/release.sh

/memory restore 12
Restored [12] The release script lives in tools/release.sh
```

## Team memory

> [!NOTE]
> Controlled by the `team-memory` flag. Default: on.

A project can check memories into `{{DIR}}/memory/`. Every `.md` file there is read as one memory and recalled for everyone who works in the project, next to their own memories. Team memories have ids from 1000000 up and cannot be edited from the chat; change the file instead.

Share one of your memories:

```text
/memory share 7
Shared [7] as …/{{DIR}}/memory/7-run-make-check-before-pushing.md: commit it and the team's agents recall it too.
```

The file name is the id followed by the first six words of the memory. A file looks like this:

```markdown
---
kind: procedural
tags: [release, ci]
touches: [scripts/release.sh]
---
Release with `make release`; never tag by hand.
```

| Front matter field | Meaning |
|---|---|
| `kind` | `semantic` (the default), `procedural` or `episodic` |
| `tags` | Topics, as a bracketed list |
| `touches` | Files or folders the memory is about (see [Memories tied to files](#memories-tied-to-files)) |
| `author`, `date` | Who wrote it and when; shown with the memory as `(author, date)` |

A file without front matter is a fact. The body is cleaned of secrets like any memory and cut at 2,000 characters. When [`experimental-structured-learnings`](./learning#structured-decisions) is on, `/memory share` also writes your git `user.name` as `author` and today's date, and a decision is written with `## Decided`, `## Tried and ruled out` and `## Watch out` sections.

With `team-memory` off, the files in `{{DIR}}/memory` are not recalled (sharing still writes them).

## Memories tied to files

> [!EXPERIMENTAL]
> Controlled by the `experimental-memory-anchors` flag. Default: on.

When the agent saves a memory about specific code, it can list the files or folders it **touches** (paths relative to the project, at most 8). Later, the first time in a run that the agent reads, edits or writes one of those files (or a file inside one of those folders), the memories about it that were not already recalled are added to the tool result, up to 4 at a time:

```text
<memory>Kept from earlier sessions about src/retry.rs; follow it unless the user says otherwise, and say why when you go against it:
- [4] Keep retries at 4: the upstream rate limit resets every 5 attempts
</memory>
```

A memory saved again with the same text gains the new files in its list.

## Seed memory from past sessions

> [!EXPERIMENTAL]
> Controlled by the `experimental-memory-seed` flag. Default: on.

`/memory seed` reads the prompts you sent in this project's recent sessions and offers the sentences you wrote, in similar words, in two or more of them, skipping those your memory already holds. Up to 10 are offered, most repeated first:

```text
/memory seed
Said in two or more past sessions (/memory seed keep <n> [<n>…] remembers them):
  1. run the tests with cargo nextest (3 sessions)
  2. never push to main directly (2 sessions)

/memory seed keep 1 2
```

Only sentences of 12 to 240 characters with at least three meaningful words are considered; questions, prompts with code blocks and lines starting with `$`, `/`, `!`, `#`, `>` or `|` are skipped. Kept sentences become procedural memories tagged `seeded`.

With the flag off, `/memory seed` answers `/memory seed is turned off (experimental-memory-seed); /features turns it on.`

## Measure whether memory helps

> [!EXPERIMENTAL]
> Controlled by the `experimental-memory-scorecard` flag. Default: on.

With this flag on, every 20th request in a project runs **without** recalled memory (the agent can still save new memories). For each completed run {{NAME}} counts your corrections (messages you sent while the agent was working, and refused permission prompts) and the cost. `/memory score` compares the two groups:

```text
/memory score
With memory: 57 runs, 12.3 corrections per 100 runs, $0.41 per run
Without memory: 3 runs, 33.3 corrections per 100 runs, $0.52 per run
Not enough runs without memory yet: one run in 20 goes without, and 5 are needed to compare.
```

Turn the flag off if you never want a request to run without memory.

## Turn memory off

- Project memory is always on. Set `"memory-search": false` under `features` to keep recall but drop the `memory_search` tool (see [Feature flags](./experimental-features#turning-off-a-whole-built-in-feature)).
- The persistent memory of [agents](./agents) that declare a `memory` scope (sub-agents, and the agent chosen with the `agent` setting) is always on. To keep an agent from remembering, leave `memory` out of its definition.

## Related

- [Learning](./learning): lessons offered after a run, `/check`, structured decisions.
- [Instructions files](./instructions-files): rules the agent always reads.
- [Feature flags](./experimental-features): every flag mentioned here.
