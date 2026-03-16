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