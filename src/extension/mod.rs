// src/extension/mod.rs
pub mod wasm;

pub trait Extension: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn init(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn call(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>>;
}

pub fn load_extension(path: &str) -> Result<Box<dyn Extension>, Box<dyn std::error::Error>> {
    if path.ends_with(".wasm") {
        use std::sync::Arc;
        
        let logger = Arc::new(|msg: &str, level: u32| {
            match level {
                0 => eprintln!("[WASM ERROR] {}", msg),
                1 => eprintln!("[WASM INFO] {}", msg),
                _ => eprintln!("[WASM DEBUG] {}", msg),
            }
        });
        
        wasm::load_wasm_plugin(
            path,
            "wasm-plugin".to_string(),
            "1.0".to_string(),
            64 * 1024 * 1024,  // 64MB
            10_000_000,         // 10M fuel
            logger,
        )
    } else {
        Err("Unsupported extension type".into())
    }
}