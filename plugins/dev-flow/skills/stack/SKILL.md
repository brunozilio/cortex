---
name: stack
description: The team's conventions and verification commands per technology - Java services, Python services, Terraform on AWS and Angular portals. Use before changing or verifying code in a repository of the development flow, to learn how that stack is built, tested and checked.
---

# The team's stacks

A story can cross several repositories, each on one stack. Find the
stack in `.flow/config.json` (`repos[].stack`) or from the files at the
repository's root, then read the one guide that applies, in this skill's
folder:

| Stack | Root files | Guide |
|---|---|---|
| Java service | `pom.xml`, `build.gradle(.kts)` | `java.md` |
| Python service | `pyproject.toml`, `requirements.txt` | `python.md` |
| Terraform on AWS | `*.tf` | `terraform.md` |
| Angular portal | `angular.json` | `angular.md` |

Rules for every stack:

1. The repository's own instructions, README and CI workflow say how it
   is built and tested; they win over these guides. Read
   `.github/workflows/` to learn what the pipeline will run, and run the
   same locally before a PR.
2. Use the wrapper and the versions the repository pins (`./mvnw`,
   `./gradlew`, `.python-version`, `.terraform-version`, `.nvmrc`).
   Never upgrade a dependency or a tool as a side effect of a story.
3. Follow the code that is already there: its layers, naming, error
   handling and test style.
4. "Verified" means the full command ran and passed, with its output
   read; a compile is not a test run.
5. Across repositories, change the provider of a contract before its
   consumer, and keep each repository releasable on its own (an API
   stays backward compatible until every consumer moved).
