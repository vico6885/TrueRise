"""W13a SDK tests: the Python surface mirrors the Rust contracts. Where a
Rust named test exists, the SAME semantic is asserted here (mirrored names
in comments) - the SDK is a contract consumer, never a second authority."""
from __future__ import annotations

import pytest

from truerise.ledger import Ledger
from truerise.sdk import ApprovalQueue, Budget, Policy, Ratchet, Session


def scripted_router(tag: str, payload: str) -> str:
    return f"scripted response for: {payload}"


def new_session(**kwargs) -> Session:
    return Session(ledger=Ledger(), router=scripted_router, pinned_model="scripted", **kwargs)


def test_sdk_session_three_books():
    """Mirror of minimal_run_three_books_consistent."""
    session = new_session()
    summary = session.run("analyse the window", rounds=1)
    assert summary["transcript"] >= 2
    assert summary["transitions"] == 5  # run_start + plan/act/observe + verdict
    assert summary["audit_rows"] == summary["transitions"]
    session.ledger.verify()


def test_sdk_budget_denominator_not_sharded():
    """Mirror of budget_is_shared_across_hats_not_sharded."""
    budget = Budget()
    budget.consume("hat-a")
    budget.consume("hat-a")
    budget.consume("hat-b")
    assert budget.global_count == 3
    assert budget.reconcile() == 0
    with pytest.raises(ValueError, match="quota"):
        budget.consume("hat-a", quota=2)


def test_sdk_policy_ask_requires_approval_on_file():
    """Mirror of ask_allow_is_per_evaluation_never_cached."""
    policy = Policy()
    policy.set("write_file", "ask")
    assert policy.decide("write_file", False) == "needs_approval"
    assert policy.decide("write_file", True) == "allow"
    assert policy.decide("write_file", False) == "allow", "approval cached"
    # deny short-circuits + default-deny:
    policy.set("rm", "deny")
    assert policy.decide("rm", True) == "deny"
    assert policy.decide("unknown-tool", True) == "deny"


def test_sdk_approval_never_expires_into_consent():
    """Mirror of timeout_never_approves + revoked_then_approve_refused."""
    queue = ApprovalQueue()
    queue.request("r1")
    assert queue.state("r1") == "pending"
    queue.approve("r1")
    assert queue.state("r1") == "approved"
    queue.request("r2")
    queue.revoke("r2")
    with pytest.raises(ValueError, match="revoked"):
        queue.approve("r2")
    epoch = queue.next_epoch()
    with pytest.raises(ValueError, match="stale epoch"):
        queue.fenced_write(epoch - 1, "stale")
    assert queue.fenced_write(epoch, "fresh").startswith("stored:")


def test_sdk_ratchet_only_tightens():
    """Mirror of ratchet_only_tightens + score_regression_is_exit_grade."""
    ratchet = Ratchet(baseline={"regression_rate_pm": 20})  # per-mille ints
    ratchet.propose({"regression_rate_pm": 10})
    with pytest.raises(ValueError, match="loosened"):
        ratchet.propose({"regression_rate_pm": 50})
    assert ratchet.baseline["regression_rate_pm"] == 10
    with pytest.raises(ValueError, match="regressed"):
        ratchet.verify({"regression_rate_pm": 30})
    ratchet.verify({"regression_rate_pm": 10})


def test_sdk_tool_request_goes_through_the_enforcement_chain():
    """Mirror of tool_denied_by_policy_writes_no_side_effect (RFC 0002)."""
    session = new_session(
        policy=Policy(),
        tools={"write_file": lambda args: f"wrote {args['path']}"},
    )
    session.policy.set("write_file", "deny")
    act = '{"truerise_tool": {"tool": "write_file", "args": {"path": "x.md"}}}'
    session.router = lambda tag, payload: act if tag == "act" else f"scripted: {payload}"
    session.run("write a file", rounds=1)
    rows = [r.kind for r in session.ledger.rows]
    assert "tool.request" in rows and "tool.denied" in rows
    assert "tool.executed" not in rows
    assert any(
        r.detail.get("reason") == "policy_deny"
        for r in session.ledger.rows
        if r.kind == "tool.denied"
    )


