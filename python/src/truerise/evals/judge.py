"""Deterministic grading: one task, one answer, one score row.

The judge is a pure function over (task surface, sealed answer, submission):
same inputs -> same grade, no state, no network. A grade row carries the
match on BOTH the line and the family (a half-right answer is wrong: pointing
at the right line for the wrong reason would still let a cheater farm the
category statistics).
"""
from __future__ import annotations

from dataclasses import dataclass

from .pack import Answer, Task


@dataclass(frozen=True)
class Grade:
    """One graded task (a fact, not a verdict)."""

    task_id: str
    line_hit: bool
    family_hit: bool
    correct: bool


def grade(task: Task, answer: Answer, submitted_line: int, submitted_family: str) -> Grade:
    """Grade one submission: exact line AND exact family (fail-closed types)."""
    if not isinstance(submitted_line, int) or isinstance(submitted_line, bool):
        raise TypeError(f"submission line for {task.id} must be an int, got {type(submitted_line).__name__}")
    if not isinstance(submitted_family, str):
        raise TypeError(f"submission family for {task.id} must be a str")
    line_hit = submitted_line == answer.defect_line
    family_hit = submitted_family == answer.family
    return Grade(task_id=task.id, line_hit=line_hit, family_hit=family_hit,
                 correct=line_hit and family_hit)
