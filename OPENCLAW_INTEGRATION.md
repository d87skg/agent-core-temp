# 将 agent-core 与 OpenClaw 集成

[agent-core](https://github.com/d87skg/agent-core-temp) 是一个 Rust 编写的 AI 代理执行引擎，提供 Exactly-Once 保障、WASM 沙箱安全隔离和分布式调度能力。本文档说明如何将 agent-core 作为 OpenClaw 的可选执行后端，以增强技能执行的安全性和可靠性。

## 为什么选择 agent-core？

OpenClaw 目前使用 Docker 容器作为技能执行沙箱，但存在以下局限性：
- **沙箱绕过风险**：容器共享内核，存在逃逸漏洞（如 CVE-2026-24763）。
- **资源控制粗粒度**：难以精确限制 CPU 和内存。
- **缺乏幂等性保障**：网络故障可能导致任务重复执行。

agent-core 提供以下优势：
- **WASM 沙箱**：语言级隔离，无宿主内存共享，彻底杜绝沙箱绕过。
- **能力令牌**：默认零权限，需显式授权（如文件读写、网络请求）。
- **Exactly-Once 执行**：通过 CRDT 和 Fencing Tokens 确保任务不重复。
- **可观测性**：内置 Prometheus 指标，实时监控任务状态。
- **高性能**：内存操作纳秒级，WASM 调用 828 ns，Redis 调度延迟 <2ms。

## 集成方式

agent-core 通过 HTTP API 提供服务，OpenClaw 可以在技能执行时调用 agent-core 的 `/run` 接口，将任务提交给 agent-core 执行。这种方式无需修改 OpenClaw 核心，只需在技能或插件层进行适配。

### 前提条件
- 已安装 agent-core 并启动服务（参考 [快速开始](README.md#快速开始)）。
- OpenClaw 环境（可通过 Docker 或源码安装）。

### 步骤 1：在 OpenClaw 中调用 agent-core 接口

以下是一个简单的 TypeScript 示例，展示如何在 OpenClaw 技能中调用 agent-core：

```typescript
// skill.ts – 在 OpenClaw 技能中调用 agent-core
import fetch from 'node-fetch';

async function runOnAgentCore(taskType: string, payload: any): Promise<string> {
  const response = await fetch('http://localhost:3000/run', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ intent: { type: taskType, payload } })
  });
  const data = await response.json();
  return data.task_id; // agent-core 返回的任务 ID
}