def test_sdk_plain_text_never_executes():
    """The envelope is the ONLY executable shape (RFC 0002)."""
    session = new_session(policy=Policy(), tools={"write_file": lambda a: "wrote"})
    session.router = lambda tag, payload: "write_file now" if tag == "act" else "plain"
    session.run("try to trick", rounds=1)
    assert not any(r.kind.startswith("tool.") for r in session.ledger.rows)


def test_sdk_budget_exhaustion_stops_the_session():
    """Mirror of budget_exhaustion_halts_run_at_step_n."""
    session = new_session(budget=Budget(), budget_quota=1)
    summary = session.run("goal", rounds=3)
    assert summary["stopped"] == "budget", "over budget stops the session"
    assert summary["transitions"] < 3 * 5, "round 2 never started"
    rows = [r.kind for r in session.ledger.rows]
    assert "budget.exhausted" in rows


def test_native_backend_is_optional_and_pure_python_is_complete():
    """W13b: the PyO3 wheel is an OPTIONAL accelerator. When present, its
    ratchet must agree with the SDK's; when absent, the SDK still runs."""
    from truerise.sdk import native_available

    if not native_available():
        assert True, "pure-Python path is the complete fallback"
        return
    import truerise_native as native

    from truerise.sdk import Ratchet

    native.ratchet_verify('{"failures_pm": 20}', '{"failures_pm": 20}')
    try:
        native.ratchet_verify('{"failures_pm": 20}', '{"failures_pm": 30}')
        raise AssertionError("regression must be refused natively")
    except ValueError:
        pass
    sdk = Ratchet(baseline={"failures_pm": 20})
    with pytest.raises(ValueError, match="regressed"):
        sdk.verify({"failures_pm": 30})


def test_python_row_hash_matches_the_rust_canonical_template():
    """Review fix B-P1-10 closure: the python canonical must be byte-identical
    to the Rust HashChain template for the same logical row - otherwise the
    two implementations mint different hashes for the same fact (the exact
    cross-implementation bug class docs/determinism.md forbids).

    The Rust template (governance/chain.rs content_hash):
      {"detail":<detail>,"hash":null,"kind":<kind>,"prev_hash":<prev>,
       "seq":<seq>,"severity":<sev>,"ts":<ts>}
    (sorted keys, compact separators, non-ASCII unescaped).
    """
    import hashlib
    import json

    from truerise.ledger import Ledger

    ledger = Ledger()
    row = ledger.append(
        "parity.probe",
        {"n": 1, "text": "非 ASCII 也必须一致"},
        severity="info",
        ts="seq@7",
    )
    row.hash = ""
    rust_template = (
        '{"detail":'
        + json.dumps(row.detail, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + ',"hash":null,"kind":"parity.probe","prev_hash":"'
        + row.prev_hash
        + '","seq":'
        + str(row.seq)
        + ',"severity":"info","ts":"seq@7"}'
    )
    expected = hashlib.sha256(rust_template.encode("utf-8")).hexdigest()
    assert row.content_hash() == expected, (
        "python canonical diverged from the Rust template - "
        "chains written by one implementation would be rejected by the other"
    )


def test_persisted_row_shape_is_rust_parseable():
    """The persisted line carries exactly the Rust AuditEventV2 fields (the
    Rust wire type denies unknown fields - a python-only key would make
    every python-written chain unparseable in Rust)."""
    import json as _json
    import tempfile
    from pathlib import Path

    from truerise.ledger import Ledger

    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "chain.jsonl"
        ledger = Ledger(path=path)
        ledger.append("probe.kind", {"x": 1}, ts="seq@1")
        row_keys = sorted(_json.loads(path.read_text(encoding="utf-8").splitlines()[0]).keys())
        assert row_keys == ["detail", "hash", "kind", "prev_hash", "seq", "severity", "ts"], row_keys
