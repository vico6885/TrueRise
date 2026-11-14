# Governance (治理)

EN: The append-only audit hash chain is the SINGLE authority for "what
happened" — zero exemptions (built-in providers, the tooling itself, and this
repository's own gates all write through it). Rows carry
{seq, ts, kind, severity, detail, prev_hash, hash}; verification recomputes
the whole chain (seq continuity / prev-hash linkage / content hash) and
locates tampering BY ROW. Legacy-prefixed rows stay readable and never
govern. Approvals never expire into consent (the only path to Approved is an
explicit approve call). Deletion flow = three gates in a fixed order
(risk analysis → itemized targets → human approval token); the POLICY is
pluggable, the ORDER is not.

CR: 追加审计哈希链是"发生了什么"的唯一权威——零豁免。行结构
{seq,ts,kind,severity,detail,prev_hash,hash}；校验全量重算三查并按行定位篡改。
legacy 前缀行只读不治。审批永不超时转同意（Approved 仅显式调用可达）。删除流程=
固定顺序三关（风险分析→逐项目标→人工批准令牌）；策略可插拔，顺序不可。

History floor (历史地板): the repository blob count must never decrease; a
decrease is evidence destruction and fails the history door. Floor bumps
require an `authorization` field pointing at the external governance log
(reviewers cross-check it out-of-repo) — in-repo bumps are not
self-authorizable.
历史地板：blob 计数只增不减；下降即证据销毁=红。提地板须 authorization 字段外链
治理日志（评审者仓外核对）——仓内提地板不可自授权。
