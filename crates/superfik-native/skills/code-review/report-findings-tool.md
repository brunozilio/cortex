# Reporting findings

Superfik shows a review's results on the canvas as the agent's final
message; there is no separate tool to report them. Every level reports the
same way, in text, unless its own instructions give a stricter format.

Write the findings once, in the final message, as a numbered list ranked
most-severe first (correctness before cleanup). If nothing survived, say
`No findings.` Each entry:

```
1. path/to/file.ext:123 — one-sentence statement of the defect
   Failure: concrete inputs/state → wrong output/crash
```

- **File and line.** The repository-relative path and the 1-indexed line the
  finding anchors to.
- **Summary.** One sentence stating the defect: the claim, not its rationale.
- **Failure.** The concrete inputs or state and the wrong output or crash.
  For a cleanup finding, the concrete cost instead (what is duplicated,
  wasted, harder to maintain, or which instruction file's rule is broken).
- **Category** (optional, in brackets after the location): a short
  kebab-case slug of the finding type, such as `correctness`,
  `simplification`, `efficiency`, `test-coverage`.
- **Verdict** (only when a verify pass ran): `CONFIRMED` or `PLAUSIBLE`,
  beside the category.

Report at most 32 findings in any case.

When fixes were applied after the review (`--fix`), repeat the list once
more after applying them, each entry ending with its outcome: `fixed`,
`skipped` (with the reason) or `no change needed`.
