#!/usr/bin/env python3
"""Run trigger evaluation for a skill description.

Tests whether a skill's description causes the agent to trigger (read the
skill) for a set of queries. Outputs results as JSON.

The agent runs through a headless agent command the user configures (the
SKILL_EVAL_AGENT environment variable or --agent-command; see utils.py).
"""

import argparse
import json
import os
import select
import shutil
import subprocess
import sys
import time
import uuid
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

from scripts.utils import AGENT_HELP, agent_command, parse_skill_md

def find_project_root() -> Path:
    """Find the project root by walking up from cwd.

    The first directory holding `.superfik/` (else `.git`) is the project, so
    the skill we create ends up where the agent looks for the project's
    skills.
    """
    current = Path.cwd()
    for marker in (".superfik", ".git"):
        for parent in [current, *current.parents]:
            if (parent / marker).exists():
                return parent
    return current

def _tool_calls(event) -> list[tuple[str, dict]]:
    """Every (name, input) tool call found anywhere in a JSON event."""
    calls: list[tuple[str, dict]] = []
    if isinstance(event, dict):
        kind = event.get("type")
        if kind in ("tool_use", "tool_call", "function_call") and "name" in event:
            raw = event.get("input", event.get("arguments", {}))
            if isinstance(raw, str):
                try:
                    raw = json.loads(raw)
                except json.JSONDecodeError:
                    raw = {"raw": raw}
            calls.append((str(event.get("name", "")), raw if isinstance(raw, dict) else {}))
        for value in event.values():
            calls.extend(_tool_calls(value))
    elif isinstance(event, list):
        for value in event:
            calls.extend(_tool_calls(value))
    return calls

def run_single_query(
    query: str,
    skill_name: str,
    skill_description: str,
    timeout: int,
    project_root: str,
    model: str | None = None,
    command: str | None = None,
) -> bool:
    """Run a single query and return whether the skill was triggered.

    Creates a project skill in .superfik/skills/ so it appears in the agent's
    list of skills, then runs the headless agent command with the raw query.
    Reads its output as JSON lines when it prints them: the first tool call
    decides (a Skill or Read call naming the test skill is a trigger, any
    other tool is not). Output that is not JSON counts as a trigger when it
    names the test skill, which only appears there if the agent used it.
    """
    unique_id = uuid.uuid4().hex[:8]
    clean_name = f"{skill_name}-skill-{unique_id}"
    skill_dir = Path(project_root) / ".superfik" / "skills" / clean_name
    skill_file = skill_dir / "SKILL.md"

    try:
        skill_dir.mkdir(parents=True, exist_ok=True)
        indented_desc = "\n  ".join(skill_description.split("\n"))
        skill_content = (
            f"---\n"
            f"name: {clean_name}\n"
            f"description: |\n"
            f"  {indented_desc}\n"
            f"---\n\n"
            f"# {skill_name}\n\n"
            f"This skill handles: {skill_description}\n"
        )
        skill_file.write_text(skill_content)

        argv, stdin_text = agent_command(query, model, command)
        process = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE if stdin_text is not None else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=project_root,
        )
        if stdin_text is not None and process.stdin:
            process.stdin.write(stdin_text.encode("utf-8"))
            process.stdin.close()

        start_time = time.time()
        buffer = ""
        plain_text = ""
        pending_tool_name = None
        accumulated_json = ""

        def decide_line(line: str):
            """True/False once a line settles it, None to keep reading."""
            nonlocal pending_tool_name, accumulated_json, plain_text
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                plain_text += line + "\n"
                return None

            if isinstance(event, dict) and event.get("type") == "stream_event":
                se = event.get("event", {})
                se_type = se.get("type", "")
                if se_type == "content_block_start":
                    cb = se.get("content_block", {})
                    if cb.get("type") == "tool_use":
                        tool_name = cb.get("name", "")
                        if tool_name in ("Skill", "Read"):
                            pending_tool_name = tool_name
                            accumulated_json = ""
                        else:
                            return False
                elif se_type == "content_block_delta" and pending_tool_name:
                    delta = se.get("delta", {})
                    if delta.get("type") == "input_json_delta":
                        accumulated_json += delta.get("partial_json", "")
                        if clean_name in accumulated_json:
                            return True
                elif se_type in ("content_block_stop", "message_stop"):
                    if pending_tool_name:
                        return clean_name in accumulated_json
                    if se_type == "message_stop":
                        return False
                return None

            for tool_name, tool_input in _tool_calls(event):
                if tool_name in ("Skill", "Read"):
                    return clean_name in json.dumps(tool_input)
                return False

            if isinstance(event, dict) and event.get("type") == "result":
                return False
            return None

        try:
            while time.time() - start_time < timeout:
                if process.poll() is not None:
                    remaining = process.stdout.read()
                    if remaining:
                        buffer += remaining.decode("utf-8", errors="replace")
                    break

                ready, _, _ = select.select([process.stdout], [], [], 1.0)
                if not ready:
                    continue

                chunk = os.read(process.stdout.fileno(), 8192)
                if not chunk:
                    break
                buffer += chunk.decode("utf-8", errors="replace")

                while "\n" in buffer:
                    line, buffer = buffer.split("\n", 1)
                    line = line.strip()
                    if not line:
                        continue
                    decided = decide_line(line)
                    if decided is not None:
                        return decided

            for line in buffer.split("\n"):
                line = line.strip()
                if line:
                    decided = decide_line(line)
                    if decided is not None:
                        return decided
            return clean_name in plain_text
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    finally:
        if skill_dir.exists():
            shutil.rmtree(skill_dir, ignore_errors=True)

