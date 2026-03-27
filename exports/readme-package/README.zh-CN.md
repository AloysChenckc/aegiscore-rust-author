# aegiscore-rust-author

一个面向 AegisCore 的 Rust-first 蓝图 authoring runtime。

`aegiscore-rust-author` 是一个开源的、Rust-first 的蓝图生成与规范化运行时，用于把产品想法、外部 AI 生成的 blueprint、已有项目上下文，整理成一套**可验证、可回读、可审计、可交接**的机器可读蓝图包。

它不是一个“只会写文档的 prompt 工具”，而是一套面向 staged development、模块化规划、worktree 开发流程和 machine-readable contract 的 authoring runtime。

---

## 为什么需要它

很多 AI 辅助开发流程的问题不在于“生成不了内容”，而在于这些内容很难真正进入工程链路。

常见问题包括：

- blueprint 文档容易和真实工程约束漂移
- 阶段边界不明确
- 模块职责和语言约束不清楚
- 不同 AI 生成的 blueprint 风格不统一
- 下游 runtime 或 agent 很难判断什么才是 authoritative truth

`aegiscore-rust-author` 解决的正是这个问题：

**把 blueprint 从“描述性文本”提升为“可验证的工程协议”。**

---

## 它能做什么

`aegiscore-rust-author` 可以：

- 从产品想法、PRD、架构说明生成 staged blueprint package
- 导入并归一化 `markdown`、`json`、`toml` 外部 blueprint
- 明确规划模块职责和实现语言
- 把人类可读蓝图文档编译成 machine-readable contract
- 构建 source-first semantic IR，用于归一化与验证
- 生成 patch plan、patch base、replay report、decision artifact
- 把 worktree 开发流程建模成 machine-readable protocol
- 从磁盘回读并重新验证已生成的 workspace bundle
- 为下游 runtime 和 agent 生成更短的 handoff artifact

---

## 核心定位

这个项目不是一个自由发挥的“AI 规划器”，也不是一个无限泛化的文档解释器。

它真正强的地方是：

- 域内强治理
- 高可信归一化
- machine-readable truth
- 可重放、可验证、可迁移
- worktree-aware 的 staged workflow 建模

它特别适合：

- blueprint-driven development
- staged AI agent execution
- contract-first 工程流程
- module-aware / worktree-aware 项目规划
- 需要把 authoring 和 execution 严格分层的系统

---

## 运行时边界

`aegiscore-rust-author` 负责：

- blueprint generation
- blueprint normalization
- semantic extraction
- module planning
- implementation language planning
- contract compilation
- readiness / decision artifact
- workspace revalidation
- schema migration

它不负责：

- blueprint gate 审批
- coding execution
- finalize 或 stage advance 执行
- `git worktree` 生命周期管理

这些执行态职责应交给 `aegiscore-rust-runtime`。

---

## Quick Start

### 1. 选择一个 workspace

先创建或选择一个目标 workspace，蓝图包会被输出到这里。

示例：

```powershell
mkdir my-agent-project
cd my-agent-project
```

### 2. 准备输入

你可以从这些内容开始：

- 产品想法
- PRD 或架构说明
- 外部 `markdown` / `json` / `toml` 蓝图
- 一个已经存在 `blueprint/` 文档的 workspace

### 3. 运行 authoring runtime

常用模式包括：

- `new-project`
- `import-blueprint`
- `update-blueprint`
- `recompile-contract`
- `validate-workspace`
- `migrate-workspace`

例如：导入一份外部 blueprint

```powershell
cargo run -p ara-cli -- emit `
  --workspace C:\Path\To\Workspace `
  --project-name my-agent-project `
  --source-summary "AI agent project for structured local control" `
  --source-file C:\Path\To\external-blueprint.md `
  --mode import-blueprint
```

### 4. 重新验证输出结果

在 emit 之后，建议始终执行一次 workspace 回读验证：

```powershell
cargo run -p ara-cli -- validate-workspace `
  --workspace C:\Path\To\Workspace
```

### 5. 查看生成产物

核心输出位于：

- `blueprint/`
- `.codex/auto-dev/`

重点产物包括：

- `resolved-contract.json`
- `semantic-ir.json`
- `normalization-report.json`
- `worktree-protocol.json`
- `readiness.json`
- `decision-summary.json`
- `agent-brief.json`

### 6. 交给 runtime

当包成功通过验证并到达 `candidate-for-blueprint-gate` 后，再交给 `aegiscore-rust-runtime` 执行。

---

## 项目结构

```text
aegiscore-rust-author/
├── crates/
│   ├── ara-schemas/
│   ├── ara-core/
│   ├── ara-runtime/
│   ├── ara-cli/
│   └── ara-host-api/
├── defaults/
├── docs/
├── exports/
├── scripts/
├── templates/
├── wrappers/
├── SKILL.md
└── Cargo.toml
```

### 关键目录说明

- `crates/ara-schemas/`
  保存 schema、错误码、machine-readable struct 和 schema version。

- `crates/ara-core/`
  核心 authoring 逻辑：归一化、semantic IR、模块规划、contract 编译、patch planning、worktree protocol、readiness、validation。

- `crates/ara-runtime/`
  文件系统、fingerprint、路径规范化、稳定写入和底层运行时辅助。

- `crates/ara-cli/`
  authoring、validation、migration、patch replay 的命令行入口。

- `crates/ara-host-api/`
  给其他 Rust 宿主嵌入使用的 library-first API。

- `defaults/`
  被运行时真正执行的 TOML 默认策略和 contract 默认值。

- `docs/`
  架构说明、schema 说明、归一化规则、语言策略、readiness 模型和能力总结。

