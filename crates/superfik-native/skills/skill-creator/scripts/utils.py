"""Shared utilities for skill-creator scripts."""

import os
import shlex
import subprocess
from pathlib import Path

AGENT_ENV = "SKILL_EVAL_AGENT"

AGENT_HELP = (
    f"No headless agent command is configured. Set {AGENT_ENV} (or pass "
    "--agent-command) to a command that runs one prompt non-interactively in "
    "the current directory, with the project's skills available, and prints "
    "its output. Use {prompt} where the prompt goes (otherwise it is sent on "
    "stdin) and, optionally, {model} where the model id goes. For trigger "
    "evaluation the output should be JSON lines with the agent's tool calls "
    "(see the Description Optimization section of SKILL.md)."
)

def agent_command(prompt: str, model: str | None, command: str | None = None) -> tuple[list[str], str | None]:
    """Build the headless agent invocation for a prompt.

    Returns (argv, stdin_text). `command` overrides the AGENT_ENV variable.
    Raises RuntimeError with setup instructions when none is configured.
    """
    template = command or os.environ.get(AGENT_ENV, "").strip()
    if not template:
        raise RuntimeError(AGENT_HELP)
    argv: list[str] = []
    prompt_in_args = False
    for part in shlex.split(template):
        if "{prompt}" in part:
            prompt_in_args = True
            part = part.replace("{prompt}", prompt)
        if "{model}" in part:
            if not model:
                if part == "{model}":
                    if argv and argv[-1].startswith("-"):
                        argv.pop()
                    continue
                part = part.replace("{model}", "")
            else:
                part = part.replace("{model}", model)
        argv.append(part)
    return argv, (None if prompt_in_args else prompt)

def run_agent_text(prompt: str, model: str | None, command: str | None = None, timeout: int = 300) -> str:
    """Run the headless agent once and return what it printed."""
    argv, stdin_text = agent_command(prompt, model, command)
    result = subprocess.run(
        argv,
        input=stdin_text if stdin_text is not None else "",
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"{argv[0]} exited {result.returncode}\nstderr: {result.stderr}"
        )
    return result.stdout

def parse_skill_md(skill_path: Path) -> tuple[str, str, str]:
    """Parse a SKILL.md file, returning (name, description, full_content)."""
    content = (skill_path / "SKILL.md").read_text()
    lines = content.split("\n")

    if lines[0].strip() != "---":
        raise ValueError("SKILL.md missing frontmatter (no opening ---)")

    end_idx = None
    for i, line in enumerate(lines[1:], start=1):
        if line.strip() == "---":
            end_idx = i
            break

    if end_idx is None:
        raise ValueError("SKILL.md missing frontmatter (no closing ---)")

    name = ""
    description = ""
    frontmatter_lines = lines[1:end_idx]
    i = 0
    while i < len(frontmatter_lines):
        line = frontmatter_lines[i]
        if line.startswith("name:"):
            name = line[len("name:"):].strip().strip('"').strip("'")
        elif line.startswith("description:"):
            value = line[len("description:"):].strip()
            if value in (">", "|", ">-", "|-"):
                continuation_lines: list[str] = []
                i += 1
                while i < len(frontmatter_lines) and (frontmatter_lines[i].startswith("  ") or frontmatter_lines[i].startswith("\t")):
                    continuation_lines.append(frontmatter_lines[i].strip())
                    i += 1
                description = " ".join(continuation_lines)
                continue
            else:
                description = value.strip('"').strip("'")
        i += 1

    return name, description, content
