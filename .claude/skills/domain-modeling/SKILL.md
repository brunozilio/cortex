---
name: domain-modeling
description: Build and sharpen a project's domain model. Use when discussing codebase terminology, writing or editing docs/GLOSSARY.md, or recording or editing an ADR under docs/decisions/.
---

# Domain Modeling

Actively build and sharpen the project's domain model as you design. This is the *active* discipline: challenging terms, inventing edge-case scenarios, and writing the glossary and decisions down the moment they crystallise. (Merely *reading* `docs/GLOSSARY.md` for vocabulary is not this skill: that's a one-line habit any skill can do. This skill is for when you're changing the model, not just consuming it.)

## File structure

This repo has a single context. Both files already exist:

```
/
├── docs/
│   ├── GLOSSARY.md                   ← the project's vocabulary
│   └── decisions/
│       ├── 0001-long-lived-session.md
│       └── NNNN-kebab-title.md       ← one file per decision
└── crates/
```

A new term goes into `docs/GLOSSARY.md`; a new decision gets its own file under `docs/decisions/`, numbered one above the highest existing number.

Files under `docs/` must not spell the product name. Write "the app" for the product, `<command>` for its binary, `.<command>/` for its project folder and `~/.<command>/` for its user folder.

## During the session

### Challenge against the glossary

When the user uses a term that conflicts with the existing language in `docs/GLOSSARY.md`, call it out immediately. "Your glossary defines 'cancellation' as X, but you seem to mean Y. Which is it?"

### Sharpen fuzzy language

When the user uses vague or overloaded terms, propose a precise canonical term. "You're saying 'account': do you mean the Customer or the User? Those are different things."

### Discuss concrete scenarios

When domain relationships are being discussed, stress-test them with specific scenarios. Invent scenarios that probe edge cases and force the user to be precise about the boundaries between concepts.

### Cross-reference with code

When the user states how something works, check whether the code agrees. If you find a contradiction, surface it: "Your code cancels entire Orders, but you just said partial cancellation is possible. Which is right?"

### Update the glossary inline

When a term is resolved, update `docs/GLOSSARY.md` right there. Don't batch these up: capture them as they happen. Use the format in [GLOSSARY-FORMAT.md](./GLOSSARY-FORMAT.md).

An entry says what the term means and names the main place it lives in the code; nothing more. Do not treat `docs/GLOSSARY.md` as a spec, a scratch pad, or a repository for implementation decisions. It is a glossary and nothing else; decisions go to `docs/decisions/`.

### Offer ADRs sparingly

Only offer to create an ADR when all three are true:

1. **Hard to reverse**: the cost of changing your mind later is meaningful
2. **Surprising without context**: a future reader will wonder "why did they do it this way?"
3. **The result of a real trade-off**: there were genuine alternatives and you picked one for specific reasons

If any of the three is missing, skip the ADR. Otherwise add `docs/decisions/NNNN-kebab-title.md` using the format in [ADR-FORMAT.md](./ADR-FORMAT.md).
