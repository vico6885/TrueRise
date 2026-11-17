#!/usr/bin/env python3
"""Documentation gate: bilingual pairing, link integrity, command integrity.

Three machine assertions (a comment is not a check):

1. PAIRING  - every registered user-facing document exists in BOTH languages
   (``name.md`` authoritative English, ``name.zh-CN.md`` Chinese) and each
   carries a header link to the other. Design notes listed in INLINE_BILINGUAL
   must actually contain Chinese text in the single file that claims to be
   bilingual - a document that says "Bilingual" and ships no Chinese is a
   defect, not a style choice.
2. LINKS    - every relative Markdown link resolves to a file in the tree.
3. COMMANDS - every ``tools/<script>.py`` and ``python -m truerise`` invocation
   named in a document refers to something that exists.

Exit codes: 0 = documentation contract holds, 1 = violations listed,
2 = the registry itself is broken (missing anchor file is fail-closed).
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Documents that must exist as an English/Chinese pair.
PAIRED = [
    "README.md",
    "docs/README.md",
    "docs/quickstart.md",
    "docs/cli.md",
    "docs/limits.md",
    "docs/anti_cheat_bench.md",
    "docs/task_pack_guide.md",
    "docs/provider_conformance.md",
    "docs/report/anti_cheat_bench_v0.1.md",
    "providers/README.md",
    "python/README.md",
    "tests/README.md",
]

# Single-file documents that carry both languages inline.
INLINE_BILINGUAL = [
    "docs/arch.md",
    "docs/determinism.md",
    "docs/governance.md",
    "docs/timesafety.md",
    "docs/topology.md",
    "docs/rfc/README.md",
]

CJK = re.compile(r"[㐀-䶿一-鿿]")
LINK = re.compile(r"\]\((?!https?:|mailto:|#)([^)#\s]+)")
TOOL_SCRIPT = re.compile(r"tools/([a-z0-9_]+\.py)")
MODULE_CALL = re.compile(r"python -m (truerise[\w.]*)")
HEADER_SWITCH = re.compile(r"^\[English\]\([^\)]*\) \| \[简体中文\]\([^\)]*\)", re.MULTILINE)


def read(path: Path) -> str:
    if not path.exists():
        raise FileNotFoundError(str(path))
    return path.read_text(encoding="utf-8", errors="replace")


def check_pairing(problems: list[str]) -> None:
    for rel in PAIRED:
        en, zh = REPO / rel, REPO / rel.replace(".md", ".zh-CN.md")
        for side, path in (("English", en), ("Chinese", zh)):
            try:
                text = read(path)
            except FileNotFoundError:
                problems.append(f"pairing: {side} document missing - {path.relative_to(REPO).as_posix()}")
                continue
            if not HEADER_SWITCH.search(text):
                problems.append(f"pairing: {path.relative_to(REPO).as_posix()} has no [English] | [简体中文] header switch")
        if zh.exists() and len(CJK.findall(zh.read_text(encoding="utf-8", errors="replace"))) < 40:
            problems.append(f"pairing: {zh.relative_to(REPO).as_posix()} is not Chinese content (>=40 CJK required)")
    for rel in INLINE_BILINGUAL:
        path = REPO / rel
        if not path.exists():
            problems.append(f"pairing: inline-bilingual document missing - {rel}")
            continue
        count = len(CJK.findall(path.read_text(encoding="utf-8", errors="replace")))
        if count < 20:
            problems.append(f"pairing: {rel} claims bilingual but carries {count} CJK characters (>=20 required)")


def all_markdown() -> list[Path]:
    """TRACKED markdown only (git ls-files). The gate judges the repository,
    not the developer's scratch directory: untracked references and ignored
    runtime output are out of scope - which is exactly what CI sees anyway
    (a fresh checkout carries no untracked files)."""
    proc = subprocess.run(
        ["git", "ls-files", "*.md"],
        cwd=REPO,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode != 0:
        raise RuntimeError(f"git ls-files failed: {proc.stderr.strip()[:200]}")
    return [REPO / line for line in proc.stdout.splitlines() if line.strip()]


def check_links(problems: list[str]) -> None:
    for md in all_markdown():
        text = md.read_text(encoding="utf-8", errors="replace")
        for match in LINK.finditer(text):
            target = match.group(1).strip()
            if not target or target.endswith("/"):
                continue
            resolved = (md.parent / target).resolve()
            if not resolved.exists():
                line = text[: match.start()].count("\n") + 1
                problems.append(f"link: {md.relative_to(REPO).as_posix()}:{line} -> missing {target}")


def check_commands(problems: list[str]) -> None:
    for md in all_markdown():
        text = md.read_text(encoding="utf-8", errors="replace")
        for name in set(TOOL_SCRIPT.findall(text)):
            if not (REPO / "tools" / name).exists():
                problems.append(f"command: {md.relative_to(REPO).as_posix()} cites tools/{name} which does not exist")
        for module in set(MODULE_CALL.findall(text)):
            package = module.replace("truerise", "python/src/truerise").replace(".", "/")
            if module == "truerise":
                continue
            candidate = REPO / (package + ".py")
            candidate_dir = REPO / package
            if not candidate.exists() and not candidate_dir.exists():
                problems.append(f"command: {md.relative_to(REPO).as_posix()} cites `python -m {module}` which does not exist")


def main() -> int:
    problems: list[str] = []
    try:
        check_pairing(problems)
    except FileNotFoundError as exc:
        print(f"docs gate unusable: missing registry anchor {exc}", file=sys.stderr)
        return 2
    check_links(problems)
    check_commands(problems)
    if problems:
        print("documentation gate violations:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    paired = len(PAIRED) * 2
    print(f"documentation gate: {paired} paired docs + {len(INLINE_BILINGUAL)} inline-bilingual, "
          f"links and cited commands resolve (exit 0)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
