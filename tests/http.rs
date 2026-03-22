use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tempfile::tempdir;

fn test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|msg: &str, level: u32| {
        eprintln!("[TEST WASM] level={}: {}", level, msg);
    })
}

// 移除 #[tokio::test]，使用普通测试函数
#[test]
fn test_http_post_basic() {
    let wat = r#"
    (module
      (import "env" "http_post" (func $http_post (param i32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "https://httpbin.org/post")
      (data (i32.const 50) "{\"test\":\"data\"}")
      (func (export "run_test") (result i32)
        i32.const 0      ;; url_ptr
        i32.const 22     ;; url_len
        i32.const 50     ;; body_ptr
        i32.const 15     ;; body_len
        i32.const 1024   ;; result_ptr
        call $http_post
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_dir = tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("http_post.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "http_post_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec!["httpbin.org".to_string()],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("run_test", serde_json::json!(null));
    assert!(result.is_ok(), "http_post failed: {:?}", result.err());
}