"""External-solver protocol (W11a): a solver that is NOT part of this
repository runs as a SUBPROCESS through the runner.

Contract (the only thing an external solver sees and must honour):

    env  TRUERISE_PUBLIC_DIR   path to the PUBLIC surface (tasks.jsonl only)
    env  TRUERISE_OUT_DIR      path where the solver writes submission.jsonl

submission.jsonl rows: {"id": <task id>, "line": <int>, "family": <str>}

The runner grades the submission and SIGNS every grade record itself (the
runner is the executor) - a solver's own accuracy claims are self-report
and cap at L0. The protocol exists so the first third-party model reading
needs zero new harness code: point --solver-cmd at any program honouring
this contract.

This module is ALSO a reference implementation of that contract (a scripted
rule-based solver) used by the protocol tests; a real external model
replaces it by being any executable honouring the same env contract.
"""
from __future__ import annotations

import json
import os
from pathlib import Path

from .pack import TASKS_NAME, Task


def _load_tasks(public_dir: Path) -> list[Task]:
    rows = [json.loads(line) for line in (public_dir / TASKS_NAME).read_text(encoding="utf-8").splitlines() if line.strip()]
    return [Task(id=r["id"], level=int(r.get("level", 1)), prompt=r["prompt"], fixture=r["fixture"]) for r in rows]


def classify_line(line: str) -> str | None:
    """The single-source-of-truth line classifier (shared with the bench)."""
    from .solvers import classify_line as _cls

    return _cls(line)


def main() -> int:
    """The reference external solver: read the public surface, submit."""
    public_dir = Path(os.environ["TRUERISE_PUBLIC_DIR"])
    out_dir = Path(os.environ["TRUERISE_OUT_DIR"])
    out_dir.mkdir(parents=True, exist_ok=True)
    rows = []
    for task in _load_tasks(public_dir):
        hit = None
        for i, line in enumerate(task.fixture.splitlines(), start=1):
            family = classify_line(line)
            if family is not None:
                hit = (i, family)
                break
        line, family = hit if hit else (1, "future_info_use")
        rows.append({"id": task.id, "line": line, "family": family})
    (out_dir / "submission.jsonl").write_text(
        "".join(json.dumps(r, sort_keys=True) + "\n" for r in rows), encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
