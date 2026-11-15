"""Anti-Cheat Bench acceptance: determinism, answer isolation, cheat catches,
offline replay. The bench's real acceptance mirrors the demo's -- the honest
run passes through the SAME code path that fails both cheats, and every
verdict line flips with its input.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from truerise.evals import (
    CheaterSolver,
    HonestSolver,
    aggregate,
    generate_pack,
    grade,
    load_pack,
    materialize_public,
    replay,
    run_bench,
    run_trials,
    score,
)
from truerise.evals.judge import Grade
from truerise.evals.pack import CANARY, Pack
from truerise.evals.solvers import classify_line
from truerise.ledger import LedgerTampered

THRESHOLDS = {"min_accuracy": 0.8, "claim_p": 0.05}
MUTATED = {"min_accuracy": 0.5, "claim_p": 0.03}


@pytest.fixture(scope="module")
def pack(tmp_path_factory) -> Pack:
    return generate_pack(tmp_path_factory.mktemp("pack_src"), per_family=20, seed=20261115)


def test_pack_scale_and_taxonomy(pack: Pack) -> None:
    assert len(pack.tasks) >= 100  # the v0.1 publication floor
    families = {a.family for a in pack.answers}
    assert len(families) == 6
    assert not any("family" in json.loads(line) for line in
                   (pack.root / "tasks.jsonl").read_text(encoding="utf-8").splitlines()
                   if line.strip()), "public surface must not carry the family field"


def test_generator_determinism(tmp_path: Path) -> None:
    a = generate_pack(tmp_path / "a", per_family=4, seed=7)
    b = generate_pack(tmp_path / "b", per_family=4, seed=7)
    c = generate_pack(tmp_path / "c", per_family=4, seed=8)
    assert (a.pack_digest, a.answer_seal) == (b.pack_digest, b.answer_seal)
    assert a.pack_digest != c.pack_digest


def test_public_surface_carries_no_sealed_hints(pack: Pack) -> None:
    public = materialize_public(pack, Path(pack.root.parent / "work_iso"))
    text = (public / "tasks.jsonl").read_text(encoding="utf-8")
    assert CANARY not in text
    assert "@@DEFECT@@" not in text, "assembly marker must never reach the surface"
    for hint in ("FUTURE:", "BFILL:", "SAME-BAR:", "SELECTION:", "UNITS:", "APPLIED:"):
        assert hint not in text, f"family marker {hint!r} leaked into the public surface"
    assert not (public / "answers.jsonl").exists()


def test_materialize_public_fail_closed_on_canary(pack: Pack, tmp_path: Path) -> None:
    rogue = tmp_path / "rogue"
    rogue.mkdir()
    from truerise.evals.pack import ANSWERS_NAME, TASKS_NAME

    # simulate a tampered pack whose "tasks" file embeds the sealed canary
    class Tampered:
        root = rogue

        def __getattr__(self, name):
            return getattr(pack, name)

    (rogue / TASKS_NAME).write_text(
        (pack.root / TASKS_NAME).read_text(encoding="utf-8") + f'{{"c": "{CANARY}"}}\n',
        encoding="utf-8")
    (rogue / ANSWERS_NAME).write_text(
        (pack.root / ANSWERS_NAME).read_text(encoding="utf-8"), encoding="utf-8")
    with pytest.raises(RuntimeError, match="answer-isolation breach"):
        materialize_public(Tampered(), tmp_path / "w")


def test_honest_solver_full_accuracy(pack: Pack) -> None:
    passes = run_trials(HonestSolver(), pack, Path(pack.root.parent / "w_honest"), trials=1)
    graded = aggregate(passes, "last")
    summary = score(graded, pack)
    assert summary["accuracy"] == 1.0
    assert summary["failures"] == []


def test_judge_is_exact_on_both_axes() -> None:
    # grading is pure: line AND family must BOTH hit for a correct grade
    g = grade(_fake_task(), _fake_answer(3, "future_info_use"), 3, "future_info_use")
    assert g.correct
    g = grade(_fake_task(), _fake_answer(3, "future_info_use"), 3, "same_bar_execution")
    assert not g.correct and g.line_hit and not g.family_hit
    with pytest.raises(TypeError):
        grade(_fake_task(), _fake_answer(3, "future_info_use"), True, "future_info_use")


def _fake_task():
    from truerise.evals.pack import Task

    return Task(id="t", level=1, prompt="p", fixture="a\nb\nc\n")


def _fake_answer(line: int, family: str):
    from truerise.evals.pack import Answer

    return Answer(id="t", defect_line=line, family=family)


def test_honest_run_passes_cheater_and_mutator_fail(pack: Pack, tmp_path: Path) -> None:
    cert_h, _ = run_bench(solver=HonestSolver(), pack=pack,
                          work_dir=tmp_path / "wh", out_dir=tmp_path / "h",
                          trials=1, strategy="last", thresholds=THRESHOLDS,
                          solver_name="honest")
    assert cert_h.verdict == "PASS" and cert_h.reasons == []
    assert cert_h.evidence_effective_level == 4

    cert_c, _ = run_bench(solver=CheaterSolver(trials=20), pack=pack,
                          work_dir=tmp_path / "wc", out_dir=tmp_path / "c",
                          trials=20, strategy="majority", thresholds=THRESHOLDS,
                          solver_name="cheater")
    assert cert_c.verdict == "FAIL"
    assert any("corrected significance" in r and "trials=20" in r for r in cert_c.reasons)

    cert_m, _ = run_bench(solver=HonestSolver(), pack=pack,
                          work_dir=tmp_path / "wm", out_dir=tmp_path / "m",
                          trials=1, strategy="last", thresholds=THRESHOLDS,
                          solver_name="mutator", claim_thresholds=MUTATED)
    assert cert_m.verdict == "FAIL"
    assert any("prereg diff" in r and "min_accuracy" in r for r in cert_m.reasons)


def test_replay_recomputes_and_tamper_fails_closed(pack: Pack, tmp_path: Path) -> None:
    out = tmp_path / "r"
    run_bench(solver=HonestSolver(), pack=pack, work_dir=tmp_path / "wr",
              out_dir=out, trials=1, strategy="last", thresholds=THRESHOLDS,
              solver_name="honest")
    cert = replay(out)
    assert cert.verdict == "PASS" and "identical verdict" in cert.replay
    rows = [json.loads(x) for x in (out / "claims.jsonl").read_text(encoding="utf-8").splitlines()
            if x.strip()]
    rows[-1]["detail"]["verdict"] = "FAIL"
    (out / "claims.jsonl").write_text("\n".join(json.dumps(r) for r in rows) + "\n",
                                      encoding="utf-8")
    with pytest.raises(LedgerTampered):
        replay(out)


def test_runner_refuses_zero_trials(pack: Pack, tmp_path: Path) -> None:
    with pytest.raises(ValueError):
        run_trials(HonestSolver(), pack, tmp_path, trials=0)


def test_pack_validation_fails_closed(pack: Pack, tmp_path: Path) -> None:
    broken = tmp_path / "broken"
    broken.mkdir()
    (broken / "tasks.jsonl").write_text(
        (pack.root / "tasks.jsonl").read_text(encoding="utf-8"), encoding="utf-8")
    answers = (pack.root / "answers.jsonl").read_text(encoding="utf-8").splitlines()
    row = json.loads(answers[0])
    row["family"] = "not_a_family"
    (broken / "answers.jsonl").write_text(
        json.dumps(row) + "\n" + "\n".join(answers[1:]) + "\n", encoding="utf-8")
    with pytest.raises(ValueError, match="not in taxonomy"):
        load_pack(broken)


def test_aggregate_majority_cannot_un_count() -> None:
    def g(correct: bool) -> Grade:
        return Grade(task_id="t", line_hit=correct, family_hit=correct, correct=correct)

    passes = [{"t": g(True)}, {"t": g(False)}, {"t": g(False)}]
    graded = aggregate(passes, "majority")
    assert graded["t"].correct is False  # majority wins -- but ALL 3 trials were counted
    with pytest.raises(ValueError):
        aggregate(passes, "best_of")


def test_classify_rules_cover_all_six_families() -> None:
    assert classify_line("    series[i] = series[i + 1]") == "backward_fill_leak"
    assert classify_line("    drift = series[i + 2]") == "future_info_use"
    assert classify_line("    fill = close[i]") == "same_bar_execution"
    assert classify_line("    score = (series[i] - mu) * 100") == "full_sample_scaling"
    assert classify_line("    kept = [x for x in rows if x.pnl > 0]") == "sample_selection_bias"
    assert classify_line("    total = daily_rate * 12") == "unit_consistency"
    # the safe twins must NOT classify
    assert classify_line("    series[i] = series[i - 1]") is None
    assert classify_line("    next_fill = close[i + 1]") is None
    assert classify_line("    past = series[i - 1]") is None
    assert classify_line("    annual = monthly_rate * 12") is None
    assert classify_line("    kept = [x for x in rows if x.ts >= cutoff]") is None


def _long_seed_pack(tmp_path):
    from truerise.evals.long_horizon import generate_long_horizon_pack

    return generate_long_horizon_pack(tmp_path / "long_pack", chains=4, seed=77)


def test_long_horizon_deterministic_byte_reproducible(tmp_path):
    """W16: same seed -> byte-identical pack files."""
    from truerise.evals.long_horizon import generate_long_horizon_pack

    a = generate_long_horizon_pack(tmp_path / "a", chains=4, seed=77)
    b = generate_long_horizon_pack(tmp_path / "b", chains=4, seed=77)
    assert (a.root / "long_tasks.jsonl").read_bytes() == (b.root / "long_tasks.jsonl").read_bytes()
    assert a.pack_digest == b.pack_digest and a.answer_seal == b.answer_seal


def test_long_horizon_honest_completes_all_and_skipping_fails(tmp_path):
    """W16: the honest chain executes every step; a step-skipper fails even
    with the right endpoint."""
    pack = _long_seed_pack(tmp_path)

    def honest(task):
        from truerise.evals.long_horizon import execute_steps  # noqa: F401
        values = []
        prior = [task.data[0]]
        for spec in task.steps:
            kind, _, arg = spec.partition(":")
            if kind == "mean_window":
                k = int(arg)
                window = task.data[-k:] if len(task.data) >= k else task.data
                v = sum(window) // max(len(window), 1)
            elif kind == "count_above":
                v = sum(1 for x in task.data if x > int(arg))
            elif kind == "delta":
                v = task.data[-1] - prior[-1]
            else:
                v = prior[-1] * int(arg)
            values.append(v)
            prior.append(v)
        return values

    def skipper(task):
        values = honest(task)
        values[1] = 999  # wrong intermediate, endpoint may still be right
        return values

    from truerise.evals.long_horizon import run_long_horizon

    summary = run_long_horizon(honest, pack, tmp_path / "work_honest")
    assert summary["completed"] == summary["total"]
    assert summary["steps_passed_per_mille"] == 1000
    bad = run_long_horizon(skipper, pack, tmp_path / "work_skip")
    assert bad["completed"] == 0, "a skipped step fails the closed loop"
    assert bad["steps_passed_per_mille"] < 1000


def test_external_solver_records_are_executor_signed_at_L4():
    """W11a: an external solver's grades are signed by the RUNNER (the
    executor), so the evidence gate counts them at the declared level."""
    from truerise.gates import EvidenceRecord, evidence_gate
    from truerise.evals.runner import executor_signature

    sig = executor_signature("task-1", True, True)
    assert len(sig) == 64
    records = [
        EvidenceRecord(source="external-solver", declared_level=4,
                       executor_signed=True, declared_source_class="declared"),
    ]
    verdict = evidence_gate(records, claimed_level=4)
    assert verdict.effective_level == 4
    assert verdict.records_counted == 1


def test_self_reported_external_run_is_capped_at_L0():
    """W11a: the solver's OWN accuracy claim contributes nothing."""
    from truerise.gates import EvidenceRecord, evidence_gate

    records = [
        EvidenceRecord(source="external-solver-self-report", declared_level=4,
                       executor_signed=False, declared_source_class="declared"),
    ]
    verdict = evidence_gate(records, claimed_level=4)
    assert verdict.effective_level == 0
    assert verdict.records_counted == 0
    assert any("self-report" in d["reason"] for d in verdict.downgrades)


def test_external_solver_protocol_end_to_end(tmp_path):
    """W11a: the reference external solver runs as a SUBPROCESS through the
    protocol and its grades come back executor-signed."""
    import sys

    from truerise.evals.generator import generate_pack
    from truerise.evals.runner import run_external_trials

    pack = generate_pack(tmp_path / "pack", per_family=2, seed=99)
    cmd = [sys.executable, "-m", "truerise.evals.external_solver"]
    passes, signed = run_external_trials(cmd, pack, tmp_path / "work", trials=1)
    assert passes and signed
    assert all(r["executor"] == "truerise-runner" and r["signature"] for r in signed)
