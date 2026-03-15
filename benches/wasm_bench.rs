use criterion::{criterion_group, criterion_main, Criterion};
use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn create_test_logger() -> Arc<dyn Fn(&str, u32) + Send + Sync> {
    Arc::new(|_, _| {})
}

fn bench_wasm_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // 生成一个简单的 WASM 模块（动态生成）
    let wat = r#"
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add
      )
    )
    "#;
    let wasm = wat::parse_str(wat).expect("failed to parse WAT");
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let wasm_path = temp_dir.path().join("add.wasm");
    std::fs::write(&wasm_path, wasm).expect("failed to write wasm file");

    let plugin = rt.block_on(async {
        load_wasm_plugin(
            wasm_path.to_str().unwrap(),
            "add_bench".to_string(),
            "1.0".to_string(),
            64 * 1024 * 1024,
            10_000_000,
            create_test_logger(),
        ).expect("failed to load plugin")
    });

    c.bench_function("wasm_add", |b| {
        b.to_async(&rt).iter(|| async {
            // 注意：call 是同步方法，不需要 .await
            let _ = plugin.call("add", serde_json::json!([3, 5]));
        })
    });
}

criterion_group!(wasm_benches, bench_wasm_add);
criterion_main!(wasm_benches);