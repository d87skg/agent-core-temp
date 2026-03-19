# Redis 调度器

## 概述
基于 Redis Streams 的分布式任务调度器，支持消费者组、任务确认（ACK）、负确认（NACK）、重试和死信队列。

## 可靠性保证
- **Exactly-Once 语义**：通过消费者组的 PEL（Pending Entries List）和 XACK 机制，确保每个任务至少被处理一次，结合幂等性实现 Exactly-Once。
- **原子性**：`nack` 操作使用 Redis pipeline 同时执行 XACK（确认原消息）和 XADD（重新提交或移入死信），确保操作原子性，避免消息丢失或重复。
- **死信队列**：当任务重试次数达到 `max_retries` 后，自动移入死信队列 `{stream}:dead`，可通过 `dead_letter_count()` 查询。

## 监控指标
通过 Prometheus 暴露以下指标（访问 `/metrics`）：
- `redis_tasks_submitted_total`：提交的任务总数
- `redis_tasks_popped_total`：弹出的任务总数
- `redis_tasks_acked_total`：确认的任务总数
- `redis_tasks_nacked_total`：负确认的任务总数
- `redis_dead_letter_size`：死信队列当前长度
- `redis_operation_failures_total`：Redis 操作失败次数

## 配置参数
- `max_retries`：最大重试次数（默认 3）
- 其他 Redis 连接参数通过环境变量设置（如 `REDIS_URL`）