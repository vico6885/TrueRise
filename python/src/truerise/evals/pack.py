"""Task pack protocol: the public task surface and the sealed answer surface.

A pack directory contains exactly two JSONL files:

    tasks.jsonl   PUBLIC. One task per row: id, level, prompt, fixture.
                  The task carries NO family field -- the family is part of
                  the sealed answer; publishing it would leak the answer.
    answers.jsonl SEALED. One answer per row: id, defect_line, family.

The answer file never ships next to the solver: the runner materialises the
public surface into a fresh directory and hands the solver a PATH to that
copy only. A canary token is embedded in answers.jsonl; the runner refuses
to grade a run whose public-surface scan finds the canary (structural answer
isolation, checked before any grading).

The sealed family taxonomy (the bench's defect classification):

    future_info_use         decision at t uses information only available after t
    full_sample_scaling     a statistic computed over the FULL sample is applied
                            inside the scoring window (self-influencing)
    backward_fill_leak      a later value is back-filled into earlier slots
    same_bar_execution      a fill is assumed on the same bar the decision uses
    sample_selection_bias   rows are dropped by a rule correlated with the
                            outcome being scored
    unit_consistency        two quantities enter one comparison in different
                            units / calendars without conversion

Rows are validated: ids unique and aligned across both surfaces, defect_line
inside the fixture line range, family in the taxonomy.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path

FAMILIES: tuple[str, ...] = (
    "future_info_use",
    "full_sample_scaling",
    "backward_fill_leak",
    "same_bar_execution",
    "sample_selection_bias",
    "unit_consistency",
)

CANARY = "truerise-evals-canary-sealed-surface"
TASKS_NAME = "tasks.jsonl"
ANSWERS_NAME = "answers.jsonl"

PROMPT = (
    "The function below is part of a scoring pipeline over a daily series. "
    "Exactly ONE line introduces a defect from one of these families: "
    + ", ".join(FAMILIES)
    + ". Report JSON: {\"line\": <1-based line number>, \"family\": \"<family>\"}."
)


@dataclass(frozen=True)
class Task:
    """One public task: the solver may see everything here, nothing else."""

    id: str
    level: int
    prompt: str
    fixture: str  # python-like code, line numbers are 1-based as displayed

    def line_count(self) -> int:
        return len(self.fixture.splitlines())


@dataclass(frozen=True)
class Answer:
    """One sealed answer (judge-side only)."""

    id: str
    defect_line: int
    family: str


@dataclass(frozen=True)
class Pack:
    """A validated pack: public tasks + sealed answers + digests."""

    root: Path
    tasks: tuple[Task, ...]
    answers: tuple[Answer, ...]
    pack_digest: str
    answer_seal: str

    def answer_map(self) -> dict[str, Answer]:
        return {a.id: a for a in self.answers}


def _read_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    for i, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:  # fail closed: a broken row poisons the pack
            raise ValueError(f"{path.name}:{i + 1}: invalid JSON row ({exc})") from exc
    return rows


def load_pack(root: Path) -> Pack:
    """Load and validate a pack directory (fail-closed on any inconsistency)."""
    tasks_path, answers_path = root / TASKS_NAME, root / ANSWERS_NAME
    if not tasks_path.exists() or not answers_path.exists():
        raise FileNotFoundError(f"pack needs {TASKS_NAME} + {ANSWERS_NAME} under {root}")
    tasks = tuple(
        Task(id=r["id"], level=int(r.get("level", 1)), prompt=r["prompt"], fixture=r["fixture"])
        for r in _read_rows(tasks_path)
    )
    answers = tuple(
        Answer(id=r["id"], defect_line=int(r["defect_line"]), family=r["family"])
        for r in _read_rows(answers_path)
    )
    _validate(tasks, answers)
    pack_digest = hashlib.sha256(tasks_path.read_bytes()).hexdigest()
    answer_seal = hashlib.sha256(answers_path.read_bytes()).hexdigest()
    return Pack(root=root, tasks=tasks, answers=answers,
                pack_digest=pack_digest, answer_seal=answer_seal)


def _validate(tasks: tuple[Task, ...], answers: tuple[Answer, ...]) -> None:
    task_ids = [t.id for t in tasks]
    answer_ids = [a.id for a in answers]
    if len(set(task_ids)) != len(task_ids):
        raise ValueError("duplicate task ids in tasks.jsonl")
    if sorted(task_ids) != sorted(answer_ids):
        raise ValueError("task/answer id sets differ (both surfaces must align)")
    for t in tasks:
        if PROMPT[:40] not in t.prompt:
            raise ValueError(f"task {t.id}: prompt is not the canonical bench prompt")
        if t.line_count() < 2:
            raise ValueError(f"task {t.id}: fixture too small to hide a defect")
    for a in answers:
        if a.family not in FAMILIES:
            raise ValueError(f"answer {a.id}: family {a.family!r} not in taxonomy")
    # answers must live INSIDE the fixture line range
    by_id = {t.id: t for t in tasks}
    for a in answers:
        n = by_id[a.id].line_count()
        if not 1 <= a.defect_line <= n:
            raise ValueError(f"answer {a.id}: defect_line {a.defect_line} outside 1..{n}")
