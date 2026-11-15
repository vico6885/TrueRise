"""OpenAI Agents SDK integration pattern (W15): TrueRise wraps the agent
LOOP, not the agent.

The pattern:

    1. the agent + its tools stay YOURS (the Agents SDK owns handoffs);
    2. TrueRise owns the GOVERNANCE: budget caps, the hash-chained audit of
       every step, the three-state policy for any tool that touches the
       world (RFC 0002), and the ratchet over the agent's task score;
    3. one command re-plays the recorded run offline (the tape).

Run it:  pip install openai-agents truerise  (guarded below)
         python examples/openai_agents_harness.py
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python" / "src"))

from truerise.ledger import Ledger  # noqa: E402
from truerise.sdk import Policy, Session  # noqa: E402


def agent_turn(payload: str) -> str:
    """YOUR agent call (Agents SDK Runner.run etc.). The harness never sees
    inside it - it sees the request and the response, and audits both."""
    try:
        from agents import Runner, arithmetic_agent  # your agent definition
    except ImportError:
        return f"[unaudited-agent stub for] {payload}"
    return str(Runner.run_sync(arithmetic_agent, payload))


def main() -> int:
    ledger = Ledger()
    policy = Policy()
    policy.set("write_file", "ask")  # fail-closed default for world-touching tools
    session = Session(ledger=ledger, router=lambda tag, p: agent_turn(p), pinned_model="your-agent")
    summary = session.run("compute the corrected significance for 20 trials", rounds=1)
    print(
        f"agents-sdk harness run: transcript {summary['transcript']}, "
        f"audit rows {summary['audit_rows']} (every agent turn is on the chain)"
    )
    ledger.verify()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
