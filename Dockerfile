# 第一阶段：构建
FROM rust:1.84 AS builder

WORKDIR /app

# 复制依赖清单
COPY Cargo.toml Cargo.lock ./

# 创建所有 bench 占位文件和 src/main.rs
RUN mkdir -p benches src && \
    touch benches/performance.rs \
          benches/idempotency_benchmark.rs \
          benches/idempotency_bench.rs \
          benches/scheduler_bench.rs \
          benches/wasm_bench.rs && \
    echo "fn main() {}" > src/main.rs

# 预构建依赖（利用缓存）
RUN cargo build --release --features redis-scheduler,sled-storage || true

# 清理旧的编译产物，准备实际构建
RUN rm -f target/release/deps/agent_core_temp*

# 复制完整源码（将覆盖之前创建的占位文件）
COPY . .

# 实际构建
RUN cargo build --release --features redis-scheduler,sled-storage

# 第二阶段：运行镜像
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制编译好的二进制
COPY --from=builder /app/target/release/agent-core-temp /usr/local/bin/

# 创建数据目录
RUN mkdir -p /data/sled

EXPOSE 3000

# 默认环境变量（可在运行时覆盖）
ENV SCHEDULER_TYPE=redis
ENV REDIS_URL=unix:///var/run/redis/redis.sock
ENV IDEMPOTENCY_STORAGE=sled
ENV SLED_PATH=/data/sled

CMD ["agent-core-temp"]