- `scripts/`
  skill 结构验证和端到端 self-check 脚本。

- `templates/`
  蓝图文档和 machine artifact 模板。

- `wrappers/`
  PowerShell、POSIX shell、batch 的薄封装启动器。

### 输出的 workspace 结构

一个生成好的 workspace 通常长这样：

```text
<workspace>/
├── blueprint/
│   ├── authority/
│   ├── workflow/
│   ├── stages/
│   └── modules/
└── .codex/
    └── auto-dev/
        ├── project-contract.toml
        ├── resolved-contract.json
        ├── semantic-ir.json
        ├── normalization-report.json
        ├── patch-plan.json
        ├── worktree-protocol.json
        ├── readiness.json
        ├── decision-summary.json
        └── agent-brief.json
```

---

## 主要能力

### Blueprint generation

从想法、PRD、架构说明生成 staged blueprint package。

### External blueprint normalization

导入 `markdown`、`json`、`toml` blueprint，并归一化到本地 schema。

### Semantic IR

构建 source-first semantic IR，保存：

- normalized source truth
- rendered projection
- semantic frames
- semantic clusters
- semantic conflicts
- ambiguity 与 risk 信号

### Module planning

为模块生成：

- responsibility
- layer
- language assignment
- artifact ownership
- worktree-role binding

### Patch and update runtime

支持：

- patch-base
- patch-plan
- patch replay
- reverse replay proof
- patch execution reporting

### Worktree-aware protocol

支持：

- worktree model
- role ownership
- branch patterns
- exclusive paths
- sync / merge / cleanup rules
- worktree-aware patch scope validation

### Workspace revalidation

支持从磁盘重新加载并验证 workspace，检查：

- schema version drift
- contract inconsistency
- manifest tampering
- readiness drift
- semantic drift
- patch drift
- worktree protocol drift

---

## 输出产物

`aegiscore-rust-author` 不只是生成 Markdown 文档，它还会产出一整套 machine-readable artifact，例如：

- `blueprint/authority/`
- `blueprint/workflow/`
- `blueprint/stages/`
- `blueprint/modules/00-module-catalog.json`
- `.codex/auto-dev/project-contract.toml`
- `.codex/auto-dev/resolved-contract.json`
- `.codex/auto-dev/semantic-ir.json`
- `.codex/auto-dev/normalization-report.json`
- `.codex/auto-dev/change-report.json`
- `.codex/auto-dev/patch-base.json`
- `.codex/auto-dev/patch-plan.json`
- `.codex/auto-dev/patch-execution-report.json`
- `.codex/auto-dev/worktree-protocol.json`
- `.codex/auto-dev/readiness.json`
- `.codex/auto-dev/decision-summary.json`
- `.codex/auto-dev/agent-brief.json`
- `.codex/auto-dev/task-progress.json`

这些产物共同组成一条下游 runtime 和 agent 可以安全消费的 machine-readable truth chain。

---

## 为什么用 Rust

这个项目用 Rust，不是为了“炫技术栈”，而是因为它承载的是“真相层逻辑”，不是普通脚本胶水。

Rust 在这里的价值包括：

- 强类型 schema 和 contract 建模
- 更稳定的 machine artifact 生成
- 更可靠的 validation、replay、migration 逻辑
- 更好的跨平台一致性
- 更适合 library-first + CLI-first 的 runtime 形态

整体语言分工是：

- Rust：schemas、core、runtime、CLI、host API
- PowerShell / sh / bat：仅做薄包装
- Markdown / TOML / JSON / YAML：承载内容和协议

---

## 适合谁

如果你正在做这些事，这个项目会很适合：

- blueprint-driven development
- staged AI agent workflow
- contract-first 工程链路
- worktree-aware 规划流程
- 需要给下游 execution runtime 提供可靠输入的 authoring layer

---

## 它不是什么

`aegiscore-rust-author` 不是：

- 通用自由文本 AI planner
- 任意文档风格的万能编译器
- `git worktree` 管理器
- 编码执行 runtime
- 下游 execution engine 的替代品

更准确地说，它是：

**一个高可信的 authoring runtime，用于把 blueprint intent 变成工程协议。**

---

## 设计哲学

这个项目更重视：

- determinism over improvisation
- machine truth over prose-only output
- replayability over hidden merge behavior
- validation over optimistic guessing
- explicit protocol over loose interpretation

一句话：它追求的是“可解释、可重放、难以悄悄漂移”的 authoring 系统。

---

## 开源价值

这个项目开源的价值，不只是“又一个 AI 工具”。

它提供了一条更严谨的路线，让 AI-assisted development 从：

- prompt-first

走向：

- blueprint-first
- contract-first
- machine-verifiable authoring

如果你也在探索：

- AI 软件架构 authoring
- blueprint-driven development
- machine-verifiable planning
- worktree-aware agent workflow
- governance-first agent engineering

那么 `aegiscore-rust-author` 会是一个很好的参考。

---

## 当前状态

这个项目已经具备完整 authoring runtime 形态，包括：

- blueprint generation
- normalization
- contract compilation
- semantic IR
- patch 与 replay evidence
- worktree protocol
- readiness / decision / handoff artifact
- workspace validation 与 migration

它当前的定位已经很清楚：

**一个高可信的 AegisCore authoring runtime，而不是无限泛化平台。**

---

## 相关项目

- `aegiscore-rust-runtime`
  下游执行运行时，负责消费 `aegiscore-rust-author` 生成的 blueprint package 与 machine-readable contract。

---

## 一句话总结

`aegiscore-rust-author` 用来把想法、外部蓝图和项目上下文，编译成可验证、可审计、可交接的 machine-readable blueprint package，服务于 staged AI-driven development。
