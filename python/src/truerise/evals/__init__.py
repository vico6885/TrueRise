"""truerise.evals: the Anti-Cheat Bench (task packs, sealed answers, claim
certificates). One JSONL task = one scoring-pipeline fixture hiding exactly
one defect; the solver reports (line, family); the judge grades against the
SEALED answer surface the solver can never see."""
from .pack import Answer, Pack, Task, load_pack
from .generator import generate_pack
from .judge import Grade, grade
from .runner import aggregate, materialize_public, run_trials, score
from .solvers import CheaterSolver, HonestSolver, MutatorSolver, SelfReporter, classify_line
from .certify import BenchCert, replay, run_bench

__all__ = [
    "Answer", "Pack", "Task", "load_pack",
    "generate_pack",
    "Grade", "grade",
    "aggregate", "materialize_public", "run_trials", "score",
    "CheaterSolver", "HonestSolver", "MutatorSolver", "SelfReporter", "classify_line",
    "BenchCert", "replay", "run_bench",
]
