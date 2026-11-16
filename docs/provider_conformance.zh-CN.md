# 沙箱提供方一致性达标

[English](provider_conformance.md) | [简体中文](provider_conformance.zh-CN.md)

TrueRise 的一切代码执行必须经一致性达标的沙箱提供方。本页讲怎么加一个 —— 你自己的容器、microVM、jail 或进程内围栏 —— 以及它怎么被判。

先把两条铁律放在最前面，因为它们是设计而不是政策口号：

1. **自检只产事实，判定权威只有 `truerise-eval`。** 提供方永远不能自称达标。
2. **提供方住在自己的仓库里。** 本仓只放端口与一致性套件，不放别的
   （见 [`../providers/README.zh-CN.md`](../providers/README.zh-CN.md)）。

## 1. 端口

`crates/truerise-sandbox/src/provider.rs` —— 五个方法，没有可选项：

| 方法 | 契约 |
|---|---|
| `create(id, workdir) -> SandboxHandle` | 建立一个被围栏约束的工作区 |
| `execute(handle, action, payload) -> ExecutionOutput` | 执行一个动作；输出必须带上**本次执行实际生效**的能力等级 |
| `destroy(handle)` | 释放资源；同一句柄可重复调用 |
| `metrics(handle) -> HashMap<String, u64>` | 整数单位计数 |
| `audit_handle(handle) -> String` | 该沙箱在哈希链上的行引用前缀 |

能力位是声明式的（`CapabilityBits`：快照 / 分叉 / 暂停 / 恢复），观测等级有**三态** ——
全部可用、降级、不可用。降级绝不静默；不可用一律拒绝执行，而不是假装在跑。

## 2. 三区必失探针

`crates/truerise-sandbox/src/conformance.rs` 的 `self_check(root)` 恰好探三区。
正确的提供方在这三个探针上应当 FAIL，意思是攻击被拒绝：

| 区 | 探针 | 要求的拒绝方式 |
|---|---|---|
| ESCAPE（`conformance.rs:77`） | 越出围栏的路径 | 返回 `SandboxError` 拒绝，而不是把路径洗白后放行 |
| OVERSELL（`conformance.rs:106`） | 你没声明过的能力 | 按**未声明**拒绝 |
| RESIDUE（`conformance.rs:119`） | `destroy` 之后的磁盘状态 | 不留任何残留 |

逃逸探针在建仓期就立过功：首次运行抓到随仓附带的进程内提供方一处真实逃逸 ——
`Path::starts_with` 是字面比较、不解析 `..`。围栏随后改成按路径分量判定
（`ParentDir` / `RootDir` / `Prefix` 一律拒绝）。这正是必失探针的意义：它考你的实现，不考你的 README。

## 3. 从事实到判词

```rust
use truerise_sandbox::self_check;
use truerise_eval::judge;

let report = self_check(&root);          // 只有事实
let verdict = judge(&report);            // 三区全过才是 PASS
```

以下三条在 `cargo test -p truerise-eval` 里执法：

- `all_regions_pass_yields_pass`
- `any_region_fail_yields_fail`
- `verdict_is_recorded_and_reproducible` —— 无法写入审计链的判词是**错误**，不是警告
  （`judge_and_record`）

## 4. 新提供方自检清单

```bash
cargo test -p truerise-sandbox                                  # 进程内参考实现
cargo test -p truerise-eval                                     # 判定权威
python tools/lint_deny_isolation.py                             # 许可隔离
cargo cyclonedx --format json --spec-version 1.5 && python tools/check_sbom.py
#   （SBOM 这一步需要 cargo-cyclonedx 0.5.10，CI 里版本精确钉死）
```

然后在你自己的仓里：

- [ ] 实现 `SandboxProvider` 全部五个方法；生产路径不得出现 `unwrap`/`expect`
      （工作区 lint 直接 deny）
- [ ] 如实声明能力位；探针实测后上报**观测等级**而不是声明等级
- [ ] 对你的提供方跑 `self_check` 并公开报告
- [ ] 依赖一律精确版本钉死
- [ ] 许可只能 Apache-2.0 或 MIT —— BSL/GPL/AGPL 不得进依赖树，且由 CI 机器断言
      （`tools/lint_deny_isolation.py`）
- [ ] `unsafe` 只在带逐块 `// SAFETY:` 理由处出现，且标准对齐 `truerise-platform`
      （全工作区唯一允许它的 crate）

## 5. 怎么被收录

用 `feature_request` 模板开 issue，并附上：

1. `self_check` 报告（三区齐全），
2. 你的能力声明 + 实测观测等级，
3. crate 路径或仓库 URL，
4. 你所链接每一个组件的许可。

维护者把被接受的提供方记进 [`CHANGELOG.md`](../CHANGELOG.md)（注明每条判词的测量 commit），并注明
每个判词的测量 commit。项目只有一位维护者，所以这套流程刻意保持简陋 ——
**承载信任的是一致性判词，不是那份清单。**

## 6. 提供方的职责边界

- 不在你自己的 crate 里下一致性结论（判定权只在 `truerise-eval`）。
- 模型路由与记忆后端各有自己的端口 trait（`ModelRouter` 在
  `crates/truerise-core/src/router.rs`、`MemoryBackend` 在
  `crates/truerise-memory/src/lib.rs`）；工具暴露目前由 `truerise-mcp` 加三态策略承担 ——
  见 [`../providers/README.zh-CN.md`](../providers/README.zh-CN.md) 里点名的 `ToolProvider` 缺口。
  改任何端口 trait 都必须先立 RFC（`docs/rfc/README.md`）。
