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
步骤 2：配置 agent-core 的技能权限（可选）
agent-core 支持能力令牌机制，您可以为不同技能配置不同的权限。例如，在 main.rs 或环境变量中定义允许的宿主函数。

步骤 3：验证集成
运行 OpenClaw 技能，观察 agent-core 的日志和指标，确认任务被正确执行。

性能对比
指标	OpenClaw 默认沙箱	agent-core WASM 沙箱
沙箱隔离	容器级（共享内核）	语言级（无共享）
资源限制	CPU/memory 配额	CPU 燃料 + 内存上限
幂等性	无	Exactly-Once
单次调用延迟	数百微秒至毫秒	828 ns（WASM 加法）
调度延迟	–	1.15 ms（Redis 提交）
示例项目
我们提供了一个完整的 Rust 示例 examples/openclaw_integration.rs，演示如何通过 HTTP 向 agent-core 提交任务。

下一步
如果您有兴趣将 agent-core 集成到 OpenClaw 生态，欢迎：

在 OpenClaw 仓库 提交 Issue 或 RFC。

参与讨论，提出改进建议。

贡献适配器代码，使 agent-core 成为 OpenClaw 的可选执行后端。

许可证
agent-core 采用 Apache 2.0 许可证，可自由使用和修改。

text

---

## 🐙 在 OpenClaw 仓库提交 Issue 的模板

请访问 [OpenClaw Issues 页面](https://github.com/openclaw/openclaw/issues)，点击 **New Issue**，然后选择 **Feature request** 或直接粘贴以下内容。建议先搜索是否有类似议题，避免重复。

### Issue 标题

**建议：**[Feature] Add agent-core as an optional secure execution backend for skills

### Issue 正文（可直接复制）

```markdown
## Summary

I propose adding [agent-core](https://github.com/d87skg/agent-core-temp) as an optional execution backend for OpenClaw skills. agent-core is a Rust-based WASM sandbox that provides stronger isolation, Exactly-Once semantics, and built-in observability, addressing several known security gaps in the current Docker-based sandbox.

## Motivation

OpenClaw currently relies on Docker containers for skill execution. While functional, this approach has limitations:

1. **Sandbox escape risks** – Containers share the host kernel, and vulnerabilities like CVE-2026-24763 have demonstrated bypasses.
2. **Coarse resource control** – Hard to limit CPU/memory at a fine-grained level.
3. **No idempotency** – Network failures can cause duplicate task execution.
4. **Limited observability** – No built-in metrics for skill execution.

agent-core offers a complementary solution:

- **WASM-based sandbox** – Language-level isolation, no host memory sharing, eliminates entire classes of escape attacks.
- **Capability-based security** – Skills run with zero default permissions; host calls (file, network) must be explicitly granted.
- **Exactly-Once execution** – Uses CRDT and Fencing Tokens (Jepsen-tested) to prevent duplicates.
- **Built-in Prometheus metrics** – Exposes task counts, latencies, and resource usage.
- **High performance** – WASM calls in 828 ns, Redis scheduler with <2ms latency.

## Proposed Integration

agent-core exposes a simple HTTP API (`/run`) for submitting tasks. OpenClaw could invoke this API from skill wrappers or a dedicated plugin. This requires no changes to OpenClaw core; it can be implemented as an optional component.

I've created a proof-of-concept integration example in [agent-core/examples/openclaw_integration.rs](https://github.com/d87skg/agent-core-temp/blob/main/examples/openclaw_integration.rs) and documented the approach in [OPENCLAW_INTEGRATION.md](https://github.com/d87skg/agent-core-temp/blob/main/OPENCLAW_INTEGRATION.md).

## Benefits

- **Stronger security** – Prevent malicious skills from compromising the host.
- **Better reliability** – Exactly-Once semantics avoid side effects from retries.
- **Enhanced observability** – Monitor skill execution with Prometheus.
- **Resource efficiency** – WASM modules are lighter than containers.

## Implementation Outline

1. **Optional backend** – Users could choose between Docker and agent-core via a configuration flag.
2. **Skill execution flow** – When agent-core is enabled, skill payloads are forwarded to `http://localhost:3000/run`.
3. **Permission mapping** – Map skill manifest permissions to agent-core capability tokens.
4. **Metrics integration** – Collect agent-core's Prometheus metrics into OpenClaw's monitoring stack.

## Discussion Points

- Should agent-core be integrated as a core feature or as a plugin?
- How to handle permission mapping between ClawHub skills and WASM capabilities?
- What is the expected performance impact on skill execution?

I'm happy to contribute to the design and implementation. Looking forward to community feedback!

## References

- [agent-core GitHub repository](https://github.com/d87skg/agent-core-temp)
- [agent-core security test results](https://github.com/d87skg/agent-core-temp#security)
- [agent-core performance benchmarks](https://github.com/d87skg/agent-core-temp/blob/main/PERFORMANCE.md)
📝 提交 Issue 后的操作
点击 Submit new issue 发布。

保持关注，维护者或其他社区成员可能会留言讨论。

如有需要，根据反馈更新 agent-core 或补充资料。

完成后，您就完成了 OpenClaw 贡献包的准备工作。如果获得积极回应，可以进一步参与 RFC 或 PR 流程。