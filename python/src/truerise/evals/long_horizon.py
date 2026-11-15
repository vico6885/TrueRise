"""Long-horizon closed-loop tasks (W16): multi-step pipelines whose FINAL
state is machine-checkable, with the completion rate as the headline metric.

A long task is a small deterministic computation over a published series:
the solver must execute EVERY step and report the full value chain. A task
counts as CLOSED-LOOP COMPLETE only when every step matches the sealed
chain - a task that lands the final number by skipping an intermediate step
is a failure, because the point is the loop, not the endpoint.

    long_tasks.jsonl    PUBLIC: id, prompt, data (ints), steps (spec list)
    long_answers.jsonl  SEALED: id, chain (one sealed value per step), canary

The isolation/canary semantics are inherited from the defect bench: the
public surface never carries the canary or the sealed chain; comparing
across environments is refused downstream by the same fingerprint rules.

Integers only (the float rule): completion travels as counts and
per-mille; sealed values are integers.
"""
from __future__ import annotations

import hashlib
import json
import random
from dataclasses import dataclass
from pathlib import Path

CANARY = "truerise-evals-canary-sealed-surface"
LONG_TASKS_NAME = "long_tasks.jsonl"
LONG_ANSWERS_NAME = "long_answers.jsonl"
LONG_PROMPT = (
    "Execute the FULL pipeline over the given series and report EVERY step "
    "value in order. A missing or wrong intermediate step fails the task "
    "even when the final value is right. Report JSON: "
    '{"id": "<task id>", "steps": [<v1>, <v2>, ...]} (integers).'
)


@dataclass(frozen=True)
class LongTask:
    """One public long-horizon task."""

    id: str
    prompt: str
    data: tuple[int, ...]
    steps: tuple[str, ...]  # step specs, e.g. "mean_window:3"

    def step_count(self) -> int:
        return len(self.steps)


@dataclass(frozen=True)
class LongAnswer:
    """One sealed chain (judge side): the expected value per step."""

    id: str
    chain: tuple[int, ...]


@dataclass(frozen=True)
class LongPack:
    """A validated long-horizon pack."""

    root: Path
    tasks: tuple[LongTask, ...]
    answers: tuple[LongAnswer, ...]
    pack_digest: str
    answer_seal: str


def _step_value(spec: str, data: tuple[int, ...], prior: list[int]) -> int:
    """Execute one step spec (deterministic; integers only)."""
    kind, _, arg = spec.partition(":")
    if kind == "mean_window":
        k = int(arg)
        window = data[-k:] if len(data) >= k else data
        return sum(window) // max(len(window), 1)
    if kind == "count_above":
        threshold = int(arg)
        return sum(1 for v in data if v > threshold)
    if kind == "delta":
        # delta between the last data point and the previous STEP value
        return data[-1] - prior[-1]
    if kind == "scale":
        factor = int(arg)
        return prior[-1] * factor
    raise ValueError(f"unknown step spec: {spec!r}")


def _make_task(rng: random.Random, index: int) -> tuple[LongTask, LongAnswer]:
    n = rng.randint(6, 10)
    data = tuple(rng.randint(-20, 20) for _ in range(n))
    steps: list[str] = [f"mean_window:{rng.randint(2, 4)}"]
    steps.append(f"count_above:{rng.randint(-5, 5)}")
    steps.append("delta:0")
    steps.append(f"scale:{rng.randint(2, 3)}")
    task = LongTask(
        id=f"long-{index:03d}",
        prompt=LONG_PROMPT,
        data=data,
        steps=tuple(steps),
    )
    chain: list[int] = []
    prior: list[int] = [data[0]]
    for spec in task.steps:
        value = _step_value(spec, data, prior)
        chain.append(value)
        prior.append(value)
    return task, LongAnswer(id=task.id, chain=tuple(chain))


