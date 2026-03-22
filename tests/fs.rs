use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tempfile::tempdir;

fn test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|msg: &str, level: u32| {
        eprintln!("[TEST WASM] level={}: {}", level, msg);
    })
}

#[tokio::test]
async fn test_workspace_read() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    // 先写一个测试文件
    let test_file = workspace_path.join("test.txt");
    let test_content = b"Hello from workspace_read test!";
    std::fs::write(&test_file, test_content).unwrap();

    let wat = r#"
    (module
      (import "env" "workspace_read" (func $workspace_read (param i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "test.txt")
      (func (export "read_file") (result i32)
        i32.const 0      ;; path_ptr
        i32.const 8      ;; path_len
        i32.const 1024   ;; result_ptr
        call $workspace_read
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("fs_read.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "fs_read_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        Some(workspace_path.clone()),
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("read_file", serde_json::json!(null));
    assert!(result.is_ok(), "workspace_read failed: {:?}", result.err());
}

#[tokio::test]
async fn test_workspace_delete_with_confirm() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    // 先写一个测试文件
    let test_file = workspace_path.join("delete_me.txt");
    let test_content = b"to be deleted";
    std::fs::write(&test_file, test_content).unwrap();
    assert!(test_file.exists());

    let wat = r#"
    (module
      (import "env" "workspace_delete" (func $workspace_delete (param i32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "delete_me.txt")
      (data (i32.const 50) "yes")
      (func (export "delete_file") (result i32)
        i32.const 0      ;; path_ptr
        i32.const 13     ;; path_len
        i32.const 50     ;; confirm_ptr
        i32.const 3      ;; confirm_len
        i32.const 1024   ;; result_ptr
        call $workspace_delete
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("fs_delete.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "fs_delete_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        Some(workspace_path.clone()),
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("delete_file", serde_json::json!(null));
    assert!(result.is_ok(), "workspace_delete failed: {:?}", result.err());

    // 验证文件已被删除
    assert!(!test_file.exists(), "file should have been deleted");
}