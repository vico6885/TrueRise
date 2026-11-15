# Integrations (生态集成)

[English]
TrueRise meets the Python agent ecosystem through THREE surfaces, in
deliberate order of preference:

1. **MCP (the tool supply side).** `truerise mcp --serve` exposes a
   zero-exposure MCP server (empty allowlist by default). Any MCP-capable
   framework or editor can mount TrueRise-governed tools without any
   TrueRise-specific adapter. Every call lands on the hash chain.
2. **The Python SDK (the harness embedding side).** `truerise.sdk.Session`
   wraps ANY agent loop - LangGraph graphs, OpenAI Agents SDK runners, or a
   bare function - with budget caps, the hash-chained audit, the three-state
   policy and the ratchet. Two official patterns ship in `examples/`
   (`langgraph_harness.py`, `openai_agents_harness.py`); both are
   import-guarded and run without the third-party framework installed.
3. **The external-solver protocol (the evaluation side).** Any solver that
   honours the one-file contract (`TRUERISE_PUBLIC_DIR` in,
   `submission.jsonl` out) can be evaluated by the bench via
   `run_external_trials` - the runner grades and executor-signs the records.

**What we deliberately do NOT do:** ship per-framework shims that pin
framework versions, or re-implement judging in Python (the authority stays
in `truerise-eval`). The pattern is a wrapper, not a fork.

OTLP export: `truerise-observe` carries an `otel` feature
(`cargo check -p truerise-observe --features otel`); spans mirror the audit
rows one-way, sensitive keys redacted on the way out.

[简体中文]
TrueRise 以三个面接入 Python Agent 生态，优先级刻意分明：

1. **MCP（工具供给面）。** `truerise mcp --serve` 起一个默认零暴露的 MCP
   server（空允许清单）；任何支持 MCP 的框架或编辑器都能直接挂 TrueRise
   治理下的工具，无需任何专用适配器；每次调用都落哈希链。
2. **Python SDK（harness 嵌入面）。** `truerise.sdk.Session` 可以包裹任何
   agent 循环——LangGraph 图、OpenAI Agents SDK runner、或一个普通函数——
   提供预算上限、哈希链审计、三态策略与棘轮。`examples/` 附两个官方模式
   （`langgraph_harness.py`、`openai_agents_harness.py`），均带导入守卫，
   未安装第三方框架也能跑。
3. **外部解题器协议（评测面）。** 任何遵守单文件契约（`TRUERISE_PUBLIC_DIR`
   进、`submission.jsonl` 出）的解题器都能经 `run_external_trials` 被基准
   评测——由 runner 判卷并对记录做 executor 签名。

**刻意不做：** 钉死框架版本的逐框架 shim；在 Python 里复制判定权威
（判定权只在 `truerise-eval`）。模式是包裹，不是分叉。

OTLP 导出：`truerise-observe` 带 `otel` feature
（`cargo check -p truerise-observe --features otel`）；span 单向镜像审计行，
敏感键在出口脱敏。
