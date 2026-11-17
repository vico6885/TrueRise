# Limits

[English](limits.md) | [简体中文](limits.zh-CN.md)

What this framework can enforce, what it can only measure, and what it can
never promise. This page is a hard boundary declaration, not a disclaimer.

## The self-reference boundary

A verification layer cannot verify its own target. Whether the criteria
measure what the project actually wants (construct validity), and whether the
goal itself is the right one (Goodhart), are outside the machine-decidable
closure. Therefore:

- **Machine layer**: unsupported claims, unannounced stubs, incomplete
  ledgers, incomparable comparisons, unapproved merges, and unmotivated
  give-ups become machine-refusable.
- **Procedure layer**: "is the goal the right one" becomes a sealed, signed,
  versioned, externally reviewable public object - never an algorithmic
  verdict.

Any sentence that promises beyond this boundary ("runaway-proof", "hallucination-free", "never misleads") is exactly the claim class this
project's own gate refuses. The claim gate (`tools/check_claim_terms.py`)
enforces the promise-grade wording rules on the guarded surfaces below.

## Promise grades

Every governance promise in this project is graded. The grade registry lives
in `claim_terms.json` (`promise_grades`) and is machine-checked for closure:
every backlog package appears at least once, rows are unique per
(package, aspect), and unknown grades are refused.

| Grade | What it means | Example |
|---|---|---|
| **A enforceable** | the mechanism refuses to issue the artifact | unsupported claims, unannounced stubs, unequal-budget comparisons |
| **B measurable** | the mechanism publishes its own precision and cost | interception precision/recall, gate net-benefit ledger |
| **C reducible-only** | the mechanism lowers a probability, never to zero | deliberately crafted deception, misleading wording, goal selection |

Enforcement rule: the claim gate rejects any sentence that upgrades a grade-C
outcome into grade-A wording (guarantee-family words). An allowlist line is
accepted only if it carries an explicit negation - the gate checks for it,
so a boundary sentence must say "does not promise", never "promises".

## What is never promised

1. Open-world truth. The framework refuses **unsupported** statements and
   recomputes **closed-form** ones; it does not and cannot certify that an
   open-world statement is true.
2. Goal correctness. A sealed objective is declared, signed and versioned -
   never "validated".
3. User satisfaction. Satisfaction is an outcome, not a criterion; the
   framework enforces per-item evidence, gap disclosure and self-service
   verification instead.
4. Its own integrity. The gates' bite is itself tested (mutation drills);
   an untested gate is treated as nonexistent, not as trusted.
5. Race-free intent verification. The mid-run intent check re-reads the
   contract file AFTER the run: a window remains where the file is edited
   and restored DURING a slow provider call (TOCTOU). The seal-on-chain
   row bounds the damage (any post-run divergence still surfaces); closing
   the window fully needs an OS-level file lock on the contract, queued for
   the write-gate batch.
