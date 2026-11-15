.PHONY: check test fmt clippy gates gates-py wiring demo replay dual docs doc-links verify

# --- Rust -------------------------------------------------------------------

check:
	cargo check --workspace --all-targets

test:
	cargo test --workspace

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# --- Python -----------------------------------------------------------------

gates-py:
	python tools/lint_banned_words.py
	python tools/lint_git_history.py
	python tools/lint_file_length.py
	python tools/lint_deny_isolation.py
	python tools/lint_schemas.py
	python tools/lint_spec_map.py
	python tools/check_leakguard_skeleton.py
	python tools/check_claim_terms.py
	python tools/lint_dco.py
	python tools/check_docs.py

# Full local mirror of the CI gate set, minus the two jobs that need extra
# tooling (cargo-deny, cargo-cyclonedx) and the benchmark baseline job. Use
# `make verify` for the complete parity run.
gates: gates-py wiring

wiring:
	python tools/check_wiring.py --selftest
	python tools/check_wiring.py

# --- Behaviour you can look at ---------------------------------------------

demo:
	python -m truerise demo
	python -m truerise evals demo
	cargo run -p truerise-cli -- demo

replay:
	python -m truerise demo --replay

dual:
	python tools/check_dual_impl.py

docs:
	cargo doc --workspace --no-deps

doc-links:
	python tools/check_docs.py

# Everything CI asserts that this machine can assert, in one command.
verify: fmt clippy check test gates-py wiring
	python -m pytest python/tests -q
	python tools/check_dual_impl.py
	@echo "local verify complete (CI adds: cargo deny, SBOM, MSRV 1.85, criterion baseline)"
