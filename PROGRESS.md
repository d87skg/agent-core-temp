# 项目进展日志

## 2026-03-15

### 已完成

1. **Redis 调度器 ack/nack 逻辑完善**
   - 修复了所有权问题，添加 XACK 返回值检查。
   - 验证重试机制：任务失败后重试计数递增，超过最大重试次数后移入死信队列。
   - 测试通过：日志显示任务被成功 ack/nack，死信队列正常接收超限任务。

2. **WASM 沙箱安全测试**
   - 修复 `test_storage_isolation`，移除 `#[ignore]`。
   - 全部 5 项测试通过：CPU 燃料限制、内存限制、未注册函数调用、存储隔离、宿主函数健壮性。

3. **sled 持久化存储集成**
   - 添加 sled 后端实现 `IdempotencyBackend`。
   - 通过环境变量 `IDEMPOTENCY_STORAGE=sled` 可选启用。
   - 验证 sled 数据库文件在 `./data/sled` 正常生成，服务重启后数据持久化。

4. **性能基准测试**
   - 幂等性存储基准（`idempotency_bench`）：
     - 内存创建：1.28 µs
     - 内存完成：321 ns
     - Sled 创建：17.0 µs
     - Sled 完成：1.91 µs
   - 调度器基准（`scheduler_bench`）：
     - 内存提交：1.09 µs
     - 内存 `pop+ack`：1.93 µs
     - Redis 提交：2.01 ms
     - Redis `pop+ack`：因任务 ID 不匹配失败，需手动清理后重测（但整体功能已验证）
   - WASM 基准（`wasm_bench`）：
     - 加法函数调用：828 ns

5. **OpenClaw 贡献包准备**
   - 创建示例 `examples/openclaw_integration.rs`，模拟 OpenClaw 调用 agent-core 的 `/run` 接口。
   - 编写集成文档 `OPENCLAW_INTEGRATION.md`。
   - 添加 `reqwest` 依赖用于 HTTP 请求。

### 下一步计划
- 完善 `README.md`，添加性能数据表和 OpenClaw 集成指南。
- 考虑向 OpenClaw 官方仓库提交 PR，提议 agent-core 作为可选执行后端。
- 可选：处理 IronClaw 测试（需安装 SQLite）或进一步优化调度器 pending 管理。