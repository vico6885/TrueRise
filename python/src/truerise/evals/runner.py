"""Bench runner: materialises the public surface, mediates trials, grades.

Answer isolation is structural, not advisory: the runner copies tasks.jsonl
into a fresh directory and hands the solver that PATH ONLY. answers.jsonl
never leaves the pack directory, and the public surface is scanned for the
seal canary BEFORE any grading (a leak is fail-closed, not a warning).

Trial mediation: the solver cannot run its own hidden trials -- every trial
goes through `run_trials`, so the trial count the certificate reports is the
runner's count. Selecting after the fact (majority / best-of) aggregates the
SAME counted trials; it cannot un-count them.
"""
from __future__ import annotations

import hashlib
import json
import os
import shlex
import shutil
import subprocess
from collections import Counter
from pathlib import Path

from .judge import Grade, grade
from .pack import ANSWERS_NAME, CANARY, TASKS_NAME, Pack
from .solvers import Solver

EXECUTOR_ID = "truerise-runner"


def executor_signature(task_id: str, line_hit: bool, family_hit: bool) -> str:
    """The executor's signature over one grade record (W11a): the runner
    signs what IT observed - a solver's own claims are self-report (L0)."""
    payload = f"{EXECUTOR_ID}|{task_id}|{int(line_hit)}|{int(family_hit)}".encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def materialize_public(pack: Pack, work_dir: Path) -> Path:
    """Copy the public surface into work_dir and fail-closed on canary leaks."""
    work_dir.mkdir(parents=True, exist_ok=True)
    public = work_dir / "public"
    public.mkdir(exist_ok=True)
    shutil.copy2(pack.root / TASKS_NAME, public / TASKS_NAME)
    # fail closed: NO sealed content may sit next to the solver
    for path in public.rglob("*"):
        if path.is_file() and CANARY in path.read_text(encoding="utf-8", errors="replace"):
            raise RuntimeError(f"answer-isolation breach: canary found in {path}")
    if (public / ANSWERS_NAME).exists():
        raise RuntimeError("answer-isolation breach: answers.jsonl materialised as public surface")
    return public


def run_trials(solver: Solver, pack: Pack, work_dir: Path, trials: int) -> list[dict[str, Grade]]:
    """Run ``trials`` full passes; every pass counts (monotonic trial ledger)."""
    if trials < 1:
        raise ValueError("trials must be >= 1 (zero-trial runs are not runs)")
    _ = materialize_public(pack, work_dir)
    answers = pack.answer_map()
    passes: list[dict[str, Grade]] = []
    for _ in range(trials):
        row: dict[str, Grade] = {}
        for task in pack.tasks:
            line, family = solver.solve(task)
            row[task.id] = grade(task, answers[task.id], line, family)
        passes.append(row)
    return passes


def aggregate(passes: list[dict[str, Grade]], strategy: str) -> dict[str, Grade]:
    """Reduce the counted trials to one grade per task.

    last      single-trial semantics (the honest default)
    majority  per-task vote across trials -- the CHEATER's aggregation; the
              vote happens AFTER all trials were counted
    """
    if strategy == "last":
        return dict(passes[-1])
    if strategy == "majority":
        out: dict[str, Grade] = {}
        for task_id in passes[0]:
            votes: Counter[tuple[bool, bool]] = Counter()
            for p in passes:
                g = p[task_id]
                votes[(g.line_hit, g.family_hit)] += 1
            (line_hit, family_hit), _ = votes.most_common(1)[0]
            out[task_id] = Grade(task_id=task_id, line_hit=line_hit,
                                 family_hit=family_hit, correct=line_hit and family_hit)
        return out
    raise ValueError(f"unknown aggregation strategy: {strategy!r}")


def score(graded: dict[str, Grade], pack: Pack) -> dict:
    """Accuracy overall + per family + the failure inventory (the miss list).

    Family attribution comes from the SEALED answers (judge side); the miss
    list carries submitted-vs-sealed rows so a failure is auditable without
    ever exposing the sealed surface to the solver."""
    answers = pack.answer_map()
    correct = sum(1 for g in graded.values() if g.correct)
    per_family: dict[str, dict[str, int]] = {}
    failures: list[dict] = []
    for task_id, g in sorted(graded.items()):
        answer = answers[task_id]
        bucket = per_family.setdefault(answer.family, {"correct": 0, "total": 0})
        bucket["total"] += 1
        bucket["correct"] += int(g.correct)
        if not g.correct:
            failures.append({
                "task_id": task_id,
                "family": answer.family,
                "sealed_line": answer.defect_line if g.line_hit else None,
                "submitted_line": None,  # filled by the caller from the run record
                "line_hit": g.line_hit,
                "family_hit": g.family_hit,
            })
    return {"correct": correct, "total": len(graded),
            "accuracy": correct / len(graded) if graded else 0.0,
            "per_family": per_family, "failures": failures}


def run_external_trials(solver_cmd: str | list[str], pack: Pack, work_dir: Path, trials: int = 1) -> tuple[list[dict[str, Grade]], list[dict]]:
    """Run an EXTERNAL solver (W11a): the subprocess sees the PUBLIC surface
    only and writes submissions; the runner grades and SIGNS every record.

    Answer isolation is the SAME structural check as the in-process path
    (the public surface is scanned for the canary BEFORE grading).

    Returns (passes, signed_records) - the records carry the executor
    signature so the evidence gate can count them at the declared level.

    Raises RuntimeError on isolation breaches / solver protocol failures.
    """
    if trials < 1:
        raise ValueError("trials must be >= 1 (zero-trial runs are not runs)")
    public = materialize_public(pack, work_dir)
    answers = pack.answer_map()
    passes: list[dict[str, Grade]] = []
    signed: list[dict] = []
    for _ in range(trials):
        out_dir = work_dir / "submissions"
        out_dir.mkdir(exist_ok=True)
        env = dict(os.environ)
        env["TRUERISE_PUBLIC_DIR"] = str(public.resolve())
        env["TRUERISE_OUT_DIR"] = str(out_dir.resolve())
        argv = shlex.split(solver_cmd) if isinstance(solver_cmd, str) else list(solver_cmd)
        proc = subprocess.run(
            argv, env=env, capture_output=True, text=True, timeout=600
        )
        if proc.returncode != 0:
            raise RuntimeError(f"external solver failed ({proc.returncode}): {proc.stderr[-400:]}")
        submission = out_dir / "submission.jsonl"
        if not submission.exists():
            raise RuntimeError("external solver protocol breach: no submission.jsonl")
        row: dict[str, Grade] = {}
        for line in submission.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            doc = json.loads(line)
            task_id = doc["id"]
            answer = answers[task_id]
            g = grade(pack.task_by_id(task_id), answer, int(doc["line"]), doc["family"])
            row[task_id] = g
            signed.append({
                "task_id": task_id,
                "line_hit": g.line_hit,
                "family_hit": g.family_hit,
                "correct": g.correct,
                "executor": EXECUTOR_ID,
                "signature": executor_signature(task_id, g.line_hit, g.family_hit),
            })
        missing = set(answers) - set(row)
        if missing:
            raise RuntimeError(f"external solver protocol breach: no submission for {sorted(missing)[:3]}")
        passes.append(row)
    return passes, signed
