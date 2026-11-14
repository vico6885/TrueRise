"""CLI entry: ``python -m truerise demo`` with perturbation / replay switches."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .demo import replay, run_demo

DEFAULT_OUT = Path("output/truerise")


def _print_certificate(v) -> None:
    fp = v.fingerprint
    print(f"CLAIM     : {v.claim}")
    print(f"FINGERPRINT: model={fp['model']} harness={fp['harness']} "
          f"env={fp['environment']} evaluator={fp['evaluator']}")
    print(f"VERDICT   : {v.verdict}")
    for reason in v.reasons:
        print(f"            {reason}")
    print(f"CERTIFICATE: {v.certificate_path}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="truerise", description="TrueRise integrity-evaluation CLI (0.1-series)")
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
    args = parser.parse_args(argv)

    if args.cmd != "demo":
        return 2
    if args.replay:
        v = replay(args.out)
        _print_certificate(v)
        print(f"REPLAY    : {v.replay}")
        return 0
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


if __name__ == "__main__":
    sys.exit(main())
