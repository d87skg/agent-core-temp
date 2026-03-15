# agent-core ⚙️ · AI Agent 的可靠执行层

[![Crates.io][crates-badge]][crates-url]
[![License][license-badge]][license-url]
[![Jepsen Test][jepsen-badge]][jepsen-url]
[![WASM Sandbox][wasm-badge]][wasm-url]

[crates-badge]: https://img.shields.io/crates/v/agent-core-temp
[crates-url]: https://crates.io/crates/agent-core-temp
[license-badge]: https://img.shields.io/badge/license-Apache--2.0-blue
[license-url]: https://github.com/d87skg/agent-core-temp/blob/main/LICENSE
[jepsen-badge]: https://img.shields.io/badge/Jepsen-✔️%20linearizable-brightgreen
[jepsen-url]: https://github.com/d87skg/agent-core-temp/tree/main/tests/jepsen_idempotency.rs
[wasm-badge]: https://img.shields.io/badge/WASM%20Sandbox-✔️%20secure-brightgreen
[wasm-url]: https://github.com/d87skg/agent-core-temp/tree/main/tests/wasm_sandbox_test.rs

---

## 📌 一句话定位

**agent-core** 是一个为 AI Agent（智能体）设计的**可靠执行引擎**——让代理拥有 Exactly‑Once 执行保障、可验证身份和资产所有权的能力。  
它是连接 AI “大脑”与外部世界的**可信神经中枢**。

---

## 🧩 架构图

```mermaid
flowchart TB
    subgraph AI Agent Frameworks
        A[OpenClaw / NemoClaw / CoPaw ...]
    end
    subgraph agent-core
        direction TB
        B[幂等性核心<br/>Fencing Tokens + CRDT]
        C[WASM 沙箱<br/>内存/CPU 隔离]
        D[可观测性<br/>Prometheus 指标]
        E[分布式调度器<br/>Redis]
    end
    subgraph External World
        F[API / 数据库 / 区块链]
    end
    A -- "提交任务" --> B
    B -- "执行 Exactly-Once" --> C
    C -- "安全调用" --> F
    B -.-> D
    E -.-> B
    D --> F
    说明：

AI Agent 框架将任务提交给 agent-core，由幂等性核心确保任务无论执行多少次都只有一次生效。

任务逻辑在 WASM 沙箱中运行，保证隔离与安全。

所有执行步骤均通过 Prometheus 暴露指标，便于监控。

Redis 调度器支持分布式任务分发，并已实现重试和死信队列。
✨ 核心特性
🔁 Exactly‑Once 执行保障
基于 Fencing Tokens + CRDT 存储，通过 Jepsen 风格的线性一致性、网络分区、时钟回拨测试。

🔒 安全的 WASM 沙箱
内存限制、CPU 燃料（Fuel）、能力隔离，防止无限循环、内存爆炸和未授权系统调用。
已通过 5 项安全测试验证。

📊 内置可观测性
Prometheus 指标（任务数、重试次数、WASM 执行耗时等）自动集成，开箱即用。

⚙️ 高性能并发执行器
基于 tokio 的并行执行器，支持高并发任务调度。

🌐 分布式调度器
Redis 实现，支持任务重试、死信队列，通过 pipeline 优化后单次操作延迟 <1.2ms。

🧩 模块化设计
19 个冻结模块，严格遵循 27 条宪法原则，接口永不修改。