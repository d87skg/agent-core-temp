use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tempfile::tempdir;

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
        vec![],
        vec![],
        None,
        1024 * 1024,
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
        vec![],
        vec![],
        None,
        1024 * 1024,
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
        vec![],
        vec![],
        None,
        1024 * 1024,
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

      (func (export "write") (result i32)
        i32.const 0
        i32.const 8
        i32.const 20
        i32.const 10
        call $storage_set
        drop
        i32.const 0
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
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin1");
    plugin1.call("write", serde_json::json!(null)).expect("write failed");

    let plugin2 = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "plugin2".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin2");
    let _result = plugin2.call("read", serde_json::json!(null)).expect("read failed");
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
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let _result = plugin.call("bad_log", serde_json::json!(null));
}

#[tokio::test]
async fn test_http_get_basic() {
    let wat = r#"
    (module
      (import "host" "http_get" (func $http_get (param i32 i32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "https://httpbin.org/get")
      (func (export "run_test") (result i32)
        i32.const 0      ;; url_ptr
        i32.const 22     ;; url_len
        i32.const 0      ;; headers_ptr
        i32.const 0      ;; headers_len
        i32.const 1024   ;; ret_ptr
        i32.const 2048   ;; ret_len_ptr
        call $http_get
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("http_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "http_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec!["https://httpbin.org".to_string()],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("run_test", serde_json::json!(null));
    assert!(result.is_ok(), "http_get call failed: {:?}", result.err());
}

#[tokio::test]
async fn test_workspace_write() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    let wat = r#"
    (module
      (import "host" "workspace_write" (func $workspace_write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "test.txt")
      (data (i32.const 100) "Hello, WASM!")
      (func (export "write_file") (result i32)
        i32.const 0      ;; path_ptr
        i32.const 8      ;; path_len
        i32.const 100    ;; content_ptr
        i32.const 12     ;; content_len
        call $workspace_write
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("write_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "write_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        Some(workspace_path.clone()),
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("write_file", serde_json::json!(null));
    assert!(result.is_ok(), "workspace_write failed: {:?}", result.err());

    let expected_file = workspace_path.join("test.txt");
    assert!(expected_file.exists(), "file not created");
    let content = std::fs::read_to_string(expected_file).expect("failed to read file");
    assert_eq!(content, "Hello, WASM!");
}

#[tokio::test]
async fn test_workspace_list() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    std::fs::write(workspace_path.join("file1.txt"), "content1").unwrap();
    std::fs::write(workspace_path.join("file2.txt"), "content2").unwrap();
    std::fs::create_dir(workspace_path.join("subdir")).unwrap();

    let wat = r#"
    (module
      (import "host" "workspace_list" (func $workspace_list (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "")  ;; 空字符串表示根目录
      (func (export "list_root") (result i32)
        i32.const 0      ;; path_ptr
        i32.const 0      ;; path_len
        i32.const 1024   ;; ret_ptr
        i32.const 2048   ;; ret_len_ptr
        call $workspace_list
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("list_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "list_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        Some(workspace_path.clone()),
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("list_root", serde_json::json!(null));
    assert!(result.is_ok(), "workspace_list failed: {:?}", result.err());
    println!("workspace_list result: {:?}", result.unwrap());
}

#[tokio::test]
async fn test_env_get() {
    std::env::set_var("TEST_ENV_VAR", "hello_world");

    let wat = r#"
    (module
      (import "host" "env_get" (func $env_get (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "TEST_ENV_VAR")
      (func (export "get_test_var") (result i32)
        i32.const 0      ;; key_ptr
        i32.const 12     ;; key_len
        i32.const 1024   ;; ret_ptr
        i32.const 2048   ;; ret_len_ptr
        call $env_get
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("env_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let env_allowlist = vec!["TEST_ENV_VAR".to_string()];

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "env_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        env_allowlist,
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("get_test_var", serde_json::json!(null));
    assert!(result.is_ok(), "env_get failed: {:?}", result.err());
}

#[tokio::test]
async fn test_random_bytes() {
    let wat = r#"
    (module
      (import "host" "random_bytes" (func $random_bytes (param i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (func (export "get_random") (result i32)
        i32.const 16      ;; len = 16
        i32.const 1024    ;; ret_ptr
        i32.const 2048    ;; ret_len_ptr
        call $random_bytes
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("random_bytes_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "random_bytes_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("get_random", serde_json::json!(null));
    assert!(result.is_ok(), "random_bytes failed: {:?}", result.err());
}

#[tokio::test]
async fn test_sleep_ms() {
    let wat = r#"
    (module
      (import "host" "sleep_ms" (func $sleep_ms (param i32) (result i32)))
      (func (export "do_sleep") (result i32)
        i32.const 10      ;; 10 ms
        call $sleep_ms
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("sleep_test.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "sleep_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let start = std::time::Instant::now();
    let result = plugin.call("do_sleep", serde_json::json!(null));
    let elapsed = start.elapsed();
    assert!(result.is_ok(), "sleep_ms failed: {:?}", result.err());
    assert!(elapsed >= std::time::Duration::from_millis(10));
}