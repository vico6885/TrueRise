# 文档索引

[English](README.md) | [简体中文](README.zh-CN.md)

本目录里的每一条声明都由代码执法、由闸门核对，没有一句是建议。若文档与代码不一致，代码为准，文档视为缺陷——请开 issue。

## 从这里开始（10 分钟）

| 阅读 | 面向 | 你能拿到 |
|---|---|---|
| [`quickstart.zh-CN.md`](quickstart.zh-CN.md) | 初次使用者 | 安装、第一次运行、第一份判词、第一次篡改演练 |
| [`cli.zh-CN.md`](cli.zh-CN.md) | 所有人 | 子命令与开关的唯一权威清单 |
| [`anti_cheat_bench.zh-CN.md`](anti_cheat_bench.zh-CN.md) | 评测工程师 | 基准协议：题包格式、封盘分类学、隔离模型、证书模式 |

## 理解设计

| 阅读 | 内容 |
|---|---|
| [`arch.md`](arch.md) | 六边形端口-适配器架构、crate 地图、三账一致 |
| [`governance.md`](governance.md) | 审计哈希链作为唯一权威、三态策略、审批、删除三关 |
| [`timesafety.md`](timesafety.md) | 四个泄漏向量与各自的具名强制测试 |
| [`topology.md`](topology.md) | 仓库形态、依赖方向、许可隔离规则 |
| [`determinism.md`](determinism.md) | 三层有边界的确定性声明与诚实边界（模型步钉定+录制、浮点规则） |
| [`integrations.md`](integrations.md) | 三个生态接入面：MCP、Python SDK、外部解题器协议 |
| [`report/anti_cheat_bench_v0.1.zh-CN.md`](report/anti_cheat_bench_v0.1.zh-CN.md) | v0.1 的实测读数，以及它们不能说明什么 |

## 参与贡献

| 阅读 | 贡献单元 |
|---|---|
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | DCO + 来源清洁声明 + 工程纪律 |
| [`task_pack_guide.zh-CN.md`](task_pack_guide.zh-CN.md) | 一个题包 = 一个 PR（主漏斗） |
| [`provider_conformance.zh-CN.md`](provider_conformance.zh-CN.md) | 一个沙箱提供方 + 它的判定结论 |
| [`rfc/README.md`](rfc/README.md) | 端口 trait 与 wire 破坏性变更必须先立 RFC |
| [`../RELEASING.md`](../RELEASING.md) | 发布清单，以及卡住首个 tag 的那几项决定（文末含中文段） |
| [`../ROADMAP.md`](../ROADMAP.md) | 已交付内容（以证据背书）与下一步 |

RFC 以英文为权威文本：中文只是阅读辅助，被评审并批准的是英文文件 ——
`docs/rfc/README.md` 要求维护者批准的正是那份英文件。

## 双语约定

面向用户的文档成对交付：`name.md`（英文，权威文本）与 `name.zh-CN.md`（简体中文，同结构同数字），两个文件在页眉互链。设计说明类文档（`arch`、`governance`、`timesafety`、`topology`、`determinism`、
`integrations`）在单文件内中英并置。

配对由机器核对：`python tools/check_docs.py` 在以下情况让 CI 变红——已登记文档失去对手件、页眉互链缺失、或文档引用了不存在的路径。

## 证据在哪

| 面 | 路径 |
|---|---|
| 已入库的参考读数 | `readings/v0.1/*.json`、`readings/v0.2/*.json`（含 evolve tape） |
| 评审中的证书格式 | `rfc/0001-claim-certificate.md` |
| 闸门命令（本地全部可跑） | `../Makefile` 的 `gates` 目标、`.github/workflows/ci.yml` |
