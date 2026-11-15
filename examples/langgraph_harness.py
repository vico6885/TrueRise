"""LangGraph integration pattern (W15): TrueRise as the governance + eval
WRAPPER around any graph.

The pattern (framework-agnostic, shown with LangGraph because it is the
mainstream graph runtime):

    1. the agent graph stays YOURS - TrueRise never owns the LLM calls;
    2. every invocation of the graph runs inside a governed Session
       (budget + hash-chained audit + policy);
    3. after the graph finishes, the run is JUDGED by the bench's rules and
       the verdict lands on the ledger (ratchet: regression = non-zero).

Run it:  pip install langgraph truerise  (langgraph guarded below)
         python examples/langgraph_harness.py

This file is a PATTERN, not a shim: it deliberately does not import
TrueRise internals beyond the public SDK so it keeps working across
LangGraph versions.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python" / "src"))

from truerise.ledger import Ledger  # noqa: E402
from truerise.sdk import Budget, Session  # noqa: E402


def build_graph():
    """A two-node demo graph (plan -> act). Replace with YOUR graph."""
    try:
        from langgraph.graph import END, StateGraph
    except ImportError:
        print("langgraph not installed - showing the pattern with a stub graph")
        return None

    from typing_extensions import TypedDict

    class State(TypedDict):
        goal: str
        plan: str
        answer: str

    def plan(state: State) -> dict:
        return {"plan": f"do: {state['goal']}"}

    def act(state: State) -> dict:
        return {"answer": f"done: {state['plan']}"}

    graph = StateGraph(State)
    graph.add_node("plan", plan)
    graph.add_node("act", act)
    graph.set_entry_point("plan")
    graph.add_edge("plan", "act")
    graph.add_edge("act", END)
    return graph.compile()


def main() -> int:
    app = build_graph()
    ledger = Ledger()
    budget = Budget()
    budget_quota = 4

    def router(tag: str, payload: str) -> str:
        # YOUR model call happens here; the session governs it.
        budget.consume("langgraph-session", budget_quota)
        if tag == "plan":
            return f"plan: {payload}"
        return f"observed: {payload}"

    session = Session(ledger=ledger, router=router, pinned_model="your-model")
    goal = "verify the fold window"
    if app is not None:
        state = app.invoke({"goal": goal})
        goal = f"{goal} -> {state['answer']}"
    summary = session.run(goal, rounds=1)
    print(
        f"langgraph harness run: transcript {summary['transcript']}, "
        f"transitions {summary['transitions']}, audit rows {summary['audit_rows']}"
    )
    ledger.verify()
    print("chain verified: every step of YOUR graph is now auditable")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
