use criterion::{criterion_group, criterion_main, Criterion};
use agent_core_temp::extension::wasm::load_wasm_plugin;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn bench_wasm_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (func $add (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
            (export "add" (func $add))
        )
        "#,
    )
    .unwrap();
    let temp_dir = tempfile::tempdir().unwrap();
    let wasm_path = temp_dir.path().join("add.wasm");
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let logger = Arc::new(|_: &str, _: u32| {});
    let plugin = rt.block_on(async {
        load_wasm_plugin(
            wasm_path.to_str().unwrap(),
            "bench".to_string(),
            "1.0".to_string(),
            64 * 1024 * 1024,
            10_000_000,
            logger,
        )
        .unwrap()
    });

    c.bench_function("wasm_add", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = plugin.call("add", serde_json::json!([3, 5]));
            });
        })
    });
}

criterion_group!(benches, bench_wasm_add);
criterion_main!(benches);