def run_eval(
    eval_set: list[dict],
    skill_name: str,
    description: str,
    num_workers: int,
    timeout: int,
    project_root: Path,
    runs_per_query: int = 1,
    trigger_threshold: float = 0.5,
    model: str | None = None,
    command: str | None = None,
) -> dict:
    """Run the full eval set and return results."""
    agent_command("check", model, command)
    results = []

    with ProcessPoolExecutor(max_workers=num_workers) as executor:
        future_to_info = {}
        for item in eval_set:
            for run_idx in range(runs_per_query):
                future = executor.submit(
                    run_single_query,
                    item["query"],
                    skill_name,
                    description,
                    timeout,
                    str(project_root),
                    model,
                    command,
                )
                future_to_info[future] = (item, run_idx)

        query_triggers: dict[str, list[bool]] = {}
        query_items: dict[str, dict] = {}
        for future in as_completed(future_to_info):
            item, _ = future_to_info[future]
            query = item["query"]
            query_items[query] = item
            if query not in query_triggers:
                query_triggers[query] = []
            try:
                query_triggers[query].append(future.result())
            except Exception as e:
                print(f"Warning: query failed: {e}", file=sys.stderr)
                query_triggers[query].append(False)

    for query, triggers in query_triggers.items():
        item = query_items[query]
        trigger_rate = sum(triggers) / len(triggers)
        should_trigger = item["should_trigger"]
        if should_trigger:
            did_pass = trigger_rate >= trigger_threshold
        else:
            did_pass = trigger_rate < trigger_threshold
        results.append({
            "query": query,
            "should_trigger": should_trigger,
            "trigger_rate": trigger_rate,
            "triggers": sum(triggers),
            "runs": len(triggers),
            "pass": did_pass,
        })

    passed = sum(1 for r in results if r["pass"])
    total = len(results)

    return {
        "skill_name": skill_name,
        "description": description,
        "results": results,
        "summary": {
            "total": total,
            "passed": passed,
            "failed": total - passed,
        },
    }

def main():
    parser = argparse.ArgumentParser(description="Run trigger evaluation for a skill description")
    parser.add_argument("--eval-set", required=True, help="Path to eval set JSON file")
    parser.add_argument("--skill-path", required=True, help="Path to skill directory")
    parser.add_argument("--description", default=None, help="Override description to test")
    parser.add_argument("--num-workers", type=int, default=10, help="Number of parallel workers")
    parser.add_argument("--timeout", type=int, default=30, help="Timeout per query in seconds")
    parser.add_argument("--runs-per-query", type=int, default=3, help="Number of runs per query")
    parser.add_argument("--trigger-threshold", type=float, default=0.5, help="Trigger rate threshold")
    parser.add_argument("--model", default=None, help="Model for the headless agent (default: its configured model)")
    parser.add_argument("--agent-command", default=None, help="Headless agent command (default: $SKILL_EVAL_AGENT)")
    parser.add_argument("--verbose", action="store_true", help="Print progress to stderr")
    args = parser.parse_args()

    eval_set = json.loads(Path(args.eval_set).read_text())
    skill_path = Path(args.skill_path)

    if not (skill_path / "SKILL.md").exists():
        print(f"Error: No SKILL.md found at {skill_path}", file=sys.stderr)
        sys.exit(1)

    try:
        agent_command("check", args.model, args.agent_command)
    except RuntimeError:
        print(f"Error: {AGENT_HELP}", file=sys.stderr)
        sys.exit(1)

    name, original_description, content = parse_skill_md(skill_path)
    description = args.description or original_description
    project_root = find_project_root()

    if args.verbose:
        print(f"Evaluating: {description}", file=sys.stderr)

    output = run_eval(
        eval_set=eval_set,
        skill_name=name,
        description=description,
        num_workers=args.num_workers,
        timeout=args.timeout,
        project_root=project_root,
        runs_per_query=args.runs_per_query,
        trigger_threshold=args.trigger_threshold,
        model=args.model,
        command=args.agent_command,
    )

    if args.verbose:
        summary = output["summary"]
        print(f"Results: {summary['passed']}/{summary['total']} passed", file=sys.stderr)
        for r in output["results"]:
            status = "PASS" if r["pass"] else "FAIL"
            rate_str = f"{r['triggers']}/{r['runs']}"
            print(f"  [{status}] rate={rate_str} expected={r['should_trigger']}: {r['query'][:70]}", file=sys.stderr)

    print(json.dumps(output, indent=2))

if __name__ == "__main__":
    main()
