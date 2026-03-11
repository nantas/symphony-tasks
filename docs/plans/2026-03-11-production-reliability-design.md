# Production Reliability Design

## Goal

在不扩展到多实例、分布式锁、多 runner 类型的前提下，把当前 GitHub V1 协调器补齐为一个适合单进程、单实例、单试点仓库长期运行的生产化版本。

本轮设计聚焦两条主线：

- 生产化：部署方式、日志、最小运维面、运行约束
- 调度可靠性：retry 消费、终态收敛、异常恢复、状态解释一致性

不纳入本轮范围：

- 多实例协调
- 分布式锁
- Web UI
- 完整 metrics / tracing 平台
- 必做 HTTP 运维 API
- 多 runner 类型抽象

## 现状判断

当前 `main` 已完成 GitHub V1 闭环与一次真实 smoke：

- GitHub issue labels 作为状态来源
- GitHub PR 创建、查询、merge、close issue
- pilot 仓库接线与 runner wrapper
- 自动 merge 与人工 merge 两条 closeout 路径

但生产化与可靠性仍有几个明显缺口：

1. `retry_queue` 已能持久化和恢复，但还没有真正参与调度。
2. `reconcile_once()` 目前只有“PR watch reconcile + candidate issue dispatch”两层，没有外部终态收敛、retry 注入、周期摘要。
3. `recover_runtime_state()` 已能识别 interrupted run，但没有形成完整的恢复策略。
4. 运维面目前主要依赖本地状态文件，没有正式定义结构化事件日志和周期摘要。
5. daemon 虽然可运行，但还没有明确的 Linux/systemd 生产部署契约。

## 设计原则

### 1. 沿用 Symphony spec 主线

本轮设计尽量贴近 `SPEC.md` 推荐的生产调度模型：

- 单一 orchestrator 作为 scheduling state 的唯一变更者
- 每轮 poll tick 先 reconcile，再 dispatch
- retry queue 进入正式调度链路
- 失败恢复以重新轮询和重新调度为主，而不是恢复进程内上下文

### 2. 不重写 orchestrator 核心

当前 `dispatch_issue()`、`create_pr_for_run()`、`reconcile_pr_watch()` 的职责边界已经足够清晰。

本轮只在 `reconcile_once()` 外围补一层“调度控制面”，而不是引入新的大型 scheduler 子系统。

### 3. 运维面先做到最小合规

V1 先满足：

- operator 可见失败
- operator 可理解当前状态
- operator 可按文档部署和排障

HTTP 运维 API 可以作为下一阶段增强，而不是本轮硬要求。

## 一、调度主循环

### 目标

把当前单次 `reconcile_once()` 扩展为适合长期 daemon 运行的 5 段式调度周期。

### 建议周期

每轮固定执行以下 5 段：

1. 恢复与预处理
2. `pr_watch` reconcile
3. 外部终态收敛
4. retry 与 candidate dispatch
5. 周期摘要输出

### 1. 恢复与预处理

输入：

- `run_records`
- `pr_watch`
- `retry_queue`

职责：

- 读取当前本地状态
- 标记 interrupted run
- 生成本轮唯一权威的内存态快照

设计要点：

- 本轮调度只依赖本地持久化状态和远端最新轮询结果
- 不依赖进程内残留对象
- 不尝试恢复 agent 线程或进程上下文

### 2. `pr_watch` reconcile

优先处理所有已经有 PR 的 issue。

原因：

- 这些任务已进入后半程
- closeout 优先级高于拉新 issue
- 可更快释放本地 active state

行为：

- PR 已 merged：直接收尾
- review 通过且可 merge：自动 merge，然后收尾
- 仍等待 review：维持 watch
- 状态异常：写入失败或 retry 路径

### 3. 外部终态收敛

在 dispatch 前，先检查本地仍保有状态的 issue 是否已经在远端进入终态。

要处理的场景：

- issue 已关闭
- issue 已进入 `done`
- issue 已取消或不再属于 active state
- PR 已关闭但未 merged

收敛目标：

- 从 watch / retry / active claimed 集合中移除
- 不再继续协调
- 在日志摘要中给出明确的“收敛动作”

### 4. retry 与 candidate dispatch

`retry_queue` 从“记录文件”升级为“正式调度输入”。

本轮逻辑：

- 未到期 retry：继续屏蔽
- 已到期 retry：重新参与 dispatch 候选
- 新抓取的远端 candidate issues：与到期 retry 一起走统一筛选

统一筛选仍受以下限制：

- global concurrency
- repo concurrency
- claimed / running 去重
- workflow active states

### 5. 周期摘要输出

每轮 reconcile 结束时输出固定摘要，作为最小 observability 基线。

建议摘要至少包含：

- `reconciled_prs`
- `dispatched_runs`
- `retry_due`
- `retry_scheduled`
- `terminal_converged`
- `failed_runs`
- `skipped_due_to_backoff`

## 二、状态与收敛规则

### 1. 状态分层

保留当前 `RunStatus` 枚举，但补一层调度解释规则：

- `active`
  - `Claiming`
  - `PreparingWorkspace`
  - `RunningAgent`
  - `AwaitingPrCreation`
  - `AwaitingHumanReview`
- `retryable`
  - `Failed` 且存在有效 retry entry
- `terminal`
  - `Completed`
- `abnormal`
  - 本地仍为 active，但远端 issue / PR 已进入不一致终态

