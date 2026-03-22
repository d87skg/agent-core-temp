use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tempfile::tempdir;

fn test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|msg: &str, level: u32| {
        eprintln!("[TEST WASM] level={}: {}", level, msg);
    })
}

#[tokio::test]
async fn test_cost_tracker() {
    let wat = r#"
    (module
      (import "env" "cost_tracker" (func $cost_tracker (param i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (func (export "track_cost") (result i32)
        i32.const 100    ;; tokens
        i32.const 5      ;; cost_cents
        i32.const 1024   ;; result_ptr
        call $cost_tracker
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");

    let temp_wasm_dir = tempfile::tempdir().expect("failed to create temp wasm dir");
    let wasm_path = temp_wasm_dir.path().join("cost.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = load_wasm_plugin(
        wasm_path.to_str().unwrap(),
        "cost_test".to_string(),
        "1.0".to_string(),
        64 * 1024 * 1024,
        10_000_000,
        test_logger(),
        vec![],
        vec![],
        None,
        1024 * 1024,
    ).expect("failed to load plugin");

    let result = plugin.call("track_cost", serde_json::json!(null));
    assert!(result.is_ok(), "cost_tracker failed: {:?}", result.err());
}