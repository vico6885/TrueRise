"""CLI entry: ``python -m truerise demo`` and the Anti-Cheat Bench (evals)."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .demo import replay as demo_replay
from .demo import run_demo

DEFAULT_OUT = Path("output/truerise")
DEFAULT_THRESHOLDS = {"min_accuracy": 0.8, "claim_p": 0.05}
MUTATED_THRESHOLDS = {"min_accuracy": 0.5, "claim_p": 0.03}


def _print_certificate(v) -> None:
    fp = v.fingerprint
    print(f"CLAIM     : {v.claim}")
    print(f"FINGERPRINT: model={fp['model']} harness={fp['harness']} "
          f"env={fp['environment']} evaluator={fp['evaluator']}")
    print(f"VERDICT   : {v.verdict}")
    for reason in v.reasons:
        print(f"            {reason}")
    print(f"CERTIFICATE: sealed at {v.prereg_seal[:12]} ({v.certificate_path})")


def _run_demo(args) -> int:
    if args.replay:
        v = demo_replay(args.out)
        _print_certificate(v)
        print(f"REPLAY    : {v.replay}")
        # Review fix (P1): a MISMATCH verdict used to exit 0 - the one
        # command whose whole contract is "recompute and compare" went green
        # on disagreement.
        return 1 if "MISMATCH" in v.verdict else 0
    perturb = {}
    for kv in args.perturb:
        k, _, val = kv.partition("=")
        perturb[k.strip()] = float(val) if k.strip() == "claim_p" else int(val)
    v = run_demo(
        args.out,
        n_trials=1 if args.honest else perturb.get("n_trials", 1000),
        claim_p=0.04 if args.honest else perturb.get("claim_p", 0.03),
        honest_seal=args.honest_seal or args.honest,
        sign_records=args.sign_records or args.honest,
        drop_fingerprint=args.drop_fingerprint,
    )
    _print_certificate(v)
    return 0 if v.verdict == "PASS" else 1


# -- evals (Anti-Cheat Bench) -----------------------------------------------

def _print_bench_cert(cert, summary=None) -> None:
    fp = cert.fingerprint
    print(f"CLAIM     : {cert.claim}")
    print(f"FINGERPRINT: solver={fp['model']} harness={fp['harness']} "
          f"pack={fp['environment']} evaluator={fp['evaluator']}")
    print(f"VERDICT   : {cert.verdict}")
    for reason in cert.reasons:
        print(f"            {reason}")
    if summary is not None:
        print(f"ACCURACY  : {summary['accuracy']:.0%} ({summary['correct']}/{summary['total']})")
    print(f"CERTIFICATE: sealed at {cert.prereg_seal} ({cert.certificate_path})")


def _run_evals(args) -> int:
    from truerise.evals import CheaterSolver, HonestSolver, generate_pack, load_pack, run_bench

    if args.evals_cmd == "gen":
        pack = generate_pack(Path(args.out), per_family=args.per_family, seed=args.seed)
        print(f"pack: {len(pack.tasks)} tasks -> {args.out}/{{tasks,answers}}.jsonl")
        print(f"pack_digest={pack.pack_digest[:12]} answer_seal={pack.answer_seal[:12]}")
        return 0

    if args.evals_cmd == "run":
        pack = load_pack(Path(args.pack))
        solver = CheaterSolver(trials=args.trials) if args.cheat else HonestSolver()
        cert, summary = run_bench(
            solver=solver, pack=pack, work_dir=Path(args.work), out_dir=Path(args.out),
            trials=args.trials, strategy=args.strategy, thresholds=DEFAULT_THRESHOLDS,
            solver_name="cheater" if args.cheat else "honest",
            claim_thresholds=MUTATED_THRESHOLDS if args.mutate else None,
        )
        _print_bench_cert(cert, summary)
        return 0 if cert.verdict == "PASS" else 1

    # demo: one pack, three runs -- honest must PASS, both cheats must FAIL
    pack = generate_pack(Path(args.out) / "pack", per_family=args.per_family, seed=args.seed)
    print(f"pack: {len(pack.tasks)} tasks (pack_digest={pack.pack_digest[:12]})")
    outcomes: dict[str, str] = {}
    cert, summary = run_bench(
        solver=HonestSolver(), pack=pack, work_dir=Path(args.out) / "w_honest",
        out_dir=Path(args.out) / "honest", trials=1, strategy="last",
        thresholds=DEFAULT_THRESHOLDS, solver_name="honest")
    outcomes["honest"] = cert.verdict
    _print_bench_cert(cert, summary)
    cert, summary = run_bench(
        solver=CheaterSolver(trials=20), pack=pack, work_dir=Path(args.out) / "w_cheat",
        out_dir=Path(args.out) / "cheater", trials=20, strategy="majority",
        thresholds=DEFAULT_THRESHOLDS, solver_name="cheater")
    outcomes["cheater"] = cert.verdict
    _print_bench_cert(cert, summary)
    cert, summary = run_bench(
        solver=HonestSolver(), pack=pack, work_dir=Path(args.out) / "w_mut",
        out_dir=Path(args.out) / "mutator", trials=1, strategy="last",
        thresholds=DEFAULT_THRESHOLDS, solver_name="mutator",
        claim_thresholds=MUTATED_THRESHOLDS)
    outcomes["mutator"] = cert.verdict
    _print_bench_cert(cert, summary)
    print(f"SUMMARY   : {outcomes} (acceptance: honest PASS, cheater FAIL, mutator FAIL)")
    return 0 if outcomes == {"honest": "PASS", "cheater": "FAIL", "mutator": "FAIL"} else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="truerise", description="TrueRise integrity-evaluation CLI")
    sub = parser.add_subparsers(dest="cmd", required=True)
    demo = sub.add_parser("demo", help="three cheating scenarios, caught in 60 seconds")
    demo.add_argument("--perturb", action="append", default=[],
                      help="flip inputs: key=value (n_trials=10 moves the corrected denominator; claim_p=0.001 clears S2)")
    demo.add_argument("--honest-seal", action="store_true", help="thresholds match the seal (S1 line off)")
    demo.add_argument("--sign-records", action="store_true", help="executor-signed records (S3 line off)")
    demo.add_argument("--honest", action="store_true", help="all three scenarios honest -> PASS")
    demo.add_argument("--drop-fingerprint", action="store_true", help="refuse cross-environment comparison")
    demo.add_argument("--out", type=Path, default=DEFAULT_OUT, help="certificate output dir")
    demo.add_argument("--replay", action="store_true", help="offline recomputation of the last verdict")

    ev = sub.add_parser("evals", help="Anti-Cheat Bench: task packs, sealed answers, certificates")
    ev_sub = ev.add_subparsers(dest="evals_cmd", required=True)
    ev_gen = ev_sub.add_parser("gen", help="generate a deterministic task pack")
    ev_gen.add_argument("--out", type=Path, default=Path("output/truerise_evals/pack"))
    ev_gen.add_argument("--per-family", type=int, default=20, help="tasks per family (6 families)")
    ev_gen.add_argument("--seed", type=int, default=20261115)
    ev_run = ev_sub.add_parser("run", help="run a solver over a pack")
    ev_run.add_argument("--pack", type=Path, default=Path("output/truerise_evals/pack"))
    ev_run.add_argument("--trials", type=int, default=1)
    ev_run.add_argument("--strategy", choices=["last", "majority"], default="last")
    ev_run.add_argument("--cheat", action="store_true", help="use the best-of-N cheater solver")
    ev_run.add_argument("--mutate", action="store_true", help="re-claim against mutated thresholds")
    ev_run.add_argument("--work", type=Path, default=Path("output/truerise_evals/work"))
    ev_run.add_argument("--out", type=Path, default=Path("output/truerise_evals/run"))
    ev_demo = ev_sub.add_parser("demo", help="generate a pack + honest/cheater/mutator runs")
    ev_demo.add_argument("--out", type=Path, default=Path("output/truerise_evals"))
    ev_demo.add_argument("--per-family", type=int, default=20)
    ev_demo.add_argument("--seed", type=int, default=20261115)
    args = parser.parse_args(argv)

    if args.cmd == "evals":
        return _run_evals(args)
    return _run_demo(args)


if __name__ == "__main__":
    sys.exit(main())