### 2. retry 规则

本轮优先实现“失败重试”闭环，不把 spec 中的多 turn continuation retry 一次性全部引入。

规则：

- 失败后写入 `retry_queue`
- entry 至少包含：
  - `issue_id`
  - `identifier`
  - `attempt`
  - `due_at`
  - `error`
- 到期后重新进入 dispatch
- 超过上限后保持 `Failed`

设计取舍：

- 支持 failure backoff：本轮必须完成
- 正常 continuation retry：作为下一阶段兼容项保留，不在本轮强行扩展 runner 语义

### 3. 外部终态收敛规则

建议以远端真实状态优先。

规则：

- issue 已关闭，且无 open PR watch
  - 清理 retry/watch
  - run 收敛为 terminal
- issue 已 terminal label
  - 不再参与 dispatch
- PR 已 closed 且未 merged
  - 不自动推进
  - 记为失败或待人工处理
- PR 已 merged
  - 无论谁 merge，统一走 closeout

### 4. interrupted run 恢复

daemon 重启后：

- 不恢复进程内 timer
- 不恢复 agent 线程上下文
- 只恢复调度意图
- interrupted run 由下一轮重新判定是否进入 retryable / re-dispatch 路径

这与 Symphony spec 的“fresh polling + re-dispatching eligible work”一致。

### 5. workspace 规则

workspace 不是调度真相来源，只是执行副产物。

规则：

- workspace 存在不代表 run 活跃
- terminal 后先保留 workspace 用于排障
- cleanup 作为显式策略，不直接耦合在当前调度主循环里

## 三、最小运维面与生产部署约束

### 1. 最小可观测输出

本轮先做：

- 结构化事件日志
- 周期摘要日志

事件日志建议覆盖：

- issue 被选中
- run 创建
- PR 创建
- PR 收尾
- retry 入队
- retry 到期
- terminal 收敛
- 失败落盘

建议字段：

- `repo_id`
- `issue_id`
- `issue_identifier`
- `run_status`
- `pr_ref`
- `attempt`
- `event`
- `result`
- `error`

### 2. 最小状态面

本轮不强制提供 HTTP server。

正式定义以下两类为运维面：

- 日志摘要
- 本地状态文件
  - `var/runs/...`
  - `var/state/pr_watch.json`
  - `var/state/retry_queue.json`

### 3. 部署约束

针对单实例 Linux daemon，明确以下约束：

- 一个实例对应一个 state root
- lock file 保证单实例
- `systemd` 负责：
  - 启动
  - 重启
  - 环境变量注入
  - stdout/stderr 收集
- 运行前置：
  - `GITHUB_TOKEN`
  - runner 程序与参数
  - repository config
  - workflow 文件可读

### 4. HTTP 运维 API 的定位

按 spec，将 HTTP `/api/v1/*` 定位为“可选增强”。

V1 必做：

- 结构化日志
- 周期摘要
- 文件状态可读
- `systemd` 部署契约

V1 可选：

- `/api/v1/state`
- `/api/v1/<issue_identifier>`
- `/api/v1/refresh`

## 四、测试与验收标准

### 1. 测试分层

建议新增四层测试：

- 单元测试
  - retry 到期/未到期筛选
  - terminal 收敛判定
  - interrupted run 恢复规则
  - 摘要 counters
- orchestrator 集成测试
  - 5 段周期顺序
  - `pr_watch` 优先于 dispatch
  - retry 到期后进入 dispatch
  - 外部关闭后停止协调
- 恢复测试
  - 从 `run_record + pr_watch + retry_queue` 恢复
  - 不恢复进程内 timer
- 真实 smoke
  - daemon 启动
  - 正常 issue -> PR -> merge
  - 失败 -> retry -> 重调度
  - 外部关闭 -> 本地收敛

### 2. 本轮必须覆盖的回归场景

- 到期 retry 被重新调度
- 未到期 retry 不会被调度
- `pr_watch` 中 merged PR 会优先收尾
- 外部关闭 issue 会从本地 active/retry/watch 中收敛移除
- interrupted run 重启后不会被误判为仍在运行
- 每轮 reconcile 产出可读摘要

### 3. 完成判定

本轮完成定义为：

1. `cargo test` 通过，并新增 retry/terminal/recovery/summary 测试
2. `cargo fmt -- --check` 与 `cargo clippy --all-targets --all-features -- -D warnings` 通过
3. daemon 模式下能观察到结构化事件日志和周期摘要
4. retry/backoff 与 terminal 收敛行为能在 smoke 中复现
5. 有一份 Linux 部署与最小运维手册

### 4. 不纳入本轮验收

- 多实例协调
- 分布式锁
- 多 runner 类型抽象
- Web UI
- 完整 metrics / trace 系统
- 必做 HTTP 运维 API

## 推荐实施顺序

1. 先补调度控制面，不改大模块边界
2. 先完成 failure retry + terminal 收敛
3. 再补摘要日志与 Linux 部署文档
4. 最后做新一轮 smoke

## 结论

推荐采用：

- 单一 orchestrator 权威状态
- `reconcile_once()` 5 段式调度主循环
- failure retry 正式消费
- 外部终态显式收敛
- 结构化日志 + 周期摘要作为最小运维面
- `systemd` 作为单实例 Linux 生产部署基线

这条路径最贴近 Symphony spec，也最适合当前单实例、单试点仓库优先的 V1 现实边界。
