"""Seeded task-pack generation (deterministic, generator-driven).

The v0.1 pack is NOT hand-written: one seeded RNG per family expands six
defect templates into a parameterised pack (>= 100 tasks). Determinism is a
contract: the same (seed, per_family) MUST byte-reproduce both surfaces --
`test_generator_determinism` pins it.

Answer-leak discipline: the defect line is assembled with a temporary
``@@DEFECT@@`` marker, located, and the marker is STRIPPED before the fixture
is published -- the public surface carries no family hint, and the decoy
lines carry no benign tag either (a grep for "benign" must not be an answer
key). The only family-bearing rows live in answers.jsonl, which embeds the
isolation canary.
"""
from __future__ import annotations

import json
import random
from pathlib import Path

from .families import DEFECT_TOKEN, build_fixture
from .pack import ANSWERS_NAME, CANARY, FAMILIES, PROMPT, TASKS_NAME, Pack, load_pack


def generate_pack(out_dir: Path, *, per_family: int = 20, seed: int = 20261115) -> Pack:
    """Generate and WRITE a pack: tasks.jsonl + answers.jsonl under out_dir.

    Deterministic: same seed + per_family -> identical bytes. Levels alternate
    (odd index = level 2 with decoys, even = level 1 bare) so the pack mixes
    bare and decoyed fixtures.
    """
    out_dir.mkdir(parents=True, exist_ok=True)
    task_rows: list[dict] = []
    answer_rows: list[dict] = []
    for family in FAMILIES:
        rng = random.Random(f"{seed}:{family}")
        for index in range(per_family):
            level = 2 if index % 2 else 1
            fixture, defect_line = build_fixture(rng, family, level)
            task_id = f"{family}-{index:03d}"
            # the assembly marker must never reach the published surface
            assert DEFECT_TOKEN not in fixture, f"{task_id}: marker leaked into public surface"
            task_rows.append({"id": task_id, "level": level, "prompt": PROMPT, "fixture": fixture})
            answer_rows.append({"id": task_id, "defect_line": defect_line,
                                "family": family, "canary": CANARY})
    (out_dir / TASKS_NAME).write_text(
        "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in task_rows), encoding="utf-8")
    (out_dir / ANSWERS_NAME).write_text(
        "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in answer_rows), encoding="utf-8")
    return load_pack(out_dir)
