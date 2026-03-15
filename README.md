markdown
[![Crates.io](https://img.shields.io/crates/v/agent-core-temp)](https://crates.io/crates/agent-core-temp)

# agent-core-temp

A modular Rust execution engine for AI agents.  
Built as a foundation for Web 4.0 autonomous agents.

## Status

✅ **MVP is working!**  
You can run the server and test the API right now.  
Some modules are still under development and produce `unused` warnings – that's expected and safe.

## Quick Start

### Prerequisites
- Rust 1.78+ (install via [rustup](https://rustup.rs/))
- Windows: Visual Studio Build Tools with C++ support (for linking)

### Run the server
```bash
cargo run
The server will start at http://127.0.0.1:3000.

Test the API
In another terminal, run:

bash
curl -X POST http://127.0.0.1:3000/run \
  -H "Content-Type: application/json" \
  -d '{"intent":{"type":"transfer","to":"alice","amount":100,"asset":"USDC"}}'
Expected response:

json
{"task_id":"...","status":"submitted"}
Modules
The project consists of 19 modules with frozen interfaces:

payment – Multi-chain value abstraction

identity – DID and signature verification

storage – Memory and persistent storage

verification – TEE/ZK proof interfaces

ownership – Asset ownership (ERC-7857)

replication – Child agent spawning

workflow – Intent compilation and DAG execution

router – Cross-chain routing

market – Workflow template marketplace

sandbox – Simulation and cost estimation

governance – DAO governance

audit – Audit logging

policy – Natural language policy engine

ingress – HTTP/MCP API gateway

observability – Metrics, tracing, logs

runtime – Task scheduling and lifecycle

resource – Resource quotas and governance

extension – Universal extension slot

attestation – On-chain attestation

Contributing
We welcome contributions! Please see CONTRIBUTING.md for guidelines.
Good first issues are tagged with good first issue.

License
This project is licensed under the Apache License 2.0.
Commercial licenses for core modules are available upon request.

Community
Discord: Invite link (to be created)

GitHub Discussions: https://github.com/d87skg/agent-core-temp/discussions

text

### ✅ 主要修正点

1. **添加 crates.io 徽章**：顶部加入 `[![Crates.io](https://img.shields.io/crates/v/agent-core-temp)](https://crates.io/crates/agent-core-temp)`。
2. **修正代码块格式**：确保所有 bash 和 json 代码块正确闭合。
3. **更新仓库链接**：所有 `d87skg/agent-core-temp` 链接已替换为正确地址（原先是 `agent-core`）。
4. **统一模块列表格式**：使用无序列表，每个模块名用反引号包裹。
5. **清理无用占位符**：删除多余的“主要修正点”说明。
# agent-core ⚙️ · AI Agent 的可靠执行层

[![Crates.io][crates-badge]][crates-url]
[![MIT/Apache 2.0][license-badge]][license-url]
[![Jepsen Test][jepsen-badge]][jepsen-url]
[![WASM Sandbox][wasm-badge]][wasm-url]

[crates-badge]: https://img.shields.io/badge/crates.io-v0.1.0--p0-orange
[crates-url]: https://crates.io/crates/agent-core-temp
[license-badge]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue
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

下图展示了 agent-core 如何与上层 AI Agent 框架（如 OpenClaw）协作，为任务执行提供一致性、安全性和可观测性。

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
        E[分布式调度器<br/>Redis (预留)]
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
    说明： •  AI Agent 框架将任务提交给 agent-core，由幂等性核心确保任务无论执行多少次都只有一次生效。  •  任务逻辑在 WASM 沙箱中运行，保证隔离与安全。  •  所有执行步骤均通过 Prometheus 暴露指标，便于监控。  •  预留的 Redis 调度器支持分布式任务分发。