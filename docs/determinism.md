# Determinism policy (确定性政策)

[English]
TrueRise claims determinism in THREE scoped layers, and explicitly refuses
to claim two things everyone secretly hopes for:

1. **Harness mechanics are deterministic.** The audit chain, the policy
   gates, the judging authority, the ratchet, and the offline replay all
   derive their outputs from their inputs alone: canonical JSON (sorted
   keys), SHA-256, integer arithmetic. Two executions over the same inputs
   and the same code produce byte-identical chains and certificates.
2. **Model steps are PINNED and RECORDED, not predicted.** A round is bound
   to one model id; any foreign answer voids the whole round (no partial
   credit). Every committed reading ships a TAPE (`readings/*/`): the
   recorded request/response pairs replay byte-exact offline — no keys, no
   network. The tape proves *a reading happened and recomputes*; it does
   NOT claim the provider will emit the same tokens next time.
3. **Floats never enter a chain row.** The canonical-JSON float rule: chain
   rows carry integers or strings ONLY, enforced at the write gate in BOTH
   implementations (Rust `HashChain::append` refuses floats; the Python
   ledger refuses floats). The reason is a real bug class this repository
   already caught once: `10.0` vs `10` divergence between the two
   implementations changed a seal digest. Ratios travel as integers
   (per-mille, basis points); p-values travel as exact fractions
   (`p_numerator / 2^p_pow2`).

**Not claimed (the honest boundary):** cross-hardware bit-identical
floating-point math, and deterministic model outputs. Where a wall-clock
timestamp exists (the `ts` field), it is provenance, not derivation: hashes
bind row content, and replay compares derived verdicts, not timestamps.

[简体中文]
TrueRise 只在三个有明确边界的层面上主张确定性，并明确拒绝两条人人暗中想要的声明：

1. **Harness 机制是确定的。** 审计链、策略门、判定权威、棘轮与离线重放的输出
   仅由输入决定：canonical JSON（键排序）、SHA-256、整数算术。相同输入 + 相同
   代码 = 字节级相同的链与证书。
2. **模型步是被钉定与被录制的，不是被预测的。** 每轮绑定一个模型 id；任何异族
   响应作废整轮（无部分得分）。每份 committed reading 附带 tape
   （`readings/*/`）：录制的请求/响应对离线逐字节重放——无需 key、无需网络。
   tape 证明的是"该读数真实发生且可复算"，不声称提供方下次会输出同样的 token。
3. **浮点数永不进入链行。** canonical JSON 浮点规则：链行只允许整数或字符串，
   在双侧写入门强制执法（Rust `HashChain::append` 拒绝浮点；Python ledger 同样
   拒绝）。原因是一次真实事故：`10.0` 与 `10` 的双实现分歧改掉了封印摘要。
   比例以整数传播（per-mille、basis points）；p 值以精确分数传播
   （`p_numerator / 2^p_pow2`）。

**不声称（诚实边界）：** 跨硬件位级一致的浮点运算；确定性的模型输出。
时间戳字段（`ts`）是溯源信息而非推导输入：哈希绑定行内容，重放比较的是推导
出的判词，不是时间戳。
