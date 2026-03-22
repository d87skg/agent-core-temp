use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tempfile::tempdir;

fn test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|msg: &str, level: u32| {
        eprintln!("[TEST WASM] level={}: {}", level, msg);
    })
}

#[tokio::test]
async fn test_permission_check() {
    let wat = r#"
    (module
      (import "env" "permission_check" (func $permission_check (param i32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "http")
      (data (i32.const 50) "get")
      (func (export "check_http_get") (result i32)
        i32.const 0      ;; resource_ptr
        i32.const 4      ;; resource_len
        i32.const 50     ;; action_ptr
        i32.const 3      ;; action_len
        i32.const 1024   ;; result_ptr
        call $permission_check
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("permission.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "permission_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec!["https://example.com".to_string()],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("check_http_get", serde_json::json!(null));
    assert!(result.is_ok(), "permission_check failed: {:?}", result.err());
}

#[tokio::test]
async fn test_audit_log() {
    let wat = r#"
    (module
      (import "env" "audit_log" (func $audit_log (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "file_read")
      (data (i32.const 50) "test.txt")
      (data (i32.const 100) "success")
      (data (i32.const 150) "read 100 bytes")
      (func (export "log_file_read") (result i32)
        i32.const 0      ;; operation_ptr
        i32.const 8      ;; operation_len
        i32.const 50     ;; resource_ptr
        i32.const 8      ;; resource_len
        i32.const 100    ;; status_ptr
        i32.const 7      ;; status_len
        i32.const 150    ;; details_ptr
        i32.const 13     ;; details_len
        call $audit_log
        drop
        i32.const 0
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("audit.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "audit_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("log_file_read", serde_json::json!(null));
    assert!(result.is_ok(), "audit_log failed: {:?}", result.err());
}