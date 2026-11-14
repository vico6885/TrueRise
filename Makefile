.PHONY: check test fmt clippy gates

check:
	cargo check --workspace --all-targets

test:
	cargo test --workspace

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

gates:
	python tools/lint_banned_words.py
	python tools/lint_git_history.py
	python tools/lint_file_length.py
	python tools/lint_deny_isolation.py
	python tools/lint_schemas.py
	python tools/check_leakguard_skeleton.py
