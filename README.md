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

    ## 🧩 WASM 宿主函数

agent-core 为 WASM 插件提供了以下宿主函数，所有函数遵循能力基安全模型（默认拒绝，需显式授权）：

| 函数名 | 描述 | 安全限制 |
|--------|------|----------|
| `log` | 输出日志 | 无 |
| `storage_get` / `storage_set` | 插件隔离的键值存储 | 每个插件实例独立存储 |
| `http_get` | 发起 HTTP GET 请求 | 域名白名单、响应大小限制 |
| `workspace_write` | 写入文件到工作区 | 路径必须相对，禁止 `..`，大小限制 10MB |
| `workspace_list` | 列出工作区目录内容 | 路径必须相对，返回 JSON 数组 |
| `env_get` | 获取环境变量 | 仅允许预设的键名 |
| `random_bytes` | 生成加密安全随机数 | 无 |
| `sleep_ms` | 延迟执行（阻塞） | 最大 10 秒 |

详细说明和调用示例请参阅 [WASM 宿主函数文档](docs/wasm_host_functions.md)。