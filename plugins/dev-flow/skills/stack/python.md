# Python services

## Build and verify

Find the tool from the repository: `uv.lock` (uv), `poetry.lock`
(Poetry), otherwise a virtualenv with `requirements*.txt`. Prefix the
commands accordingly (`uv run`, `poetry run`).

| Step | Command |
|---|---|
| Fast loop | `pytest tests/test_refunds.py -q` |
| Tests | `pytest -q` |
| Lint and format | `ruff check .` and `ruff format --check .` (or the project's `flake8`/`black`) |
| Types | `mypy .` or `pyright`, when configured |

Run every step the CI workflow runs. A new dependency is added with the
tool (`uv add`, `poetry add`) so the lock file changes with it, pinned
like its neighbours.

## Code

- Type hints on every new function; data crossing a boundary is a typed
  model (Pydantic or dataclass), not a bare dict.
- Keep handlers thin (Lambda handler, FastAPI or Flask route): parse,
  call the service function, map the result. The rule lives in a
  function a test can call without the framework.
- Configuration comes from the environment through the existing settings
  module; no credentials in code. AWS calls go through `boto3` clients
  created once and passed in, so tests can replace them.
- Raise specific exceptions and translate them at the edge; never a
  bare `except:` and never swallow an error silently.
- Lambda code: the handler is idempotent (events are delivered more than
  once), heavy clients are created outside the handler, and the timeout
  and memory live in Terraform, not in comments.

## Tests

- `pytest` with fixtures over setup methods; `moto` or the project's
  stubs for AWS, never the real account.
- Test behaviour through the service function or the HTTP client
  (`TestClient`), with one assertion subject per test.
- API acceptance scenarios for a Python service still live in the
  team's Cucumber suite when that is where the others are (`dev-flow:test-automation`).
