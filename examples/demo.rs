// examples/demo.rs
use agent_core_temp::extension::load_extension;
use agent_core_temp::runtime::executor::ParallelExecutor;
use agent_core_temp::observability;
use std::sync::Arc;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化可观测性（指标将通过 ingress 暴露）
    observability::init_observability().expect("failed to init observability");
    println!("✅ Observability initialized (metrics available via ingress on port 3000)");

    // 2. 创建一个 WASM 插件（动态生成，无需预编译）
    let wasm_bytes = wat::parse_str(r#"
        (module
            (import "host" "log" (func $log (param i32 i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "Hello from WASM!")

            ;; 让 greet 返回 i32 0，以匹配 call 的期望
            (func (export "greet") (result i32)
                i32.const 1   ;; level
                i32.const 0   ;; ptr
                i32.const 16  ;; len
                call $log
                i32.const 0   ;; 返回 0
            )
        )
    "#).expect("failed to parse WAT");

    // 将 WASM 字节码写入临时文件
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("demo.wasm");
    std::fs::write(&wasm_path, wasm_bytes).expect("failed to write wasm file");

    // 3. 加载 WASM 插件
    let plugin = load_extension(wasm_path.to_str().unwrap())
        .expect("failed to load plugin");
    println!("✅ Loaded plugin: {} v{}", plugin.name(), plugin.version());

    // 4. 调用插件中的 "greet" 函数（现在返回 0，不会出错）
    let result = plugin.call("greet", serde_json::json!(null))
        .expect("failed to call greet");
    println!("✅ Greet called, returned: {}", result);

    // 5. 使用并发执行器执行阻塞任务
    let executor = Arc::new(ParallelExecutor::new(4)); // 最大并发 4
    let mut handles = vec![];

    for i in 0..8 {
        let executor = executor.clone();
        handles.push(tokio::spawn(async move {
            let task_id = format!("task-{}", i);
            let result = executor.execute(task_id.clone(), move || {
                // 模拟耗时计算
                std::thread::sleep(std::time::Duration::from_millis(100));
                i * i
            }).await.expect("task execution failed");
            println!("✅ Task {} completed: {}", task_id, result);
            Ok::<_, anyhow::Error>(())
        }));
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("task panicked")?;
    }

    // 6. 记录自定义指标
    observability::inc_requests(42);
    observability::record_request_duration(0.123);

    println!("\n🎉 All done! If the ingress server is running, visit http://127.0.0.1:3000/metrics to see metrics.");

    // 保持程序运行一小段时间，方便观察输出
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    Ok(())
}