def generate_long_horizon_pack(out_dir: Path, *, chains: int = 6, seed: int = 20261116) -> LongPack:
    """Generate a deterministic long-horizon pack (same seed = same bytes)."""
    rng = random.Random(seed)
    tasks: list[LongTask] = []
    answers: list[LongAnswer] = []
    for i in range(chains):
        task, answer = _make_task(rng, i)
        tasks.append(task)
        answers.append(answer)
    out_dir.mkdir(parents=True, exist_ok=True)
    task_rows = "".join(
        json.dumps(
            {"id": t.id, "prompt": t.prompt, "data": list(t.data), "steps": list(t.steps)},
            sort_keys=True,
        )
        + "\n"
        for t in tasks
    )
    answer_rows = "".join(
        json.dumps({"id": a.id, "chain": list(a.chain), "canary": CANARY}, sort_keys=True) + "\n"
        for a in answers
    )
    (out_dir / LONG_TASKS_NAME).write_text(task_rows, encoding="utf-8")
    (out_dir / LONG_ANSWERS_NAME).write_text(answer_rows, encoding="utf-8")
    return load_long_horizon_pack(out_dir)


def load_long_horizon_pack(root: Path) -> LongPack:
    """Load + validate (fail-closed on misalignment)."""
    tasks_path, answers_path = root / LONG_TASKS_NAME, root / LONG_ANSWERS_NAME
    tasks = tuple(
        LongTask(
            id=r["id"],
            prompt=r["prompt"],
            data=tuple(r["data"]),
            steps=tuple(r["steps"]),
        )
        for r in _rows(tasks_path)
    )
    answers = tuple(
        LongAnswer(id=r["id"], chain=tuple(r["chain"])) for r in _rows(answers_path)
    )
    if [t.id for t in tasks] != [a.id for a in answers]:
        raise ValueError("long pack: task/answer id alignment broken")
    for a in answers:
        task = next(t for t in tasks if t.id == a.id)
        if len(a.chain) != task.step_count():
            raise ValueError(f"long pack: {a.id} chain length != step count")
    return LongPack(
        root=root,
        tasks=tasks,
        answers=answers,
        pack_digest=hashlib.sha256(tasks_path.read_bytes()).hexdigest(),
        answer_seal=hashlib.sha256(answers_path.read_bytes()).hexdigest(),
    )


def _rows(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def execute_steps(task: LongTask, submitted: list[int]) -> tuple[bool, int]:
    """Grade one submission: closed-loop complete iff EVERY step matches.

    Returns (complete, steps_passed). Partial credit is REPORTED but never
    converts into completion.
    """
    expected: list[int] = []
    prior: list[int] = [task.data[0]]
    for spec in task.steps:
        value = _step_value(spec, task.data, prior)
        expected.append(value)
        prior.append(value)
    passed = sum(1 for e, s in zip(expected, submitted) if e == s)
    return passed == len(expected), passed


def run_long_horizon(solve, pack: LongPack, work_dir: Path) -> dict:
    """Run a solver over the long pack; the runner mediates and grades.

    ``solve(task) -> list[int]`` (the submitted step values). Returns the
    summary (integers only: counts + per-mille) - the caller records it.
    """
    work_dir.mkdir(parents=True, exist_ok=True)
    completed = 0
    steps_passed = 0
    steps_total = 0
    failures: list[dict] = []
    for task in pack.tasks:
        submitted = solve(task)
        if len(submitted) != task.step_count():
            failures.append({"task_id": task.id, "reason": "wrong step count"})
            steps_total += task.step_count()
            continue
        complete, passed = execute_steps(task, submitted)
        completed += int(complete)
        steps_passed += passed
        steps_total += task.step_count()
        if not complete:
            failures.append({"task_id": task.id, "steps_passed": passed})
    return {
        "completed": completed,
        "total": len(pack.tasks),
        "steps_passed": steps_passed,
        "steps_total": steps_total,
        "steps_passed_per_mille": (steps_passed * 1000) // max(steps_total, 1),
        "failures": failures,
    }
