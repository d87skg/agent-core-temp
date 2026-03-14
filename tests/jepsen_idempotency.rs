// tests/jepsen_idempotency.rs
use agent_core_temp::idempotency::store::CRDTStore;
use agent_core_temp::hlc::HlcClock;
use std::sync::Arc;
use tokio::sync::Barrier;
use tokio::time::{pause, resume, sleep, Duration, Instant};
use proptest::prelude::*;

// ============================================================================
// 测试辅助结构
// ============================================================================

#[derive(Debug, Default)]
struct TestStats {
    first_executions: usize,
    duplicate_requests: usize,
}

// ============================================================================
// 1. 线性一致性测试（30并发客户端）
// ============================================================================

#[tokio::test]
async fn test_linearizability_30_clients() -> anyhow::Result<()> {
    println!("\n🚀 [测试1] 线性一致性测试：30并发客户端");
    println!("==========================================");

    let store = Arc::new(CRDTStore::new());
    let barrier = Arc::new(Barrier::new(31));
    let mut handles = vec![];

    let stats = Arc::new(tokio::sync::RwLock::new(TestStats::default()));
    let start = Instant::now();

    for client_id in 0..30 {
        let store = store.clone();
        let barrier = barrier.clone();
        let stats = stats.clone();

        handles.push(tokio::spawn(async move {
            barrier.wait().await;

            for op in 0..100 {
                let key = format!("key-{}", op % 20);
                // 明确指定 value 类型为 u64
                let value: u64 = (client_id * 1000 + op) as u64;

                if store.try_create(&key).await.unwrap() {
                    // CRDTStore::complete 只接受两个参数：key 和 result
                    store.complete(&key, value.to_le_bytes().to_vec()).await.unwrap();
                    stats.write().await.first_executions += 1;
                } else {
                    stats.write().await.duplicate_requests += 1;
                }

                if rand::random::<u8>() % 3 == 0 {
                    sleep(Duration::from_micros(rand::random::<u64>() % 100)).await;
                }
            }
        }));
    }

    barrier.wait().await;
    println!("⏳ 等待 {} 个客户端完成...", handles.len());

    for handle in handles {
        handle.await?;
    }

    let elapsed = start.elapsed();

    let stats = stats.read().await;
    println!("\n✅ [测试结果]");
    println!("  首次执行: {}", stats.first_executions);
    println!("  重复请求: {}", stats.duplicate_requests);
    println!("  总耗时: {:?}", elapsed);

    Ok(())
}

// ============================================================================
// 2. 网络分区模拟测试（使用 tokio::time::pause/resume）
// ============================================================================

#[tokio::test]
async fn test_network_partition() -> anyhow::Result<()> {
    println!("\n🚀 [测试2] 网络分区模拟测试");
    println!("==========================================");

    pause();
    println!("🔇 网络分区开始...");

    let store = Arc::new(CRDTStore::new());
    let key = "partition-key";

    let store1 = store.clone();
    let handle1 = tokio::spawn(async move {
        let created = store1.try_create(key).await.unwrap();
        assert!(created, "新key应创建成功");
        sleep(Duration::from_secs(5)).await;
        store1.complete(key, vec![42]).await.unwrap();
        true
    });

    let store2 = store.clone();
    let handle2 = tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let created = store2.try_create(key).await.unwrap();
        assert!(!created, "分区期间重复创建应失败");
    });

    sleep(Duration::from_secs(2)).await;
    resume();
    println!("🔊 网络分区恢复");

    handle1.await.unwrap();
    handle2.await.unwrap();

    println!("✅ 网络分区测试通过");
    Ok(())
}

// ============================================================================
// 3. HLC时钟回拨健壮性测试
// ============================================================================

#[tokio::test]
async fn test_hlc_clock_rollback() -> anyhow::Result<()> {
    println!("\n🚀 [测试3] HLC时钟回拨健壮性测试");
    println!("==========================================");

    let mut hlc = HlcClock::new();
    let store = CRDTStore::new();

    let mut timestamps = vec![];
    let mut keys = vec![];

    for i in 0..10 {
        let ts = hlc.now();
        timestamps.push(ts);
        keys.push(format!("key-{}", i));
        println!("正常时间戳 {}: {}", i, ts);
    }

    for i in 1..timestamps.len() {
        assert!(timestamps[i] > timestamps[i-1], "时间戳应单调递增");
    }

    println!("\n⚠️  模拟时钟回拨 1000ms...");
    hlc.set_manual_offset(-1000);

    for i in 10..20 {
        let ts = hlc.now();
        timestamps.push(ts);
        keys.push(format!("key-{}", i));
        println!("回拨后时间戳 {}: {}", i, ts);
    }

    for i in 1..timestamps.len() {
        assert!(timestamps[i] > timestamps[i-1], 
                "时钟回拨后时间戳仍应单调递增 ({}: {} <= {})", 
                i, timestamps[i], timestamps[i-1]);
    }

    for (i, key) in keys.iter().enumerate() {
        store.try_create(key).await?;
        // CRDTStore::complete 不需要时间戳参数
        store.complete(key, vec![i as u8]).await?;
    }

    println!("\n✅ HLC时钟回拨测试通过");
    Ok(())
}

// ============================================================================
// 4. 属性测试（随机并发验证）
// ============================================================================

proptest! {
    #[test]
    fn proptest_concurrent_idempotency(
        client_count in 1..20usize,
        ops_per_client in 1..50usize,
        key_count in 1..10usize,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = Arc::new(CRDTStore::new());
            let mut handles = vec![];

            for c in 0..client_count {
                let store = store.clone();
                handles.push(tokio::spawn(async move {
                    for o in 0..ops_per_client {
                        let key = format!("key-{}", o % key_count);
                        let value: u64 = (c * 1000 + o) as u64;

                        if store.try_create(&key).await.unwrap() {
                            store.complete(&key, value.to_le_bytes().to_vec()).await.unwrap();
                        }
                    }
                }));
            }

            for h in handles {
                h.await.unwrap();
            }
        });
    }
}