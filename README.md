markdown
# agent-core

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

GitHub Discussions: https://github.com/d87skg/agent-core/discussions

text

### 主要修正点
1. **代码块格式**：确保每个代码块（如 `bash`、`json`）都被正确包裹在三个反引号中，并且内容在代码块内部。
2. **链接格式**：将 `CONTRIBUTING.md`、`LICENSE` 等链接统一为 Markdown 格式。
3. **占位符**：保留 `你的用户名` 部分，你需要将其替换为你的实际 GitHub 用户名（例如 `zhangsan`）。如果不确定，可以先不管，后续在 GitHub 网页上直接编辑。
4. **URL 格式**：将 `http://127.0.0.1:3000` 包裹在反引号内，避免被误认为链接。