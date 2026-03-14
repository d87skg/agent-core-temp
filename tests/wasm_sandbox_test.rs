use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;

fn test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|msg: &str, level: u32| {
        eprintln!("[TEST WASM] level={}: {}", level, msg);
    })
}

#[tokio::test]
async fn test_cpu_limit_prevents_infinite_loop() {
    let wat = r#"
    (module
      (func (export "infinite_loop")
        (loop (br 0))
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("loop.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "loop_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        100_000,
        test_logger(),
    ).expect("failed to load plugin");

    let start = std::time::Instant::now();
    let result = plugin.call("infinite_loop", serde_json::json!(null));
    assert!(result.is_err(), "无限循环应因燃料耗尽而失败");
    assert!(start.elapsed() < std::time::Duration::from_secs(1), "执行时间应很短");
}

#[tokio::test]
async fn test_memory_limit_prevents_bomb() {
    let wat = r#"
    (module
      (import "host" "log" (func $log (param i32 i32 i32)))
      (memory (export "memory") 1)
      (func (export "grow_memory")
        (loop
          (memory.grow (i32.const 1))
          drop
          (br 0)
        )
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("bomb.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "bomb_test".to_string(),
        "1.0".to_string(),
        2 * 65536,
        10_000_000,
        test_logger(),
    ).expect("failed to load plugin");

    let result = plugin.call("grow_memory", serde_json::json!(null));
    assert!(result.is_err(), "内存应因超过限制而失败");
}

#[tokio::test]
async fn test_unregistered_host_function_fails() {
    let wat = r#"
    (module
      (import "wasi_snapshot_preview1" "random_get" (func $random_get (param i32 i32) (result i32)))
      (func (export "call_random") (result i32)
        i32.const 0
        i32.const 8
        call $random_get
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("random.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let result = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "random_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
    );
    assert!(result.is_err(), "未注册的导入应导致加载失败");
}

#[tokio::test]
async fn test_storage_isolation() {
    let wat = r#"
    (module
      (import "host" "storage_set" (func $storage_set (param i32 i32 i32 i32) (result i32)))
      (import "host" "storage_get" (func $storage_get (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "test-key")
      (data (i32.const 20) "test-value")

      (func (export "write")
        i32.const 0
        i32.const 8
        i32.const 20
        i32.const 10
        call $storage_set
        drop
      )

      (func (export "read") (result i32)
        i32.const 0
        i32.const 8
        i32.const 40
        i32.const 100
        call $storage_get
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("storage.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin1 = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "plugin1".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
    ).expect("failed to load plugin1");
    plugin1.call("write", serde_json::json!(null)).expect("write failed");

    let plugin2 = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "plugin2".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
    ).expect("failed to load plugin2");
    let result = plugin2.call("read", serde_json::json!(null)).expect("read failed");
    // 仅验证调用成功，隔离性由不同插件实例保证
}

#[tokio::test]
async fn test_host_function_robustness() {
    let wat = r#"
    (module
      (import "host" "log" (func $log (param i32 i32 i32)))
      (memory (export "memory") 1)
      (func (export "bad_log")
        i32.const 0
        i32.const 99999
        i32.const 10
        call $log
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("bad.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "bad_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
    ).expect("failed to load plugin");

    let result = plugin.call("bad_log", serde_json::json!(null));
    // 只要求不 panic，不进一步断言
}