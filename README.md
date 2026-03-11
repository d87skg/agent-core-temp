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
